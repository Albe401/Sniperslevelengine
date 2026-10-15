//! Empirical excursions, not target-hit probabilities. No target censoring or pooled symbols.
use crate::route_4h::{self, Minute, Scenario};
use serde::{Deserialize, Serialize};
use snipers_core::calendar::{self, Day};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub scenario: Scenario,
    pub horizon: String,
    pub deadline: u64,
    pub favorable_fraction: f64,
    pub adverse_fraction: f64,
    pub peak_regular_minutes: u64,
    pub invalidated: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Projection {
    pub horizon: String,
    pub state: String,
    pub n: usize,
    pub low: Option<f64>,
    pub typical: Option<f64>,
    pub high: Option<f64>,
    pub adverse: Option<f64>,
    pub peak_regular_minutes: Option<f64>,
    pub latest_deadline: Option<u64>,
    pub probability_claim: bool,
}
/// Linear empirical quantile (type 7); quartiles describe samples, not prediction coverage.
fn quantile(mut values: Vec<f64>, p: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    let index = (values.len() - 1) as f64 * p;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;
    values[lower] + (values[upper] - values[lower]) * index.fract()
}
pub fn project(s: &Scenario, library: &[Sample], horizon: &str) -> Projection {
    let risk = (s.origin - s.invalidation).abs() / s.origin;
    let mut candidates: Vec<_> = library
        .iter()
        .filter(|r| {
            let old = &r.scenario;
            let old_risk = (old.origin - old.invalidation).abs() / old.origin;
            old.symbol == s.symbol
                && old.setup == s.setup
                && old.structure == s.structure
                && old.side == s.side
                && old.block_minutes == s.block_minutes
                && r.horizon == horizon
                && old.at < r.deadline
                && r.deadline < s.at
                && old.origin.is_finite()
                && old.origin > 0.
                && risk.is_finite()
                && risk > 0.
                && old_risk >= risk * 0.5
                && old_risk <= risk * 2.
                && [r.favorable_fraction, r.adverse_fraction]
                    .iter()
                    .all(|v| v.is_finite() && *v >= 0.)
        })
        .collect();
    candidates.sort_by_key(|r| r.scenario.at);
    // Whole outcome windows must not overlap, even when loading multiple study files.
    let mut rows = Vec::new();
    let mut end = 0;
    for r in candidates {
        if r.scenario.at >= end {
            end = r.deadline;
            rows.push(r);
        }
    }
    let n = rows.len();
    let mut out = Projection {
        horizon: horizon.into(),
        state: "insufficient".into(),
        n,
        low: None,
        typical: None,
        high: None,
        adverse: None,
        peak_regular_minutes: None,
        latest_deadline: rows.iter().map(|r| r.deadline).max(),
        probability_claim: false,
    };
    // Product threshold, not a claim that twenty cases establish statistical validity.
    if n < 20 {
        return out;
    }
    let reach = |p| {
        s.origin
            + s.side as f64
                * s.origin
                * quantile(rows.iter().map(|r| r.favorable_fraction).collect(), p)
    };
    let low = reach(0.25);
    let typical = reach(0.5);
    let high = reach(0.75);
    if [low, typical, high]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.)
        || (typical - s.origin) * s.side as f64 <= 0.
    {
        return out;
    }
    out.state = "descriptive".into();
    out.low = Some(low);
    out.typical = Some(typical);
    out.high = Some(high);
    out.adverse = Some(s.origin * quantile(rows.iter().map(|r| r.adverse_fraction).collect(), 0.5));
    out.peak_regular_minutes = Some(quantile(
        rows.iter().map(|r| r.peak_regular_minutes as f64).collect(),
        0.5,
    ));
    out
}
fn measure(s: &Scenario, minutes: &[Minute], deadline: u64, now: u64) -> route_4h::Outcome {
    let mut uncapped = s.clone();
    uncapped.target = if s.side == 1 { f64::MAX } else { 0. };
    route_4h::resolve(&uncapped, minutes, deadline, now)
}
pub fn samples(symbol: &str, input: &[Minute], now: u64) -> Vec<Sample> {
    let (bars, _) = route_4h::aggregate(input, now);
    let mut minutes = input.to_vec();
    minutes.sort_by_key(|b| b.timestamp_ms);
    let mut result = Vec::new();
    let mut busy = BTreeMap::new();
    for i in 12..=bars.len() {
        let prefix = &bars[..i];
        let tail = prefix
            .iter()
            .rposition(|b| !b.complete)
            .map(|j| j + 1)
            .unwrap_or(0);
        if i - tail < 12 {
            continue;
        }
        let Some(s) = route_4h::detect(symbol, &prefix[tail..]).2 else {
            continue;
        };
        let Some(date) = calendar::date_at(s.at - 1) else {
            continue;
        };
        let Day::Open(session) = calendar::schedule(date) else {
            continue;
        };
        for (horizon, deadline) in [
            ("session", session.close_ms),
            ("swing2", route_4h::swing_deadline(s.at, 2).unwrap_or(s.at)),
        ] {
            if deadline <= s.at
                || deadline > now
                || busy.get(horizon).is_some_and(|end| *end > s.at)
            {
                continue;
            }
            // Disable target termination: measure the whole excursion until invalidation or horizon.
            let measured = measure(&s, &minutes, deadline, now);
            if measured.status == "data-gap" || measured.status == "pending" {
                continue;
            }
            let through = measured.resolved_at.unwrap_or(deadline);
            let mut regular = 0;
            let mut peak = 0;
            let mut best = 0_f64;
            for b in minutes
                .iter()
                .filter(|b| b.timestamp_ms >= s.at && b.timestamp_ms + 60000 <= through)
            {
                if route_4h::next_regular(b.timestamp_ms) != Some(b.timestamp_ms) {
                    continue;
                }
                regular += 1;
                let distance = if s.side == 1 {
                    b.high - s.origin
                } else {
                    s.origin - b.low
                };
                if distance > best {
                    best = distance;
                    peak = regular;
                }
            }
            busy.insert(horizon, deadline);
            result.push(Sample {
                scenario: s.clone(),
                horizon: horizon.into(),
                deadline,
                favorable_fraction: measured.favorable / s.origin,
                adverse_fraction: measured.adverse / s.origin,
                peak_regular_minutes: peak,
                invalidated: measured.status == "invalidated",
            });
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excursion_continues_past_old_target_but_stops_on_invalidation_and_rejects_gaps() {
        let mut s = signal();
        s.at = route_4h::next_regular(calendar::day_bounds("2026-10-05").unwrap().0).unwrap();
        s.target = 102.;
        let minutes: Vec<_> = [102., 104., 97., 110.]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Minute {
                timestamp_ms: s.at + i as u64 * 60000,
                open: p,
                high: p,
                low: p,
                close: p,
                volume: 1.,
            })
            .collect();
        let end = s.at + 240000;
        assert_eq!(route_4h::resolve(&s, &minutes, end, end).favorable, 2.);
        let result = measure(&s, &minutes, end, end);
        assert_eq!(result.favorable, 4.);
        assert_eq!(result.status, "invalidated");
        assert_eq!(result.resolved_at, Some(s.at + 180000));
        assert_eq!(measure(&s, &minutes[..1], end, end).status, "data-gap");
        assert_eq!(measure(&s, &minutes, end, s.at + 60000).status, "pending");
    }
    fn signal() -> Scenario {
        Scenario {
            id: "a".into(),
            symbol: "QQQ".into(),
            at: 100000,
            side: 1,
            setup: "breakout".into(),
            structure: "up".into(),
            origin: 100.,
            target: 110.,
            target_source: "range-projection".into(),
            invalidation: 98.,
            block_minutes: 240,
        }
    }
    fn library() -> Vec<Sample> {
        (0..20)
            .map(|i| {
                let mut s = signal();
                s.at = i * 1000;
                Sample {
                    scenario: s,
                    horizon: "swing2".into(),
                    deadline: i * 1000 + 500,
                    favorable_fraction: 0.01 + i as f64 * 0.001,
                    adverse_fraction: 0.005,
                    peak_regular_minutes: 30,
                    invalidated: i % 2 == 0,
                }
            })
            .collect()
    }
    #[test]
    fn quantiles_and_no_probability() {
        let p = project(&signal(), &library(), "swing2");
        assert_eq!(p.n, 20);
        assert_eq!(p.state, "descriptive");
        assert!((p.typical.unwrap() - 101.95).abs() < 1e-9);
        assert!(!p.probability_claim);
    }
    #[test]
    fn future_foreign_horizon_and_overlap_excluded() {
        let mut rows = library();
        rows[0].deadline = 100001;
        rows[1].scenario.symbol = "SPY".into();
        rows[2].horizon = "session".into();
        rows.push(rows[3].clone());
        let p = project(&signal(), &rows, "swing2");
        assert_eq!(p.n, 17);
        assert!(p.typical.is_none());
    }
    #[test]
    fn put_prices_are_ordered_directionally() {
        let mut s = signal();
        s.side = -1;
        s.invalidation = 102.;
        let mut rows = library();
        for r in &mut rows {
            r.scenario.side = -1;
            r.scenario.invalidation = 102.;
        }
        let p = project(&s, &rows, "swing2");
        assert!(p.low.unwrap() > p.typical.unwrap());
        assert!(p.typical.unwrap() > p.high.unwrap());
    }
    #[test]
    fn insufficient_and_different_structure_do_not_fallback() {
        let mut rows = library();
        rows[0].scenario.structure = "range".into();
        let p = project(&signal(), &rows, "swing2");
        assert_eq!(p.n, 19);
        assert_eq!(p.state, "insufficient");
        assert_eq!(quantile(vec![1., 2., 3., 4.], 0.5), 2.5);
    }
}
