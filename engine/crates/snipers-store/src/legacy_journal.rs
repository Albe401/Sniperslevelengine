//! Original legacy records remain immutable and visibly distinct from Rust trades.
use crate::{Error, Ledger};
use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyPreview {
    pub decisions: i64,
    pub trades: i64,
    pub open_trades: i64,
    pub closed_trades: i64,
}
pub fn inspect_legacy(path: &std::path::Path) -> Result<LegacyPreview, Error> {
    let mut db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.pragma_update(None, "query_only", true)?;
    db.busy_timeout(std::time::Duration::from_secs(2))?;
    let tx = db.transaction()?;
    let decisions = tx.query_row("SELECT COUNT(*) FROM decisions", [], |r| r.get(0))?;
    let (trades,open_trades,closed_trades):(i64,i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(status='OPEN'),0),COALESCE(SUM(status='CLOSED'),0) FROM paper_trades",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    if open_trades + closed_trades != trades {
        return Err(Error::Schema);
    }
    tx.prepare("SELECT id,opened_at,symbol,side,entry_price,current_price,stop_price,target_1,target_2,pnl_points FROM paper_trades LIMIT 0")?;
    tx.commit()?;
    Ok(LegacyPreview {
        decisions,
        trades,
        open_trades,
        closed_trades,
    })
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyEntry {
    pub id: i64,
    pub session: String,
    pub timestamp_ms: u64,
    pub status: String,
    pub record: Value,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyPage {
    pub source: &'static str,
    pub entries: Vec<LegacyEntry>,
    pub next_before_id: Option<i64>,
    pub total: i64,
    pub open_trades: i64,
    pub closed_trades: i64,
}
impl Ledger {
    pub fn legacy_journal(&self, before_id: Option<i64>, limit: u32) -> Result<LegacyPage, Error> {
        if before_id.is_some_and(|i| i <= 0) || !(1..=100).contains(&limit) {
            return Err(Error::Schema);
        }
        let tx = self.connection.unchecked_transaction()?;
        let (total,open_trades,closed_trades):(i64,i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(status='OPEN'),0),COALESCE(SUM(status='CLOSED'),0) FROM rust_legacy_records WHERE table_name='paper_trades'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        let mut entries = Vec::new();
        let mut more = false;
        {
            let mut query=tx.prepare("SELECT legacy_id,session,timestamp_ms,status,payload_json FROM rust_legacy_records WHERE table_name='paper_trades' AND (?1 IS NULL OR legacy_id<?1) ORDER BY legacy_id DESC LIMIT ?2")?;
            let mut rows = query.query(params![before_id, limit + 1])?;
            while let Some(row) = rows.next()? {
                if entries.len() == limit as usize {
                    more = true;
                    break;
                }
                let raw: String = row.get(4)?;
                let record: Value = serde_json::from_str(&raw).map_err(|_| Error::Schema)?;
                let id: i64 = row.get(0)?;
                let status: String = row.get(3)?;
                if record["id"].as_i64() != Some(id) || record["status"].as_str() != Some(&status) {
                    return Err(Error::Schema);
                }
                entries.push(LegacyEntry {
                    id,
                    session: row.get(1)?,
                    timestamp_ms: u64::try_from(row.get::<_, i64>(2)?)
                        .map_err(|_| Error::Timestamp)?,
                    status,
                    record,
                });
            }
        }
        let next_before_id = more.then(|| entries.last().unwrap().id);
        tx.commit()?;
        Ok(LegacyPage {
            source: "LEGACY_ARCHIVE",
            entries,
            next_before_id,
            total,
            open_trades,
            closed_trades,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_pages_preserve_open_positions_original_fields_and_ids() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.sqlite");
        let db = Connection::open(&source).unwrap();
        db.execute_batch("CREATE TABLE decisions(id INTEGER PRIMARY KEY,timestamp TEXT);CREATE TABLE paper_trades(id INTEGER PRIMARY KEY,opened_at TEXT,symbol TEXT,side TEXT,status TEXT,entry_price REAL,current_price REAL,stop_price REAL,target_1 REAL,target_2 REAL,pnl_points REAL,custom_note TEXT);INSERT INTO decisions VALUES(1,'2026-09-23 10:00:00');INSERT INTO paper_trades VALUES(12,'2026-09-18 11:16:23','MSFT','CALL','OPEN',493.5687,500.5301,490,500,510,NULL,'original');INSERT INTO paper_trades VALUES(15,'2026-09-22 12:53:55','WMT','CALL','OPEN',109.68,108.7286,104.2,110,111,NULL,'retained');INSERT INTO paper_trades VALUES(8,'2026-09-17 10:00:00','QQQ','PUT','CLOSED',500,498,502,498,496,2,'closed');").unwrap();
        drop(db);
        let preview = inspect_legacy(&source).unwrap();
        assert_eq!(preview.open_trades, 2);
        assert_eq!(preview.closed_trades, 1);
        let mut ledger = Ledger::open(&temp.path().join("rust.sqlite")).unwrap();
        ledger.import_legacy(&source).unwrap();
        let first = ledger.legacy_journal(None, 1).unwrap();
        assert_eq!(first.total, 3);
        assert_eq!(first.open_trades, 2);
        assert_eq!(first.entries[0].id, 15);
        assert_eq!(first.entries[0].record["custom_note"], "retained");
        assert!(first.entries[0].record["pnl_points"].is_null());
        let next = ledger.legacy_journal(first.next_before_id, 2).unwrap();
        assert_eq!(
            next.entries.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![12, 8]
        );
        assert!(next.next_before_id.is_none());
        assert!(ledger.legacy_journal(Some(0), 25).is_err());
        assert_eq!(inspect_legacy(&source).unwrap().open_trades, 2);
        let backup = temp.path().join("rust-backup.sqlite");
        ledger.backup_to(&backup).unwrap();
        let restored = Ledger::open(&backup).unwrap();
        assert_eq!(restored.legacy_journal(None, 25).unwrap().total, 3);
        assert!(ledger.backup_to(&backup).is_err());
        assert_eq!(restored.legacy_journal(None, 25).unwrap().open_trades, 2);
    }
}
