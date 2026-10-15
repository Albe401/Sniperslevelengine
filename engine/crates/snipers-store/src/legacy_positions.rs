//! Separate append-only continuation of imported positions; originals never change.
use crate::{Error, Ledger};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use snipers_core::paper::Trade;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyPosition {
    pub legacy_id: i64,
    pub symbol: String,
    pub transferred_ms: u64,
    pub trade: Trade,
}
fn originals(db: &Connection) -> Result<Vec<(i64, String, serde_json::Value)>, Error> {
    let mut query=db.prepare("SELECT legacy_id,session,payload_json FROM rust_legacy_records WHERE table_name='paper_trades' AND status='OPEN' ORDER BY legacy_id LIMIT 65")?;
    let rows = query
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    if rows.len() > 64 {
        return Err(Error::Schema);
    }
    rows.into_iter()
        .map(|(id, session, json)| {
            Ok((
                id,
                session,
                serde_json::from_str(&json).map_err(|_| Error::Schema)?,
            ))
        })
        .collect()
}
fn latest(
    db: &Connection,
    id: i64,
    original: &serde_json::Value,
) -> Result<Option<LegacyPosition>, Error> {
    let raw:Option<String>=db.query_row("SELECT payload_json FROM rust_decisions WHERE decision='LEGACY_PAPER_UPDATE' AND json_extract(payload_json,'$.position.legacyId')=?1 ORDER BY id DESC LIMIT 1",[id],|r|r.get(0)).optional()?;
    raw.map(|raw| {
        let value: serde_json::Value = serde_json::from_str(&raw).map_err(|_| Error::Schema)?;
        if value["schema"] != 1 {
            return Err(Error::Schema);
        }
        let position: LegacyPosition =
            serde_json::from_value(value["position"].clone()).map_err(|_| Error::Schema)?;
        if position.legacy_id != id
            || original["symbol"].as_str() != Some(position.symbol.as_str())
            || position.trade.opened_ms
                != original["opened_at"]
                    .as_str()
                    .and_then(snipers_core::calendar::legacy_eastern_ms)
                    .ok_or(Error::Timestamp)?
        {
            return Err(Error::Schema);
        }
        Ok(position)
    })
    .transpose()
}
pub(crate) fn has_open(db: &Connection) -> Result<bool, Error> {
    for (id, _, original) in originals(db)? {
        if latest(db, id, &original)?.is_none_or(|p| p.trade.closed_ms.is_none()) {
            return Ok(true);
        }
    }
    Ok(false)
}
fn append(
    db: &Connection,
    session: &str,
    position: &LegacyPosition,
    now: u64,
) -> Result<(), Error> {
    let payload = serde_json::to_string(&serde_json::json!({"schema":1,"position":position}))
        .map_err(|_| Error::Schema)?;
    db.execute("INSERT INTO rust_decisions(timestamp_ms,session,symbol,decision,payload_json) VALUES(?1,?2,?3,'LEGACY_PAPER_UPDATE',?4)",params![i64::try_from(now).map_err(|_|Error::Timestamp)?,session,position.symbol,payload])?;
    Ok(())
}
impl Ledger {
    /// Read-only transfer preview, also validates every source before connecting.
    pub fn preview_legacy_positions(&self, now: u64) -> Result<Vec<LegacyPosition>, Error> {
        let tx = self.connection.unchecked_transaction()?;
        let mut positions = Vec::new();
        for (id, _, original) in originals(&tx)? {
            let position = latest(&tx, id, &original)?.map(Ok).unwrap_or_else(|| {
                Ok::<_, Error>(LegacyPosition {
                    legacy_id: id,
                    symbol: original["symbol"].as_str().ok_or(Error::Schema)?.to_owned(),
                    transferred_ms: now,
                    trade: snipers_core::legacy_position::restore(&original, now)
                        .map_err(|_| Error::Geometry)?,
                })
            })?;
            if position.trade.closed_ms.is_none() {
                positions.push(position);
            }
        }
        tx.commit()?;
        Ok(positions)
    }
    /// Explicit transfer boundary, atomic across all imported OPEN positions.
    /// Call only after the operator has stopped management in the old program.
    pub fn resume_legacy_positions(&mut self, now: u64) -> Result<Vec<LegacyPosition>, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut positions = Vec::new();
        for (id, session, original) in originals(&tx)? {
            let position = if let Some(position) = latest(&tx, id, &original)? {
                position
            } else {
                let symbol = original["symbol"]
                    .as_str()
                    .filter(|s| {
                        !s.is_empty()
                            && s.len() <= 12
                            && s.bytes().all(|b| b.is_ascii_uppercase() || b == b'.')
                    })
                    .ok_or(Error::Schema)?
                    .to_owned();
                let position = LegacyPosition {
                    legacy_id: id,
                    symbol,
                    transferred_ms: now,
                    trade: snipers_core::legacy_position::restore(&original, now)
                        .map_err(|_| Error::Geometry)?,
                };
                append(&tx, &session, &position, now)?;
                position
            };
            if position.trade.closed_ms.is_none() {
                positions.push(position);
            }
        }
        tx.commit()?;
        Ok(positions)
    }
    pub fn legacy_position_states(&self) -> Result<Vec<LegacyPosition>, Error> {
        let tx = self.connection.unchecked_transaction()?;
        let mut positions = Vec::new();
        for (id, _, original) in originals(&tx)? {
            if let Some(position) = latest(&tx, id, &original)? {
                positions.push(position);
            }
        }
        tx.commit()?;
        Ok(positions)
    }
    pub fn manage_legacy_positions(
        &mut self,
        prices: &BTreeMap<String, (f64, u64)>,
        now: u64,
        verified: bool,
    ) -> Result<Vec<LegacyPosition>, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut positions = Vec::new();
        for (id, session, original) in originals(&tx)? {
            let Some(mut position) = latest(&tx, id, &original)? else {
                continue;
            };
            if position.trade.closed_ms.is_none() && verified {
                if let Some(&(price, event)) = prices.get(&position.symbol) {
                    match position.trade.observe(price, event, now, true) {
                        Ok(()) => append(&tx, &session, &position, event)?,
                        Err(
                            snipers_core::paper::Error::InvalidData
                            | snipers_core::paper::Error::OutOfOrder,
                        ) => {}
                        Err(_) => return Err(Error::Paper),
                    }
                }
            }
            positions.push(position);
        }
        tx.commit()?;
        Ok(positions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multiple_legacy_positions_resume_restart_close_independently_without_mutating_archive() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("rust.sqlite");
        let now = snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut ledger = Ledger::open(&path).unwrap();
        for (id, symbol) in [(12, "MSFT"), (15, "WMT")] {
            let record = serde_json::json!({"id":id,"symbol":symbol,"status":"OPEN","opened_at":"2026-09-23 10:00:00","side":"CALL","entry_price":100,"current_price":101,"stop_price":98,"initial_stop_price":98,"target_1":102,"target_2":105,"highest_price":103,"lowest_price":99,"mfe":3,"mae":-1,"target_1_hit":1,"target_2_hit":0,"structural_trailing_enabled":0,"trailing_stage":"OFF","vacuum_clear_count":0});
            ledger.connection.execute("INSERT INTO rust_legacy_records VALUES('paper_trades',?1,'2026-09-23',?2,'OPEN',?3)",params![id,i64::try_from(now-86400000).unwrap(),record.to_string()]).unwrap();
        }
        assert!(has_open(&ledger.connection).unwrap());
        assert!(ledger.legacy_position_states().unwrap().is_empty());
        assert_eq!(ledger.preview_legacy_positions(now).unwrap().len(), 2);
        assert!(ledger.legacy_position_states().unwrap().is_empty());
        // Failure in a later position rolls back the earlier restored one.
        ledger.connection.execute("UPDATE rust_legacy_records SET payload_json=json_set(payload_json,'$.highest_price',90) WHERE legacy_id=15",[]).unwrap();
        assert!(ledger.resume_legacy_positions(now).is_err());
        assert!(ledger.legacy_position_states().unwrap().is_empty());
        ledger.connection.execute("UPDATE rust_legacy_records SET payload_json=json_set(payload_json,'$.highest_price',103) WHERE legacy_id=15",[]).unwrap();
        assert_eq!(ledger.resume_legacy_positions(now).unwrap().len(), 2);
        drop(ledger);
        let mut ledger = Ledger::open(&path).unwrap();
        assert_eq!(
            ledger.resume_legacy_positions(now + 1).unwrap()[0].transferred_ms,
            now
        );
        let prices = BTreeMap::from([
            ("MSFT".into(), (106., now + 2)),
            ("QQQ".into(), (1., now + 2)),
        ]);
        assert!(ledger
            .manage_legacy_positions(&prices, now + 2, false)
            .unwrap()
            .iter()
            .all(|p| p.trade.closed_ms.is_none()));
        let result = ledger
            .manage_legacy_positions(&prices, now + 2, true)
            .unwrap();
        assert!(result[0].trade.closed_ms.is_some());
        assert!(result[1].trade.closed_ms.is_none());
        assert!(has_open(&ledger.connection).unwrap());
        let prices = BTreeMap::from([("WMT".into(), (97., now + 3))]);
        ledger
            .manage_legacy_positions(&prices, now + 3, true)
            .unwrap();
        assert!(!has_open(&ledger.connection).unwrap());
        let count: i64 = ledger
            .connection
            .query_row(
                "SELECT COUNT(*) FROM rust_decisions WHERE decision='LEGACY_PAPER_UPDATE'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        ledger
            .manage_legacy_positions(&prices, now + 3, true)
            .unwrap();
        assert_eq!(
            count,
            ledger
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM rust_decisions WHERE decision='LEGACY_PAPER_UPDATE'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap()
        );
        assert_eq!(ledger.legacy_journal(None, 25).unwrap().open_trades, 2);
        assert!(ledger.open_position_symbol().unwrap().is_none());
    }
}
