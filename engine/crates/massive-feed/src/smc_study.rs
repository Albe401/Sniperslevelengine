//! Offline underlying-price outcomes. Not an options backtest or a fill simulation.
use crate::route_4h::Minute;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub symbol: String,
    pub setup_id: String,
    pub side: i8,
    pub available_at: u64,
    pub entry_at: u64,
    pub entry: f64,
    pub anchor: f64,
    pub target: f64,
    pub deadline: u64,
}
pub fn measure(c: &Candidate, minutes: &[Minute], now: u64) -> Value {
    let invalid = c.setup_id.is_empty()
        || snipers_core::universe::stock_asset(&c.symbol).is_none()
        || !matches!(c.side, 1 | -1)
        || ![c.entry, c.anchor, c.target]
            .iter()
            .all(|p| p.is_finite() && *p > 0.)
        || c.available_at > c.entry_at
        || c.entry_at % 60000 != 0
        || c.deadline % 60000 != 0
        || c.deadline <= c.entry_at
        || c.deadline - c.entry_at > 6 * 3600000
        || (c.entry - c.anchor) * (c.side as f64) <= 0.
        || (c.target - c.entry) * (c.side as f64) <= 0.
        || minutes.len() > 2400;
    let pending = |state: &str| json!({"version":"smc-outcome-v1","symbol":c.symbol,"setupId":c.setup_id,"state":state,"basis":"underlying-price-not-options-pnl","profitability":null,"executionEnabled":false});
    if invalid {
        return pending("invalid");
    }
    let rows: BTreeMap<_, _> = minutes
        .iter()
        .filter(|b| {
            b.timestamp_ms >= c.entry_at
                && b.timestamp_ms < c.deadline
                && b.timestamp_ms.checked_add(60000).is_some_and(|t| t <= now)
        })
        .map(|b| (b.timestamp_ms, b))
        .collect();
    if rows.len()
        != minutes
            .iter()
            .filter(|b| {
                b.timestamp_ms >= c.entry_at
                    && b.timestamp_ms < c.deadline
                    && b.timestamp_ms.checked_add(60000).is_some_and(|t| t <= now)
            })
            .count()
    {
        return pending("duplicate-data");
    }
    let (mut favorable, mut adverse) = (0_f64, 0_f64);
    let mut terminal = None;
    let end = c.deadline.min(now / 60000 * 60000);
    for t in (c.entry_at..end).step_by(60000) {
        let Some(b) = rows.get(&t) else {
            return pending("incomplete-data");
        };
        if ![b.open, b.high, b.low, b.close, b.volume]
            .iter()
            .all(|v| v.is_finite())
            || b.low <= 0.
            || b.high < b.open.max(b.close)
            || b.low > b.open.min(b.close)
            || b.volume < 0.
        {
            return pending("invalid-data");
        }
        let date = snipers_core::calendar::date_at(t);
        if !date.is_some_and(|d|matches!(snipers_core::calendar::schedule(d),snipers_core::calendar::Day::Open(s)if t>=s.open_ms&&t<s.close_ms)){return pending("outside-rth");}
        favorable = favorable.max(if c.side == 1 {
            b.high - c.entry
        } else {
            c.entry - b.low
        });
        adverse = adverse.max(if c.side == 1 {
            c.entry - b.low
        } else {
            b.high - c.entry
        });
        let target = if c.side == 1 {
            b.high >= c.target
        } else {
            b.low <= c.target
        };
        let anchor = if c.side == 1 {
            b.low <= c.anchor
        } else {
            b.high >= c.anchor
        };
        if target || anchor {
            terminal = Some((
                if target && anchor {
                    "ambiguous-order"
                } else if target {
                    "target-observed"
                } else {
                    "anchor-observed"
                },
                t + 60000,
            ));
            break;
        }
    }
    if end <= c.entry_at {
        return pending("pending");
    }
    let (state, closed_at) = terminal.unwrap_or((
        if now >= c.deadline {
            "expired"
        } else {
            "pending"
        },
        end,
    ));
    json!({"version":"smc-outcome-v1","symbol":c.symbol,"setupId":c.setup_id,"state":state,"closedAt":closed_at,"mfeDollars":favorable,"maeDollars":adverse,"mfeR":favorable/(c.entry-c.anchor).abs(),"maeR":adverse/(c.entry-c.anchor).abs(),"basis":"underlying-price-not-options-pnl","profitability":null,"executionEnabled":false,"fillClaim":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Candidate, Vec<Minute>) {
        let at = snipers_core::calendar::rfc3339_ms("2026-10-06T10:00:00-04:00").unwrap();
        (
            Candidate {
                symbol: "QQQ".into(),
                setup_id: "smc".into(),
                side: 1,
                available_at: at,
                entry_at: at,
                entry: 100.,
                anchor: 98.,
                target: 104.,
                deadline: at + 120000,
            },
            vec![
                Minute {
                    timestamp_ms: at,
                    open: 100.,
                    high: 101.,
                    low: 99.,
                    close: 100.,
                    volume: 1.,
                },
                Minute {
                    timestamp_ms: at + 60000,
                    open: 100.,
                    high: 105.,
                    low: 99.,
                    close: 104.,
                    volume: 1.,
                },
            ],
        )
    }
    #[test]
    fn outcome_waits_for_future_closed_minutes() {
        let (c, rows) = fixture();
        assert_eq!(measure(&c, &rows, c.entry_at)["state"], "pending");
        assert_eq!(measure(&c, &rows, c.entry_at + 60000)["state"], "pending");
        assert_eq!(measure(&c, &rows, c.deadline)["state"], "target-observed");
        assert!(measure(&c, &rows, c.deadline)["profitability"].is_null());
    }
    #[test]
    fn unknown_touch_order_missing_and_duplicate_data_are_not_wins() {
        let (c, mut rows) = fixture();
        rows[1].low = 97.;
        assert_eq!(measure(&c, &rows, c.deadline)["state"], "ambiguous-order");
        assert_eq!(
            measure(&c, &rows[1..], c.deadline)["state"],
            "incomplete-data"
        );
        rows.push(rows[0].clone());
        assert_eq!(measure(&c, &rows, c.deadline)["state"], "duplicate-data");
    }
    #[test]
    fn future_signal_cannot_backdate_entry() {
        let (mut c, rows) = fixture();
        c.available_at = c.entry_at + 1;
        assert_eq!(measure(&c, &rows, c.deadline)["state"], "invalid");
    }
    #[test]
    fn outcomes_are_symmetric() {
        let (c, rows) = fixture();
        let mirror: Vec<_> = rows
            .iter()
            .map(|b| Minute {
                open: 220. - b.open,
                high: 220. - b.low,
                low: 220. - b.high,
                close: 220. - b.close,
                ..b.clone()
            })
            .collect();
        let put = Candidate {
            side: -1,
            entry: 220. - c.entry,
            anchor: 220. - c.anchor,
            target: 220. - c.target,
            ..c.clone()
        };
        let a = measure(&c, &rows, c.deadline);
        let z = measure(&put, &mirror, c.deadline);
        assert_eq!(a["state"], z["state"]);
        assert_eq!(a["mfeR"], z["mfeR"]);
        assert_eq!(a["maeR"], z["maeR"]);
    }
}
