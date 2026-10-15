use crate::{Error, Ledger};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use snipers_core::paper::Trade;
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionUpdate {
    pub state: String,
    pub trade_id: Option<i64>,
    pub trade: Option<Trade>,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredPosition {
    pub id: i64,
    pub symbol: String,
    pub trade: Trade,
}
pub(crate) fn decode(payload: &str) -> Result<Trade, Error> {
    let data: serde_json::Value = serde_json::from_str(payload).map_err(|_| Error::Schema)?;
    if data["schema"] != 1 {
        return Err(Error::Schema);
    }
    serde_json::from_value(data["trade"].clone()).map_err(|_| Error::Schema)
}
impl Ledger {
    /// Bounded recorded states for chart annotations, including completed trades.
    pub fn position_history(&self, symbol: &str, limit: u32) -> Result<Vec<StoredPosition>, Error> {
        if !(1..=100).contains(&limit) {
            return Err(Error::Schema);
        }
        let mut query=self.connection.prepare("SELECT t.id,t.symbol,d.payload_json FROM rust_paper_trades t JOIN rust_decisions d ON d.id=t.decision_id WHERE t.symbol=?1 ORDER BY t.id DESC LIMIT ?2")?;
        let rows = query
            .query_map(params![symbol, limit], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, symbol, payload)| {
                Ok(StoredPosition {
                    id,
                    symbol,
                    trade: decode(&payload)?,
                })
            })
            .collect()
    }
    /// Recover the single Rust-managed position without modifying legacy trades.
    pub fn open_position_symbol(&self) -> Result<Option<String>, Error> {
        let mut query = self
            .connection
            .prepare("SELECT symbol FROM rust_paper_trades WHERE status='OPEN' LIMIT 2")?;
        let rows = query
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.len() > 1 {
            return Err(Error::Schema);
        }
        Ok(rows.into_iter().next())
    }
    /// Update an existing position before evaluating new entries. Identity and
    /// replay checks are performed inside the same transaction as the update.
    pub fn manage_open(
        &mut self,
        symbol: &str,
        price: Option<(f64, u64)>,
        now: u64,
        verified: bool,
    ) -> Result<PositionUpdate, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row:Option<(i64,String,String)>=tx.query_row("SELECT t.id,t.symbol,d.payload_json FROM rust_paper_trades t JOIN rust_decisions d ON d.id=t.decision_id WHERE t.status='OPEN' LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        let Some((id, held_symbol, payload)) = row else {
            return Ok(PositionUpdate {
                state: "NO_POSITION".into(),
                trade_id: None,
                trade: None,
            });
        };
        let trade = decode(&payload)?;
        let mut result = PositionUpdate {
            state: "WAITING_PRICE".into(),
            trade_id: Some(id),
            trade: Some(trade.clone()),
        };
        if held_symbol != symbol {
            result.state = "POSITION_OTHER_SYMBOL".into();
            return Ok(result);
        }
        if !verified {
            result.state = "SOURCE_DISCONNECTED".into();
            return Ok(result);
        }
        let Some((price, event)) = price else {
            return Ok(result);
        };
        if !price.is_finite()
            || price <= 0.0
            || event > now
            || now - event > 15_000
            || snipers_core::calendar::date_at(event) != snipers_core::calendar::date_at(now)
        {
            result.state = "PRICE_NOT_CURRENT".into();
            return Ok(result);
        }
        if event < trade.opened_ms || trade.last_ms.is_some_and(|t| event <= t) {
            result.state = "POSITION_UNCHANGED".into();
            return Ok(result);
        }
        let trade = advance_in(&tx, id, price, event, now, verified)?;
        result.state = if trade.closed_ms.is_some() {
            "POSITION_CLOSED"
        } else {
            "POSITION_UPDATED"
        }
        .into();
        result.trade = Some(trade);
        tx.commit()?;
        Ok(result)
    }
    pub fn load_trade(&self, id: i64) -> Result<Option<Trade>, Error> {
        let payload:Option<String>=self.connection.query_row("SELECT d.payload_json FROM rust_paper_trades t JOIN rust_decisions d ON d.id=t.decision_id WHERE t.id=?1",[id],|r|r.get(0)).optional()?;
        payload.as_deref().map(decode).transpose()
    }
    /// Append the state event and update the current row in a single transaction.
    /// On rejection neither the audit trail nor the current position changes.
    pub fn advance(
        &mut self,
        id: i64,
        price: f64,
        event_ms: u64,
        now: u64,
        verified: bool,
    ) -> Result<Trade, Error> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let trade = advance_in(&transaction, id, price, event_ms, now, verified)?;
        transaction.commit()?;
        Ok(trade)
    }
}
fn advance_in(
    transaction: &rusqlite::Transaction<'_>,
    id: i64,
    price: f64,
    event_ms: u64,
    now: u64,
    verified: bool,
) -> Result<Trade, Error> {
    let (payload,previous,session,symbol):(String,i64,String,String)=transaction.query_row("SELECT d.payload_json,d.id,t.session,t.symbol FROM rust_paper_trades t JOIN rust_decisions d ON d.id=t.decision_id WHERE t.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    let mut trade = decode(&payload)?;
    trade
        .observe(price, event_ms, now, verified)
        .map_err(|_| Error::Paper)?;
    let payload = serde_json::to_string(
        &serde_json::json!({"schema":1,"trade":trade,"previousDecisionId":previous}),
    )
    .map_err(|_| Error::Schema)?;
    let event = i64::try_from(event_ms).map_err(|_| Error::Timestamp)?;
    transaction.execute("INSERT INTO rust_decisions(timestamp_ms,session,symbol,decision,payload_json) VALUES(?1,?2,?3,'PAPER_UPDATE',?4)",params![event,session,symbol,payload])?;
    let decision = transaction.last_insert_rowid();
    let closed = trade
        .closed_ms
        .map(i64::try_from)
        .transpose()
        .map_err(|_| Error::Timestamp)?;
    let status = if closed.is_some() { "CLOSED" } else { "OPEN" };
    transaction.execute("UPDATE rust_paper_trades SET current_price=?1,stop_price=?2,status=?3,closed_ms=?4,decision_id=?5 WHERE id=?6",params![trade.current,trade.stop,status,closed,decision,id])?;
    Ok(trade)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manager_updates_before_review_and_restores_overnight_without_duplicate_ticks() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("managed.sqlite");
        let now = snipers_core::calendar::legacy_eastern_ms("2026-09-24 15:59:00").unwrap();
        let mut ledger = Ledger::open(&path).unwrap();
        assert_eq!(ledger.open_position_symbol().unwrap(), None);
        let order = crate::NewTrade {
            entry: 100.0,
            stop: 98.0,
            target_1: 105.0,
            target_2: Some(110.0),
        };
        ledger
            .admit(&crate::tests::input(now), &order, now)
            .unwrap();
        assert_eq!(
            ledger.open_position_symbol().unwrap().as_deref(),
            Some("QQQ")
        );
        for (symbol, price, clock, verified, expected) in [
            (
                "SPY",
                Some((101.0, now)),
                now,
                true,
                "POSITION_OTHER_SYMBOL",
            ),
            ("QQQ", Some((101.0, now)), now, false, "SOURCE_DISCONNECTED"),
            ("QQQ", None, now, true, "WAITING_PRICE"),
            (
                "QQQ",
                Some((101.0, now)),
                now + 15001,
                true,
                "PRICE_NOT_CURRENT",
            ),
            (
                "QQQ",
                Some((101.0, now + 1)),
                now,
                true,
                "PRICE_NOT_CURRENT",
            ),
        ] {
            assert_eq!(
                ledger
                    .manage_open(symbol, price, clock, verified)
                    .unwrap()
                    .state,
                expected
            );
            assert_eq!(ledger.load_trade(1).unwrap().unwrap().current, 100.0);
        }
        assert_eq!(
            ledger
                .manage_open("QQQ", Some((101.0, now)), now, true)
                .unwrap()
                .state,
            "POSITION_UPDATED"
        );
        assert_eq!(
            ledger
                .manage_open("QQQ", Some((102.0, now)), now, true)
                .unwrap()
                .state,
            "POSITION_UNCHANGED"
        );
        drop(ledger);
        let mut ledger = Ledger::open(&path).unwrap();
        let tomorrow = snipers_core::calendar::legacy_eastern_ms("2026-09-25 09:30:01").unwrap();
        assert_eq!(
            ledger
                .manage_open("QQQ", Some((110.0, tomorrow)), tomorrow, true)
                .unwrap()
                .state,
            "POSITION_CLOSED"
        );
        assert_eq!(ledger.load_trade(1).unwrap().unwrap().pnl_points, 10.0);
        assert_eq!(ledger.open_position_symbol().unwrap(), None);
        assert!(ledger
            .review_evidence("QQQ", &[], tomorrow)
            .unwrap()
            .active_trade_id
            .is_none());
        assert_eq!(
            ledger
                .manage_open("QQQ", Some((111.0, tomorrow + 1)), tomorrow + 1, true)
                .unwrap()
                .state,
            "NO_POSITION"
        );
        let count: i64 = ledger
            .connection
            .query_row(
                "SELECT COUNT(*) FROM rust_decisions WHERE decision='PAPER_UPDATE'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }
    #[test]
    fn restart_restores_trailing_and_replay_cannot_advance_or_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("paper.sqlite");
        let now = snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut ledger = Ledger::open(&path).unwrap();
        let input = crate::tests::input(now);
        let order = crate::NewTrade {
            entry: 100.0,
            stop: 98.0,
            target_1: 105.0,
            target_2: Some(110.0),
        };
        ledger
            .admit_structural(
                &input,
                &order,
                Some(snipers_core::paper::Structure {
                    magnet: 101.0,
                    vacuum_low: 102.0,
                    vacuum_high: 104.0,
                    target_wall: 109.0,
                }),
                now,
            )
            .unwrap();
        ledger.advance(1, 104.0, now, now, true).unwrap();
        drop(ledger);
        let mut ledger = Ledger::open(&path).unwrap();
        assert_eq!(ledger.load_trade(1).unwrap().unwrap().clear_count, 1);
        assert!(ledger.advance(1, 104.0, now, now, true).is_err());
        assert_eq!(
            ledger
                .connection
                .query_row("SELECT COUNT(*) FROM rust_decisions", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
        let next = ledger.advance(1, 104.0, now + 1, now + 1, true).unwrap();
        assert_eq!(next.stop, 102.0);
        let closed = ledger.advance(1, 101.0, now + 2, now + 2, true).unwrap();
        assert_eq!(closed.pnl_points, 1.0);
        assert!(closed.closed_ms.is_some());
        drop(ledger);
        let mut ledger = Ledger::open(&path).unwrap();
        assert_eq!(ledger.load_trade(1).unwrap().unwrap().pnl_points, 1.0);
        let history = ledger.position_history("QQQ", 20).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].trade.closed_ms, Some(now + 2));
        assert!(ledger.position_history("SPY", 20).unwrap().is_empty());
        assert!(ledger.position_history("QQQ", 0).is_err());
        assert_eq!(
            ledger.admit(&input, &order, now).unwrap().reason,
            "DAILY_TRADE_LIMIT_REACHED"
        );
    }
}
