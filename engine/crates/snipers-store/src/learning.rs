//! Immutable observations and delayed outcomes, isolated from trading ledgers.
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::Path, time::Duration};

pub const SCHEMA: &str = "snipers-learning-v1";
pub const HORIZONS: [i64; 3] = [300_000, 900_000, 1_800_000];
pub const TOLERANCE_MS: i64 = 60_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub symbol: String,
    pub observed_ms: u64,
    pub price: Option<f64>,
    pub price_ms: Option<u64>,
    pub eligible: bool,
    pub quality: String,
    pub context: Value,
}

pub struct LearningStore(Connection);
impl LearningStore {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let db = Connection::open(path)?;
        db.busy_timeout(Duration::from_millis(500))?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS learning_meta(schema TEXT PRIMARY KEY);",
        )?;
        let version: Option<String> = db
            .query_row("SELECT schema FROM learning_meta LIMIT 1", [], |r| r.get(0))
            .ok();
        if version.as_deref().is_some_and(|v| v != SCHEMA) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        db.execute("INSERT OR IGNORE INTO learning_meta VALUES(?1)", [SCHEMA])?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS observations(
            id INTEGER PRIMARY KEY, symbol TEXT NOT NULL, session TEXT NOT NULL,
            bucket INTEGER NOT NULL, observed_ms INTEGER NOT NULL,
            price REAL, price_ms INTEGER, eligible INTEGER NOT NULL,
            quality TEXT NOT NULL, context_json TEXT NOT NULL CHECK(json_valid(context_json)),
            UNIQUE(symbol,bucket));
            CREATE TABLE IF NOT EXISTS outcomes(
            observation_id INTEGER NOT NULL REFERENCES observations(id), horizon_ms INTEGER NOT NULL,
            target_ms INTEGER NOT NULL, status TEXT NOT NULL DEFAULT 'pending',
            outcome_ms INTEGER, outcome_price REAL, return_bps REAL,
            PRIMARY KEY(observation_id,horizon_ms));
            CREATE INDEX IF NOT EXISTS outcomes_pending ON outcomes(status,target_ms);
            CREATE INDEX IF NOT EXISTS observations_session ON observations(symbol,session,observed_ms);")?;
        Ok(Self(db))
    }

    pub fn record(&mut self, o: &Observation) -> rusqlite::Result<()> {
        self.record_interval(o, 30_000)
    }

    /// Separate streams can retain event changes without colliding in a 30s bucket.
    pub fn record_interval(&mut self, o: &Observation, interval_ms: u64) -> rusqlite::Result<()> {
        if !(1_000..=30_000).contains(&interval_ms) { return Err(rusqlite::Error::InvalidQuery); }
        if o.observed_ms > i64::MAX as u64 - 2_000_000
            || o.symbol.is_empty()
            || o.symbol.len() > 16
            || !o.symbol.chars().all(|c| c.is_ascii_uppercase() || c == '.')
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let now = o.observed_ms as i64;
        let date =
            snipers_core::calendar::date_at(o.observed_ms).ok_or(rusqlite::Error::InvalidQuery)?;
        let session = date.to_string();
        let rth = match snipers_core::calendar::schedule(date) {
            snipers_core::calendar::Day::Open(s) => Some((s.open_ms, s.close_ms)),
            _ => None,
        };
        let valid_price = o.price.zip(o.price_ms).filter(|(p, ts)| {
            p.is_finite()
                && *p > 0.0
                && *ts <= o.observed_ms
                && o.observed_ms - *ts <= 15_000
                && snipers_core::calendar::date_at(*ts) == Some(date)
                && rth.is_some_and(|(open, close)| *ts >= open && o.observed_ms < close)
        });
        let (price, price_ms) = valid_price
            .map(|(p, t)| (Some(p), Some(t as i64)))
            .unwrap_or((None, None));
        let context =
            serde_json::to_string(&o.context).map_err(|_| rusqlite::Error::InvalidQuery)?;
        if context.len() > 262_144 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let tx = self.0.transaction()?;
        // A late observation never backfills missed outcomes or changes earlier labels.
        tx.execute(
            "UPDATE outcomes SET status='missing' WHERE status='pending' AND target_ms+?1<?2",
            params![TOLERANCE_MS, now],
        )?;
        if let (Some(p), Some(t)) = (price, price_ms) {
            tx.execute("UPDATE outcomes SET status='observed',outcome_ms=?1,outcome_price=?2,
                return_bps=(?2/(SELECT price FROM observations WHERE id=observation_id)-1.0)*10000.0
                WHERE status='pending' AND target_ms<=?1 AND ?1<=target_ms+?3
                AND observation_id IN (SELECT id FROM observations WHERE symbol=?4 AND session=?5 AND price IS NOT NULL)",
                params![t,p,TOLERANCE_MS,o.symbol,session])?;
        }
        let inserted = tx.execute("INSERT OR IGNORE INTO observations(symbol,session,bucket,observed_ms,price,price_ms,eligible,quality,context_json)
            VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![o.symbol,session,now/(interval_ms as i64),now,price,price_ms,o.eligible && price.is_some(),o.quality,context])?;
        if inserted == 1 && price.is_some() {
            let id = tx.last_insert_rowid();
            for horizon in HORIZONS {
                tx.execute(
                    "INSERT INTO outcomes(observation_id,horizon_ms,target_ms) VALUES(?1,?2,?3)",
                    params![id, horizon, now + horizon],
                )?;
            }
        }
        tx.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn holiday_after_hours_and_future_ticks_cannot_label() {
        let mut s = LearningStore::open(Path::new(":memory:")).unwrap();
        for time in [
            "2026-09-24T22:00:00Z",
            "2026-09-26T15:00:00Z",
            "2026-11-27T19:00:00Z",
        ] {
            s.record(&observation(
                snipers_core::calendar::rfc3339_ms(time).unwrap(),
                100.,
            ))
            .unwrap();
        }
        let mut future = observation(start(), 100.);
        future.price_ms = Some(start() + 1);
        s.record(&future).unwrap();
        let n: i64 =
            s.0.query_row(
                "SELECT COUNT(*) FROM observations WHERE eligible=1 OR price IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
    }
    fn observation(now: u64, price: f64) -> Observation {
        Observation {
            symbol: "QQQ".into(),
            observed_ms: now,
            price: Some(price),
            price_ms: Some(now),
            eligible: true,
            quality: "READY".into(),
            context: serde_json::json!({"schema":1}),
        }
    }
    fn start() -> u64 {
        snipers_core::calendar::rfc3339_ms("2026-09-24T15:00:00Z").unwrap()
    }
    #[test]
    fn immutable_deduplicated_and_labels_only_after_target() {
        let mut s = LearningStore::open(Path::new(":memory:")).unwrap();
        let t = start();
        s.record(&observation(t, 100.)).unwrap();
        s.record(&observation(t + 1000, 110.)).unwrap();
        let price: f64 =
            s.0.query_row("SELECT price FROM observations", [], |r| r.get(0))
                .unwrap();
        assert_eq!(price, 100.);
        s.record(&observation(t + 299_000, 105.)).unwrap();
        let count: i64 =
            s.0.query_row(
                "SELECT COUNT(*) FROM outcomes WHERE status='observed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        s.record(&observation(t + 310_000, 102.)).unwrap();
        let bps: f64 =
            s.0.query_row(
                "SELECT return_bps FROM outcomes WHERE observation_id=1 AND horizon_ms=300000",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!((bps - 200.).abs() < 1e-8);
    }
    #[test]
    fn disconnect_and_other_symbol_never_fabricate_outcomes() {
        let mut s = LearningStore::open(Path::new(":memory:")).unwrap();
        let t = start();
        s.record(&observation(t, 100.)).unwrap();
        let mut other = observation(t + 300_000, 200.);
        other.symbol = "SPY".into();
        s.record(&other).unwrap();
        s.record(&observation(t + 361_000, 104.)).unwrap();
        let status: String =
            s.0.query_row(
                "SELECT status FROM outcomes WHERE observation_id=1 AND horizon_ms=300000",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "missing");
    }
    #[test]
    fn stale_prices_are_not_training_samples() {
        let mut s = LearningStore::open(Path::new(":memory:")).unwrap();
        let mut o = observation(start(), 100.);
        o.price_ms = Some(start() - 16_000);
        s.record(&o).unwrap();
        let row: (Option<f64>, bool) =
            s.0.query_row("SELECT price,eligible FROM observations", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(row, (None, false));
    }
    #[test]
    fn restart_preserves_pending_labels_and_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.sqlite");
        let t = start();
        {
            let mut s = LearningStore::open(&path).unwrap();
            s.record(&observation(t, 100.)).unwrap();
        }
        let mut s = LearningStore::open(&path).unwrap();
        s.record(&observation(t + 900_000, 99.)).unwrap();
        let status: String =
            s.0.query_row(
                "SELECT status FROM outcomes WHERE observation_id=1 AND horizon_ms=900000",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "observed");
    }
}

