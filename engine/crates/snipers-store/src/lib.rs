//! Versioned Rust paper ledger. Refuses automatic migration of legacy schemas.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use snipers_core::session_guard;
use std::{path::Path, time::Duration};
pub mod broker_orders;
pub mod company_research;
pub mod observation_log;
pub mod bot_learning;
mod coordinator;
mod journal;
pub mod learning;
pub mod learning_report;
pub mod neuro_report;
pub use coordinator::{Admission, Cycle, CycleInput};
mod legacy;
mod legacy_journal;
mod legacy_positions;
pub mod legacy_references;
pub use legacy_journal::{inspect_legacy, LegacyPage, LegacyPreview};
pub use legacy_positions::LegacyPosition;
mod paper_state;
pub use paper_state::{PositionUpdate, StoredPosition};
mod review;
pub use journal::{JournalEntry, JournalPage};
pub use legacy::LegacyImport;
pub use review::DecisionReview;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error de almacenamiento SQLite")]
    Sql(#[from] rusqlite::Error),
    #[error("Esquema desconocido; no se modifica la base existente")]
    Schema,
    #[error("Geometría de operación simulada inválida")]
    Geometry,
    #[error("Timestamp fuera de rango SQLite")]
    Timestamp,
    #[error("Selección de sesión inválida o distinta a la ya fijada")]
    Selection,
    #[error("No se pudo crear una copia nueva de la base")]
    Io(#[from] std::io::Error),
    #[error("Actualización de simulación rechazada")]
    Paper,
}
pub struct Ledger {
    connection: Connection,
}
fn copy_database(
    source: &Connection,
    target: &mut Connection,
    limit: Duration,
) -> Result<(), Error> {
    let started = std::time::Instant::now();
    let backup = rusqlite::backup::Backup::new(source, target)?;
    loop {
        if started.elapsed() >= limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "La copia no pudo completar dentro del plazo",
            )
            .into());
        }
        if matches!(backup.step(100)?, rusqlite::backup::StepResult::Done) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
/// Read-only source and exclusive new destination. Callers must only use the
/// destination after success; errors may leave an incomplete diagnostic copy.
pub fn backup_legacy(source: &Path, destination: &Path) -> Result<(), Error> {
    let source = Connection::open_with_flags(source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    source.busy_timeout(Duration::from_secs(2))?;
    source.pragma_update(None, "query_only", true)?;
    source.prepare("SELECT id FROM decisions LIMIT 0")?;
    source.prepare("SELECT id FROM paper_trades LIMIT 0")?;
    // A backup must never replace an existing user file or a prior backup.
    drop(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?,
    );
    let mut target = Connection::open(destination)?;
    copy_database(&source, &mut target, Duration::from_secs(30))?;
    let integrity: String = target.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        return Err(Error::Schema);
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct NewTrade {
    pub entry: f64,
    pub stop: f64,
    pub target_1: f64,
    pub target_2: Option<f64>,
}
const SCHEMA: &str = "
CREATE TABLE rust_decisions (
 id INTEGER PRIMARY KEY, timestamp_ms INTEGER NOT NULL,
 session TEXT NOT NULL, symbol TEXT NOT NULL, decision TEXT NOT NULL,
 payload_json TEXT NOT NULL CHECK(json_valid(payload_json))
);
CREATE TABLE rust_paper_trades (
 id INTEGER PRIMARY KEY, session TEXT NOT NULL UNIQUE,
 symbol TEXT NOT NULL CHECK(symbol='QQQ'), side TEXT NOT NULL CHECK(side IN ('CALL','PUT')),
 opened_ms INTEGER NOT NULL, closed_ms INTEGER,
 status TEXT NOT NULL CHECK(status IN ('OPEN','CLOSED')),
 entry_price REAL NOT NULL CHECK(entry_price>0), current_price REAL NOT NULL CHECK(current_price>0),
 stop_price REAL NOT NULL CHECK(stop_price>0), target_1 REAL NOT NULL CHECK(target_1>0), target_2 REAL,
 decision_id INTEGER NOT NULL REFERENCES rust_decisions(id)
);
CREATE UNIQUE INDEX rust_single_open_trade ON rust_paper_trades(status) WHERE status='OPEN';
CREATE TABLE rust_session_selection (session TEXT PRIMARY KEY, selected_json TEXT NOT NULL CHECK(json_valid(selected_json)), locked_ms INTEGER NOT NULL);
PRAGMA user_version=1;
CREATE TABLE rust_legacy_records (table_name TEXT NOT NULL, legacy_id INTEGER NOT NULL, session TEXT NOT NULL, timestamp_ms INTEGER NOT NULL, status TEXT, payload_json TEXT NOT NULL CHECK(json_valid(payload_json)), PRIMARY KEY(table_name,legacy_id));
PRAGMA application_id=1397639504;
";
impl Ledger {
    /// Preserve the current Rust database before an explicit archive import.
    pub fn backup_to(&self, destination: &Path) -> Result<(), Error> {
        drop(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?,
        );
        let mut target = Connection::open(destination)?;
        copy_database(&self.connection, &mut target, Duration::from_secs(30))?;
        let check: String = target.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if check != "ok" {
            return Err(Error::Schema);
        }
        Ok(())
    }
    /// Idempotent within a session. A second, different membership cannot replace
    /// the frozen list after a restart or from a concurrent window.
    pub fn save_selection(
        &mut self,
        session: &str,
        symbols: &[String],
        locked_ms: u64,
    ) -> Result<(), Error> {
        validate_selection(session, symbols, locked_ms)?;
        let encoded = serde_json::to_string(symbols).map_err(|_| Error::Selection)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: Option<(String, i64)> = transaction
            .query_row(
                "SELECT selected_json,locked_ms FROM rust_session_selection WHERE session=?1",
                [session],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let timestamp = i64::try_from(locked_ms).map_err(|_| Error::Timestamp)?;
        if let Some((old, old_time)) = previous {
            if old != encoded || old_time != timestamp {
                return Err(Error::Selection);
            }
        } else {
            transaction.execute("INSERT INTO rust_session_selection(session,selected_json,locked_ms) VALUES(?1,?2,?3)",params![session,encoded,timestamp])?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn load_selection(&self, session: &str) -> Result<Option<(Vec<String>, u64)>, Error> {
        let row: Option<(String, i64)> = self
            .connection
            .query_row(
                "SELECT selected_json,locked_ms FROM rust_session_selection WHERE session=?1",
                [session],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((encoded, time)) = row else {
            return Ok(None);
        };
        let time = u64::try_from(time).map_err(|_| Error::Timestamp)?;
        let symbols: Vec<String> = serde_json::from_str(&encoded).map_err(|_| Error::Selection)?;
        validate_selection(session, &symbols, time)?;
        Ok(Some((symbols, time)))
    }
    pub fn open(path: &Path) -> Result<Self, Error> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = transaction.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version == 0 {
            let count:i64=transaction.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0))?;
            if count != 0 {
                return Err(Error::Schema);
            }
            transaction.execute_batch(SCHEMA)?;
        } else if version != 1 {
            return Err(Error::Schema);
        }
        let application_id: i64 =
            transaction.pragma_query_value(None, "application_id", |r| r.get(0))?;
        if application_id != 1397639504 {
            return Err(Error::Schema);
        }
        // Validate required columns even when an unrelated database happens to use version 1.
        transaction.prepare("SELECT id,session,symbol,side,opened_ms,closed_ms,status,entry_price,current_price,stop_price,target_1,target_2,decision_id FROM rust_paper_trades LIMIT 0")?;
        transaction.prepare("SELECT id,timestamp_ms,session,symbol,decision,payload_json FROM rust_decisions LIMIT 0")?;
        transaction.prepare(
            "SELECT session,selected_json,locked_ms FROM rust_session_selection LIMIT 0",
        )?;
        transaction.prepare("SELECT table_name,legacy_id,session,timestamp_ms,status,payload_json FROM rust_legacy_records LIMIT 0")?;
        transaction.commit()?;
        Ok(Self { connection })
    }
    /// Admission and insert occur in one write transaction, so concurrent callers
    /// cannot both observe an empty daily ledger. No trade is opened on a guard denial.
    pub fn admit(
        &mut self,
        input: &session_guard::Input,
        trade: &NewTrade,
        now: u64,
    ) -> Result<session_guard::Result, Error> {
        self.admit_structural(input, trade, None, now)
    }
    pub fn admit_structural(
        &mut self,
        input: &session_guard::Input,
        trade: &NewTrade,
        structure: Option<snipers_core::paper::Structure>,
        now: u64,
    ) -> Result<session_guard::Result, Error> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut input = input.clone();
        input.ledger_session = snipers_core::calendar::date_at(now).map(|d| d.to_string());
        let active: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM rust_paper_trades WHERE status='OPEN')",
            [],
            |r| r.get(0),
        )?;
        let daily: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM rust_paper_trades WHERE session=?1 UNION ALL SELECT 1 FROM rust_legacy_records WHERE table_name='paper_trades' AND session=?1)",
            [&input.ledger_session],
            |r| r.get(0),
        )?;
        input.active_trade_exists = Some(active || legacy_positions::has_open(&transaction)?);
        input.daily_trade_exists = Some(daily);
        let result = session_guard::evaluate(&input, now);
        if !result.allowed {
            return Ok(result);
        }
        let positive = |p: f64| p.is_finite() && p > 0.0;
        let call = result.decision == "CALL";
        if ![trade.entry, trade.stop, trade.target_1]
            .into_iter()
            .all(positive)
            || trade.target_2.is_some_and(|p| !positive(p))
            || if call {
                trade.stop >= trade.entry
                    || trade.target_1 <= trade.entry
                    || trade.target_2.is_some_and(|p| p < trade.target_1)
            } else {
                trade.stop <= trade.entry
                    || trade.target_1 >= trade.entry
                    || trade.target_2.is_some_and(|p| p > trade.target_1)
            }
        {
            return Err(Error::Geometry);
        }
        let state = snipers_core::paper::Trade::new(
            if call {
                snipers_core::paper::Side::CALL
            } else {
                snipers_core::paper::Side::PUT
            },
            trade.entry,
            trade.stop,
            trade.target_1,
            trade.target_2,
            structure,
            now,
        )
        .map_err(|_| Error::Geometry)?;
        let payload =
            serde_json::to_string(&serde_json::json!({"schema":1,"guard":result,"trade":state}))
                .map_err(|_| Error::Geometry)?;
        let now_sql = i64::try_from(now).map_err(|_| Error::Timestamp)?;
        transaction.execute("INSERT INTO rust_decisions(timestamp_ms,session,symbol,decision,payload_json) VALUES (?1,?2,?3,?4,?5)",
            params![now_sql,result.session,result.symbol,result.decision,payload])?;
        let decision_id = transaction.last_insert_rowid();
        transaction.execute("INSERT INTO rust_paper_trades(session,symbol,side,opened_ms,status,entry_price,current_price,stop_price,target_1,target_2,decision_id) VALUES (?1,?2,?3,?4,'OPEN',?5,?5,?6,?7,?8,?9)",
            params![result.session,result.symbol,result.decision,now_sql,trade.entry,trade.stop,trade.target_1,trade.target_2,decision_id])?;
        transaction.commit()?;
        Ok(result)
    }
}

fn validate_selection(session: &str, symbols: &[String], locked_ms: u64) -> Result<(), Error> {
    use snipers_core::{
        calendar::{self, Day},
        universe,
    };
    let date = calendar::date_at(locked_ms).ok_or(Error::Selection)?;
    let Day::Open(day) = calendar::schedule(date) else {
        return Err(Error::Selection);
    };
    let mut seen = std::collections::BTreeSet::new();
    if day.date != session
        || locked_ms < day.open_ms + 900_000
        || locked_ms >= day.close_ms
        || symbols.is_empty()
        || symbols.len() > 5
        || symbols
            .iter()
            .any(|s| universe::stock_asset(s).is_none() || !seen.insert(s))
    {
        return Err(Error::Selection);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_lock_timeout_is_bounded_and_source_survives_retry() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("locked.sqlite");
        let writer = Connection::open(&path).unwrap();
        writer
            .execute_batch("CREATE TABLE retained(id INTEGER);INSERT INTO retained VALUES(7);")
            .unwrap();
        let source =
            Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        source.busy_timeout(Duration::from_millis(1)).unwrap();
        writer.execute_batch("BEGIN EXCLUSIVE;").unwrap();
        let mut target = Connection::open_in_memory().unwrap();
        assert!(
            matches!(copy_database(&source,&mut target,Duration::from_millis(30)),Err(Error::Io(error)) if error.kind()==std::io::ErrorKind::TimedOut)
        );
        writer.execute_batch("ROLLBACK;").unwrap();
        copy_database(&source, &mut target, Duration::from_secs(1)).unwrap();
        assert_eq!(
            target
                .query_row("SELECT id FROM retained", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            7
        );
        assert_eq!(
            source
                .query_row("SELECT id FROM retained", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            7
        );
    }
    pub(crate) fn input(now: u64) -> session_guard::Input {
        session_guard::Input {
            symbol: "QQQ".into(),
            decision: "CALL".into(),
            data_timestamp_ms: Some(now),
            decision_ready: true,
            allow_new_entries: true,
            active_trade_exists: None,
            daily_trade_exists: None,
            ledger_session: None,
            max_data_age_seconds: 120,
        }
    }
    #[test]
    fn restart_and_second_connection_cannot_open_another_daily_trade() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("paper.sqlite");
        let now = snipers_core::calendar::day_bounds("2026-09-24").unwrap().0 + 36_000_000;
        let trade = NewTrade {
            entry: 100.0,
            stop: 99.0,
            target_1: 101.0,
            target_2: Some(102.0),
        };
        let mut first = Ledger::open(&path).unwrap();
        let mut second = Ledger::open(&path).unwrap();
        assert!(first.admit(&input(now), &trade, now).unwrap().allowed);
        assert_eq!(
            second.admit(&input(now), &trade, now).unwrap().reason,
            "ACTIVE_TRADE_ALREADY_EXISTS"
        );
        // Simulate a completed position. Closing does not erase the daily reservation.
        first
            .connection
            .execute(
                "UPDATE rust_paper_trades SET status='CLOSED',closed_ms=?1",
                [i64::try_from(now + 1000).unwrap()],
            )
            .unwrap();
        drop(first);
        drop(second);
        let mut restored = Ledger::open(&path).unwrap();
        assert_eq!(
            restored.admit(&input(now), &trade, now).unwrap().reason,
            "DAILY_TRADE_LIMIT_REACHED"
        );
        assert_eq!(
            restored
                .connection
                .query_row("SELECT COUNT(*) FROM rust_paper_trades", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn bad_geometry_rolls_back_and_foreign_schema_is_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("paper.sqlite");
        let now = snipers_core::calendar::day_bounds("2026-09-24").unwrap().0 + 36_000_000;
        let mut ledger = Ledger::open(&path).unwrap();
        let trade = NewTrade {
            entry: 100.0,
            stop: 101.0,
            target_1: 99.0,
            target_2: None,
        };
        assert!(matches!(
            ledger.admit(&input(now), &trade, now),
            Err(Error::Geometry)
        ));
        assert_eq!(
            ledger
                .connection
                .query_row("SELECT COUNT(*) FROM rust_decisions", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        let old = temp.path().join("legacy.sqlite");
        let c = Connection::open(&old).unwrap();
        c.execute_batch("CREATE TABLE paper_trades(id INTEGER PRIMARY KEY); INSERT INTO paper_trades VALUES(42);").unwrap();
        assert!(matches!(Ledger::open(&old), Err(Error::Schema)));
        assert_eq!(
            c.query_row("SELECT id FROM paper_trades", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            42
        );
        assert_eq!(
            c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn simultaneous_admission_has_exactly_one_winner() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("race.sqlite");
        drop(Ledger::open(&path).unwrap());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let now = snipers_core::calendar::day_bounds("2026-09-24").unwrap().0 + 36_000_000;
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let barrier = barrier.clone();
                let path = path.clone();
                std::thread::spawn(move || {
                    let mut ledger = Ledger::open(&path).unwrap();
                    barrier.wait();
                    ledger
                        .admit(
                            &input(now),
                            &NewTrade {
                                entry: 100.0,
                                stop: 99.0,
                                target_1: 101.0,
                                target_2: None,
                            },
                            now,
                        )
                        .unwrap()
                        .allowed
                })
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .map(|h| usize::from(h.join().unwrap()))
                .sum::<usize>(),
            1
        );
    }
    #[test]
    fn selection_survives_restart_without_silent_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("selection.sqlite");
        let now = snipers_core::calendar::day_bounds("2026-09-24").unwrap().0 + 36_000_000;
        let symbols = vec!["QQQ".into(), "SPY".into()];
        let mut ledger = Ledger::open(&path).unwrap();
        ledger.save_selection("2026-09-24", &symbols, now).unwrap();
        ledger.save_selection("2026-09-24", &symbols, now).unwrap();
        assert!(ledger
            .save_selection("2026-09-24", &["SPY".into()], now)
            .is_err());
        drop(ledger);
        let mut restored = Ledger::open(&path).unwrap();
        assert_eq!(
            restored.load_selection("2026-09-24").unwrap(),
            Some((symbols, now))
        );
        assert_eq!(restored.load_selection("2026-09-25").unwrap(), None);
        assert!(restored
            .save_selection("2026-09-25", &["SPX".into()], now + 86_400_000)
            .is_err());
        assert!(restored
            .save_selection(
                "2026-09-25",
                &["QQQ".into(), "QQQ".into()],
                now + 86_400_000
            )
            .is_err());
    }
    #[test]
    fn legacy_backup_preserves_ids_and_never_overwrites_source_or_destination() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("legacy.sqlite");
        let destination = temp.path().join("backup.sqlite");
        let db = Connection::open(&source).unwrap();
        db.execute_batch("CREATE TABLE decisions(id INTEGER PRIMARY KEY); CREATE TABLE paper_trades(id INTEGER PRIMARY KEY); INSERT INTO decisions VALUES(71); INSERT INTO paper_trades VALUES(19);").unwrap();
        drop(db);
        let before = std::fs::read(&source).unwrap();
        backup_legacy(&source, &destination).unwrap();
        let copy =
            Connection::open_with_flags(&destination, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        assert_eq!(
            copy.query_row("SELECT id FROM decisions", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            71
        );
        assert_eq!(
            copy.query_row("SELECT id FROM paper_trades", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            19
        );
        assert_eq!(std::fs::read(&source).unwrap(), before);
        assert!(backup_legacy(&source, &destination).is_err());
        assert!(backup_legacy(&source, &source).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), before);
    }
}
pub mod news;
