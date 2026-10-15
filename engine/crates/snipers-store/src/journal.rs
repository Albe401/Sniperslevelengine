//! Read models for the Rust simulation. Legacy archives remain a separate source.
use crate::{paper_state::decode, Error, Ledger};
use rusqlite::params;
use serde::Serialize;
use snipers_core::{
    paper::Trade,
    paper_performance::{Accumulator, ClosedTrade, Performance},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    pub id: i64,
    pub session: String,
    pub symbol: String,
    pub trade: Trade,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalPage {
    pub source: &'static str,
    pub entries: Vec<JournalEntry>,
    pub next_before_id: Option<i64>,
}
impl Ledger {
    /// One read transaction keeps totals and the displayed page mutually consistent.
    pub fn journal_report(
        &self,
        before_id: Option<i64>,
        limit: u32,
    ) -> Result<serde_json::Value, Error> {
        let transaction = self.connection.unchecked_transaction()?;
        let page = self.journal(before_id, limit)?;
        let performance = self.paper_performance()?;
        let legacy_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM rust_legacy_records WHERE table_name='paper_trades'",
            [],
            |r| r.get(0),
        )?;
        let report = serde_json::json!({"page":page,"performance":performance,"legacyArchivedTrades":legacy_count,"entriesEnabled":false});
        transaction.commit()?;
        Ok(report)
    }
    /// Stable descending ID cursor; a new admission cannot shift later pages.
    /// At most 100 positions are decoded, plus one lookahead record.
    pub fn journal(&self, before_id: Option<i64>, limit: u32) -> Result<JournalPage, Error> {
        if before_id.is_some_and(|v| v <= 0) || !(1..=100).contains(&limit) {
            return Err(Error::Schema);
        }
        let mut query = self.connection.prepare("SELECT t.id,t.session,t.symbol,d.payload_json FROM rust_paper_trades t JOIN rust_decisions d ON d.id=t.decision_id WHERE (?1 IS NULL OR t.id<?1) ORDER BY t.id DESC LIMIT ?2")?;
        let mut rows = query.query(params![before_id, limit + 1])?;
        let mut entries = Vec::new();
        let mut more = false;
        while let Some(row) = rows.next()? {
            if entries.len() == limit as usize {
                more = true;
                break;
            }
            entries.push(JournalEntry {
                id: row.get(0)?,
                session: row.get(1)?,
                symbol: row.get(2)?,
                trade: decode(&row.get::<_, String>(3)?)?,
            });
        }
        let next_before_id = if more {
            entries.last().map(|v| v.id)
        } else {
            None
        };
        Ok(JournalPage {
            source: "RUST_SIMULATION",
            entries,
            next_before_id,
        })
    }
    /// Stream one latest state per closed position, never count PAPER_UPDATE events.
    /// Corrupt/unknown records fail the report rather than silently improving statistics.
    pub fn paper_performance(&self) -> Result<Performance, Error> {
        let mut query = self.connection.prepare("SELECT d.payload_json FROM rust_paper_trades t JOIN rust_decisions d ON d.id=t.decision_id WHERE t.status='CLOSED' ORDER BY t.id")?;
        let mut rows = query.query([])?;
        let mut accumulator = Accumulator::default();
        while let Some(row) = rows.next()? {
            let trade = decode(&row.get::<_, String>(0)?)?;
            if trade.closed_ms.is_none() {
                return Err(Error::Schema);
            }
            accumulator
                .push(&ClosedTrade {
                    pnl_points: trade.pnl_points,
                    mfe: trade.mfe,
                    mae: trade.mae,
                    target_1_hit: trade.target_1_hit,
                    target_2_hit: trade.target_2_hit,
                    stop_hit: trade.stop_hit,
                })
                .map_err(|_| Error::Paper)?;
        }
        Ok(accumulator.finish())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn journal_cursor_survives_new_trade_and_stats_count_closed_positions_once() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("journal.sqlite");
        let mut ledger = Ledger::open(&path).unwrap();
        let order = crate::NewTrade {
            entry: 100.0,
            stop: 98.0,
            target_1: 102.0,
            target_2: Some(104.0),
        };
        for (i, day) in ["2026-09-21", "2026-09-22"].iter().enumerate() {
            let now =
                snipers_core::calendar::legacy_eastern_ms(&format!("{day} 10:00:00")).unwrap();
            ledger
                .admit(&crate::tests::input(now), &order, now)
                .unwrap();
            ledger
                .advance(i as i64 + 1, 101.0, now + 1, now + 1, true)
                .unwrap();
            ledger
                .advance(
                    i as i64 + 1,
                    if i == 0 { 104.0 } else { 98.0 },
                    now + 2,
                    now + 2,
                    true,
                )
                .unwrap();
        }
        let first = ledger.journal(None, 1).unwrap();
        assert_eq!(first.entries[0].id, 2);
        let now = snipers_core::calendar::legacy_eastern_ms("2026-09-23 10:00:00").unwrap();
        ledger
            .admit(&crate::tests::input(now), &order, now)
            .unwrap();
        drop(ledger);
        let ledger = Ledger::open(&path).unwrap();
        let second = ledger.journal(first.next_before_id, 1).unwrap();
        assert_eq!(second.entries[0].id, 1);
        assert!(second.next_before_id.is_none());
        assert_eq!(ledger.journal(None, 100).unwrap().entries.len(), 3);
        assert!(ledger.journal(None, 101).is_err());
        let report = ledger.paper_performance().unwrap();
        assert_eq!(report.total_trades, 2);
        assert_eq!(report.wins, 1);
        assert_eq!(report.losses, 1);
        assert_eq!(report.win_rate, 50.0);
        assert_eq!(report.average_pnl_points, 1.0);
        assert_eq!(report.target_2_hit_rate, 50.0);
        assert_eq!(report.stop_rate, 50.0);
        let combined = ledger.journal_report(None, 25).unwrap();
        assert_eq!(combined["performance"]["total_trades"], 2);
        assert_eq!(combined["page"]["entries"].as_array().unwrap().len(), 3);
        assert_eq!(combined["entriesEnabled"], false);
        ledger.connection.execute("UPDATE rust_decisions SET payload_json='{}' WHERE id=(SELECT decision_id FROM rust_paper_trades WHERE id=1)", []).unwrap();
        assert!(ledger.paper_performance().is_err());
        assert!(ledger.journal(None, 100).is_err());
    }
}
