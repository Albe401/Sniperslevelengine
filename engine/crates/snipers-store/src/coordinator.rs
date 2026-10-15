//! Admission boundary for verified structural evidence, never an execution loop.
use crate::{Error, Ledger, NewTrade};
use snipers_core::{
    decision_prepare::{self, Evidence},
    session_guard,
};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Admission {
    pub admitted: bool,
    pub reason: String,
    pub decision: String,
}
pub struct CycleInput<'a> {
    pub symbol: &'a str,
    pub position_symbol: &'a str,
    pub position_price: Option<(f64, u64)>,
    pub source_verified: bool,
    pub evidence: &'a [Evidence],
    pub entries_enabled: bool,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cycle {
    #[serde(flatten)]
    pub review: crate::DecisionReview,
    pub position: crate::PositionUpdate,
    pub admission: Admission,
}
impl Ledger {
    /// Management always runs first, including with admissions disabled. Admission
    /// rechecks the ledger inside its own write transaction, so another process
    /// cannot open a second trade between this review and the final insert.
    pub fn coordinate(&mut self, input: &CycleInput<'_>, now: u64) -> Result<Cycle, Error> {
        let mut position = self.manage_open(
            input.position_symbol,
            input.position_price,
            now,
            input.source_verified,
        )?;
        let rows = if input.source_verified {
            input.evidence
        } else {
            &[]
        };
        let mut review =
            self.review_evidence_mode(input.symbol, rows, now, input.entries_enabled)?;
        let selected = crate::review::select_evidence(input.symbol, rows, now).0;
        let admission = if input.entries_enabled && review.reason == "ENTRY_ALLOWED" {
            match selected {
                Some(e) => self.admit_evidence(e, true, now)?,
                None => Admission {
                    admitted: false,
                    reason: "NO_STRUCTURAL_EVIDENCE".into(),
                    decision: "WAIT".into(),
                },
            }
        } else {
            Admission {
                admitted: false,
                reason: review.reason.clone(),
                decision: "WAIT".into(),
            }
        };
        if admission.admitted {
            position = self.manage_open(input.symbol, None, now, true)?;
            position.state = "POSITION_OPENED".into();
            review.active_trade_id = position.trade_id;
            review.reason = "PAPER_ENTRY_ADMITTED".into();
        } else if review.reason == "ENTRY_ALLOWED" {
            review.reason = admission.reason.clone();
        }
        Ok(Cycle {
            review,
            position,
            admission,
        })
    }
    /// Revalidate at the time of admission, then recover active/daily state under
    /// the same IMMEDIATE transaction that writes the decision and simulated trade.
    /// No frontend ready flag or previously prepared plan is accepted here.
    pub fn admit_evidence(
        &mut self,
        evidence: &Evidence,
        enabled: bool,
        now: u64,
    ) -> Result<Admission, Error> {
        let wait = |reason: &str| Admission {
            admitted: false,
            reason: reason.into(),
            decision: "WAIT".into(),
        };
        if !enabled {
            return Ok(wait("NEW_ENTRIES_DISABLED"));
        }
        let trade = match decision_prepare::prepare(evidence, now) {
            Ok(trade) => trade,
            Err(reason) => return Ok(wait(reason)),
        };
        let input = session_guard::Input {
            symbol: evidence.symbol.clone(),
            decision: evidence.path.side.clone(),
            data_timestamp_ms: Some(evidence.price_ms),
            decision_ready: true,
            allow_new_entries: enabled,
            active_trade_exists: None,
            daily_trade_exists: None,
            ledger_session: None,
            max_data_age_seconds: 15,
        };
        let order = NewTrade {
            entry: trade.entry,
            stop: trade.stop,
            target_1: trade.target_1,
            target_2: trade.target_2,
        };
        let result = self.admit_structural(&input, &order, trade.structure, now)?;
        Ok(Admission {
            admitted: result.allowed,
            reason: result.reason,
            decision: if result.allowed {
                result.decision
            } else {
                "WAIT".into()
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snipers_core::{bars::SessionBar, calendar, structural_path};
    fn evidence(now: u64) -> Evidence {
        Evidence {
            symbol: "QQQ".into(),
            source_symbol: "QQQ".into(),
            horizon: "DAY".into(),
            published_ms: now,
            verified: true,
            price: 500.2,
            price_ms: now,
            vwap: Some(500.0),
            bars: (1..=6)
                .rev()
                .map(|n| SessionBar {
                    timestamp: now - 2_000 - n * 120_000,
                    open: 500.1,
                    high: 500.25,
                    low: 499.99,
                    close: 500.2,
                    volume: 200.0,
                    minutes_received: 2,
                    vwap: Some(500.0),
                })
                .collect(),
            path: structural_path::Input {
                side: "CALL".into(),
                magnet: Some(500.0),
                wall: Some(505.0),
                wall_score: Some(60.0),
                vacuum_low: Some(501.0),
                vacuum_high: Some(503.0),
                vacuum_confirmed: true,
                flow_transport_ready: true,
                flow_side: "CALL".into(),
                buy_contracts: 5.0,
                rvol_ready: true,
                option_liquidity_ready: true,
                ..Default::default()
            },
        }
    }
    #[test]
    fn cycle_manages_before_admission_and_restart_cannot_duplicate_daily_entry() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("cycle.sqlite");
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:02").unwrap();
        let rows = [evidence(now)];
        let mut input = CycleInput {
            symbol: "QQQ",
            position_symbol: "QQQ",
            position_price: None,
            source_verified: true,
            evidence: &rows,
            entries_enabled: false,
        };
        let mut ledger = Ledger::open(&path).unwrap();
        let disabled = ledger.coordinate(&input, now).unwrap();
        assert!(!disabled.admission.admitted);
        assert!(!disabled.review.entries_enabled);
        assert!(ledger.open_position_symbol().unwrap().is_none());
        input.entries_enabled = true;
        input.source_verified = false;
        assert!(!ledger.coordinate(&input, now).unwrap().admission.admitted);
        input.source_verified = true;
        let opened = ledger.coordinate(&input, now).unwrap();
        assert!(opened.admission.admitted);
        assert_eq!(opened.position.state, "POSITION_OPENED");
        assert_eq!(opened.review.active_trade_id, opened.position.trade_id);
        drop(ledger);
        let mut ledger = Ledger::open(&path).unwrap();
        assert_eq!(
            ledger.coordinate(&input, now + 1).unwrap().review.reason,
            "ACTIVE_TRADE_ALREADY_EXISTS"
        );
        input.entries_enabled = false;
        input.position_price = Some((500.4, now + 1));
        let managed = ledger.coordinate(&input, now + 1).unwrap();
        assert_eq!(managed.position.state, "POSITION_UPDATED");
        assert!(!managed.admission.admitted && !managed.review.entries_enabled);
        input.entries_enabled = true;
        input.position_price = Some((505., now + 2));
        let closed = ledger.coordinate(&input, now + 2).unwrap();
        assert_eq!(closed.position.state, "POSITION_CLOSED");
        assert_eq!(closed.review.reason, "DAILY_TRADE_LIMIT_REACHED");
        assert!(!closed.admission.admitted);
        assert!(ledger.open_position_symbol().unwrap().is_none());
        assert_eq!(
            ledger
                .connection
                .query_row("SELECT COUNT(*) FROM rust_paper_trades", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn structural_evidence_admission_restart_and_daily_limit_are_atomic() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("coordinator.sqlite");
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:02").unwrap();
        let e = evidence(now);
        let mut ledger = Ledger::open(&path).unwrap();
        assert_eq!(
            ledger.admit_evidence(&e, false, now).unwrap().reason,
            "NEW_ENTRIES_DISABLED"
        );
        assert_eq!(
            ledger.admit_evidence(&e, true, now + 3001).unwrap().reason,
            "STALE_DECISION_EVIDENCE"
        );
        let mut invalid = e.clone();
        invalid.path.vacuum_low = Some(499.9);
        assert_eq!(
            ledger.admit_evidence(&invalid, true, now).unwrap().reason,
            "STRUCTURAL_PAPER_GEOMETRY"
        );
        assert!(ledger.load_trade(1).unwrap().is_none());
        let result = ledger.admit_evidence(&e, true, now).unwrap();
        assert!(result.admitted);
        assert_eq!(result.decision, "CALL");
        drop(ledger);
        let mut ledger = Ledger::open(&path).unwrap();
        let trade = ledger.load_trade(1).unwrap().unwrap();
        assert_eq!(trade.entry, 500.2);
        assert_eq!(trade.structure.unwrap().target_wall, 505.0);
        assert_eq!(
            ledger.admit_evidence(&e, true, now).unwrap().reason,
            "ACTIVE_TRADE_ALREADY_EXISTS"
        );
        assert!(ledger
            .advance(1, 505.0, now + 1, now + 1, true)
            .unwrap()
            .closed_ms
            .is_some());
        assert_eq!(
            ledger.admit_evidence(&e, true, now + 2).unwrap().reason,
            "DAILY_TRADE_LIMIT_REACHED"
        );
        let count: i64 = ledger
            .connection
            .query_row("SELECT COUNT(*) FROM rust_paper_trades", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
