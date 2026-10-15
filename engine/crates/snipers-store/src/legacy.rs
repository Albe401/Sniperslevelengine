use crate::{Error, Ledger};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::path::Path;
#[derive(Debug, serde::Serialize)]
pub struct LegacyImport {
    pub decisions: usize,
    pub paper_trades: usize,
}
impl Ledger {
    /// Import a frozen backup, retaining all original columns and IDs. A changed
    /// existing row aborts the whole import; source remains read-only.
    pub fn import_legacy(&mut self, backup_path: &Path) -> Result<LegacyImport, Error> {
        let mut source =
            Connection::open_with_flags(backup_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        source.pragma_update(None, "query_only", true)?;
        let source_tx = source.transaction()?;
        let target = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut report = LegacyImport {
            decisions: 0,
            paper_trades: 0,
        };
        for (table, query, time_key) in [
            (
                "decisions",
                "SELECT * FROM decisions ORDER BY id",
                "timestamp",
            ),
            (
                "paper_trades",
                "SELECT * FROM paper_trades ORDER BY id",
                "opened_at",
            ),
        ] {
            let mut stmt = source_tx.prepare(query)?;
            let columns: Vec<_> = stmt.column_names().iter().map(|s| s.to_string()).collect();
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let mut record = serde_json::Map::new();
                for (index, name) in columns.iter().enumerate() {
                    use rusqlite::types::ValueRef;
                    let value = match row.get_ref(index)? {
                        ValueRef::Null => serde_json::Value::Null,
                        ValueRef::Integer(v) => v.into(),
                        ValueRef::Real(v) => {
                            serde_json::Number::from_f64(v).ok_or(Error::Schema)?.into()
                        }
                        ValueRef::Text(v) => {
                            std::str::from_utf8(v).map_err(|_| Error::Schema)?.into()
                        }
                        ValueRef::Blob(_) => return Err(Error::Schema),
                    };
                    record.insert(name.clone(), value);
                }
                let id = record
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .filter(|v| *v > 0)
                    .ok_or(Error::Schema)?;
                let timestamp = record
                    .get(time_key)
                    .and_then(|v| v.as_str())
                    .and_then(snipers_core::calendar::legacy_eastern_ms)
                    .ok_or(Error::Timestamp)?;
                let session = snipers_core::calendar::date_at(timestamp)
                    .ok_or(Error::Timestamp)?
                    .to_string();
                let status = if table == "paper_trades" {
                    Some(
                        record
                            .get("status")
                            .and_then(|v| v.as_str())
                            .filter(|s| ["OPEN", "CLOSED"].contains(s))
                            .ok_or(Error::Schema)?
                            .to_string(),
                    )
                } else {
                    None
                };
                let payload = serde_json::to_string(&record).map_err(|_| Error::Schema)?;
                let previous:Option<String>=target.query_row("SELECT payload_json FROM rust_legacy_records WHERE table_name=?1 AND legacy_id=?2",params![table,id],|r|r.get(0)).optional()?;
                if let Some(old) = previous {
                    if old != payload {
                        return Err(Error::Schema);
                    }
                } else {
                    if table == "paper_trades" {
                        let conflict: bool = target.query_row(
                            "SELECT EXISTS(SELECT 1 FROM rust_paper_trades WHERE session=?1)",
                            [&session],
                            |r| r.get(0),
                        )?;
                        if conflict {
                            return Err(Error::Schema);
                        }
                    }
                    target.execute("INSERT INTO rust_legacy_records(table_name,legacy_id,session,timestamp_ms,status,payload_json) VALUES(?1,?2,?3,?4,?5,?6)",params![table,id,session,i64::try_from(timestamp).map_err(|_|Error::Timestamp)?,status,payload])?;
                }
                if table == "decisions" {
                    report.decisions += 1;
                } else {
                    report.paper_trades += 1;
                }
            }
        }
        target.commit()?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(path: &Path) {
        Connection::open(path).unwrap().execute_batch("CREATE TABLE decisions(id INTEGER PRIMARY KEY,timestamp TEXT,notes TEXT,spot REAL); CREATE TABLE paper_trades(id INTEGER PRIMARY KEY,opened_at TEXT,status TEXT,pnl_points REAL); INSERT INTO decisions VALUES(71,'2026-09-24 09:55:00','texto original',100.5); INSERT INTO paper_trades VALUES(19,'2026-09-24 09:56:00','CLOSED',NULL);").unwrap();
    }
    #[test]
    fn import_is_lossless_idempotent_and_legacy_trade_consumes_daily_limit() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("legacy.sqlite");
        fixture(&path);
        let mut ledger = Ledger::open(&temp.path().join("rust.sqlite")).unwrap();
        for _ in 0..2 {
            let imported = ledger.import_legacy(&path).unwrap();
            assert_eq!(imported.decisions, 1);
            assert_eq!(imported.paper_trades, 1);
        }
        let json:String=ledger.connection.query_row("SELECT payload_json FROM rust_legacy_records WHERE table_name='decisions' AND legacy_id=71",[],|r|r.get(0)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["notes"], "texto original");
        assert_eq!(value["timestamp"], "2026-09-24 09:55:00");
        assert_eq!(value["spot"], 100.5);
        let now = snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let result = ledger
            .admit(
                &crate::tests::input(now),
                &crate::NewTrade {
                    entry: 100.0,
                    stop: 99.0,
                    target_1: 101.0,
                    target_2: None,
                },
                now,
            )
            .unwrap();
        assert_eq!(result.reason, "DAILY_TRADE_LIMIT_REACHED");
    }
    #[test]
    fn conflict_rolls_back_new_rows_and_unknown_time_never_gets_guessed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("legacy.sqlite");
        fixture(&path);
        let mut ledger = Ledger::open(&temp.path().join("rust.sqlite")).unwrap();
        ledger.import_legacy(&path).unwrap();
        Connection::open(&path).unwrap().execute_batch("INSERT INTO decisions VALUES(70,'2026-09-24 09:54:00','nuevo',100); UPDATE decisions SET notes='modificado' WHERE id=71;").unwrap();
        assert!(ledger.import_legacy(&path).is_err());
        let count: i64 = ledger
            .connection
            .query_row("SELECT COUNT(*) FROM rust_legacy_records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        assert!(snipers_core::calendar::legacy_eastern_ms("2026-11-01 01:30:00").is_none());
        assert!(snipers_core::calendar::legacy_eastern_ms("2026-03-08 02:30:00").is_none());
        assert!(snipers_core::calendar::legacy_eastern_ms("texto inválido").is_none());
    }
}
