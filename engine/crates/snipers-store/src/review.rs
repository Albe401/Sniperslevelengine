//! Persisted decision review while automatic admission remains disabled.
use crate::{Error, Ledger};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use snipers_core::{
    calendar,
    decision_prepare::{self, Evidence},
    session_guard,
};

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionReview {
    pub reviewed_ms: u64,
    pub symbol: String,
    pub model_side: Option<String>,
    pub prepared: bool,
    pub reason: String,
    pub entries_enabled: bool,
    pub active_trade_id: Option<i64>,
}
impl Ledger {
    /// One audit event per changed state, including after restart. Market-price
    /// changes alone do not produce another identical WAIT every UI publication.
    pub fn review_evidence(
        &mut self,
        symbol: &str,
        rows: &[Evidence],
        now: u64,
    ) -> Result<DecisionReview, Error> {
        self.review_evidence_mode(symbol, rows, now, false)
    }
    pub(crate) fn review_evidence_mode(
        &mut self,
        symbol: &str,
        rows: &[Evidence],
        now: u64,
        enabled: bool,
    ) -> Result<DecisionReview, Error> {
        let session = calendar::date_at(now).ok_or(Error::Timestamp)?.to_string();
        let timestamp = i64::try_from(now).map_err(|_| Error::Timestamp)?;
        let (selected, preparation_reason) = select_evidence(symbol, rows, now);
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active_trade_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM rust_paper_trades WHERE status='OPEN' LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let legacy_open = crate::legacy_positions::has_open(&tx)?;
        let daily: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM rust_paper_trades WHERE session=?1 UNION ALL SELECT 1 FROM rust_legacy_records WHERE table_name='paper_trades' AND session=?1)", [&session], |r| r.get(0))?;
        let guard = session_guard::evaluate(
            &session_guard::Input {
                symbol: symbol.into(),
                decision: selected.map_or("WAIT", |e| e.path.side.as_str()).into(),
                data_timestamp_ms: selected.map(|e| e.price_ms),
                decision_ready: selected.is_some(),
                allow_new_entries: true,
                active_trade_exists: Some(active_trade_id.is_some() || legacy_open),
                daily_trade_exists: Some(daily),
                ledger_session: Some(session.clone()),
                max_data_age_seconds: 15,
            },
            now,
        );
        let reason = match guard.reason.as_str() {
            "ENTRY_ALLOWED" if !enabled => "NEW_ENTRIES_DISABLED",
            "NO_ACTIONABLE_DECISION" => preparation_reason,
            other => other,
        }
        .to_string();
        let review = DecisionReview {
            reviewed_ms: now,
            symbol: symbol.into(),
            model_side: selected.map(|e| e.path.side.clone()),
            prepared: selected.is_some(),
            reason,
            entries_enabled: enabled,
            active_trade_id,
        };
        // Deliberately exclude the clock so the same condition is idempotent.
        let payload = serde_json::to_string(&serde_json::json!({"schema":1,"review":{
            "symbol":review.symbol,"modelSide":review.model_side,"prepared":review.prepared,
            "reason":review.reason,"entriesEnabled":review.entries_enabled,"activeTradeId":review.active_trade_id
        }}))
        .map_err(|_| Error::Schema)?;
        let previous: Option<String> = tx.query_row("SELECT payload_json FROM rust_decisions WHERE session=?1 AND symbol=?2 AND decision='WAIT_REVIEW' ORDER BY id DESC LIMIT 1", params![session,symbol], |r| r.get(0)).optional()?;
        if previous.as_deref() != Some(&payload) {
            tx.execute("INSERT INTO rust_decisions(timestamp_ms,session,symbol,decision,payload_json) VALUES(?1,?2,?3,'WAIT_REVIEW',?4)", params![timestamp,session,symbol,payload])?;
        }
        tx.commit()?;
        Ok(review)
    }
}

pub(crate) fn select_evidence<'a>(
    symbol: &str,
    rows: &'a [Evidence],
    now: u64,
) -> (Option<&'a Evidence>, &'static str) {
    let mut selected = None;
    let mut best_rr = f64::NEG_INFINITY;
    let mut preparation_reason = "NO_STRUCTURAL_EVIDENCE";
    let mut failure_priority = 0;
    for e in rows.iter().filter(|e| e.symbol == symbol) {
        match decision_prepare::prepare(e, now) {
            Ok(trade) => {
                let rr = (trade.target_1 - trade.entry).abs() / (trade.entry - trade.stop).abs();
                if rr > best_rr {
                    selected = Some(e);
                    best_rr = rr;
                }
            }
            Err(reason) if selected.is_none() => {
                let priority = if e.path.trigger_ready { 2 } else { 1 };
                if priority > failure_priority {
                    preparation_reason = reason;
                    failure_priority = priority;
                }
            }
            _ => {}
        }
    }
    (selected, preparation_reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wait_audit_survives_restart_without_duplicate_events_or_trades() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("review.sqlite");
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:02").unwrap();
        let mut ledger = Ledger::open(&path).unwrap();
        let r = ledger.review_evidence("QQQ", &[], now).unwrap();
        assert!(!r.entries_enabled);
        assert_eq!(r.reason, "NO_STRUCTURAL_EVIDENCE");
        drop(ledger);
        let mut ledger = Ledger::open(&path).unwrap();
        ledger.review_evidence("QQQ", &[], now + 1000).unwrap();
        let count: i64 = ledger
            .connection
            .query_row("SELECT COUNT(*) FROM rust_decisions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert!(ledger.load_trade(1).unwrap().is_none());
        let closed = calendar::legacy_eastern_ms("2026-09-24 16:00:02").unwrap();
        assert_eq!(
            ledger.review_evidence("QQQ", &[], closed).unwrap().reason,
            "OUTSIDE_RTH"
        );
        let count: i64 = ledger
            .connection
            .query_row("SELECT COUNT(*) FROM rust_decisions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }
}
