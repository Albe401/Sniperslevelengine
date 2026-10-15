//! Provisional pressure/price-response screen, not a calibrated probability or iceberg proof.
//! Ten-second candidates use a disjoint preceding twenty-second activity reference.
//! Midpoint excursion includes quotes outside each price bucket to avoid tautological holds.
use crate::execution_evidence::{Quote, Trade};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub version: String,
    pub status: String,
    pub reason: String,
    pub candidates: Vec<Candidate>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub price: f64,
    pub first_ms: u64,
    pub last_ms: u64,
    pub aggressive_side: String,
    pub absorbing_side: String,
    pub trades: usize,
    pub shares: f64,
    pub dominance_pct: f64,
    pub classified_pct: f64,
    pub activity_ratio: f64,
    pub displayed_size_ratio: f64,
    pub adverse_excursion: f64,
    pub allowed_excursion: f64,
    pub observation_ms: u64,
}
fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    xs[xs.len() / 2]
}
fn mid(q: &Quote) -> f64 {
    q.bid + (q.ask - q.bid) / 2.
}
pub fn evaluate(
    trades: &VecDeque<Trade>,
    quotes: &VecDeque<Quote>,
    now: u64,
    complete: bool,
) -> Read {
    let mut out = Read {
        version: "execution-absorption-v1".into(),
        status: "pending".into(),
        reason: "Needs uninterrupted thirty-second observation and fresh trades/NBBO".into(),
        ..Default::default()
    };
    if !complete || now < 30_000 {
        return out;
    }
    let start = now - 10_000;
    let reference = now - 30_000;
    let current: Vec<_> = trades
        .iter()
        .filter(|t| t.at >= start && t.at <= now)
        .collect();
    let prior: Vec<_> = trades
        .iter()
        .filter(|t| t.at >= reference && t.at < start && t.eligible)
        .collect();
    let qs: Vec<_> = quotes
        .iter()
        .filter(|q| q.at >= start && q.at <= now && q.valid && (q.ask - q.bid) / q.ask <= 0.01)
        .collect();
    if current.last().is_none_or(|t| now - t.at > 2000)
        || qs.last().is_none_or(|q| now - q.at > 2000)
        || qs.len() < 5
        || prior.len() < 10
    {
        return out;
    }
    let total: f64 = current.iter().map(|t| t.shares).sum();
    let classified: f64 = current
        .iter()
        .filter(|t| {
            t.eligible && t.quote.is_some() && t.depth_available && matches!(t.side, -1 | 1)
        })
        .map(|t| t.shares)
        .sum();
    if !total.is_finite() || total <= 0. || classified / total < 0.8 {
        out.reason =
            "Insufficient classified volume with visible-size coverage (minimum 80%)".into();
        return out;
    }
    let baseline = prior.iter().map(|t| t.shares).sum::<f64>() / 20.;
    if !baseline.is_finite() || baseline <= 0. {
        return out;
    }
    let spreads: Vec<_> = qs.iter().map(|q| q.ask - q.bid).collect();
    let prior_q: Vec<_> = quotes
        .iter()
        .filter(|q| q.at >= reference && q.at < start && q.valid && (q.ask - q.bid) / q.ask <= 0.01)
        .collect();
    if prior_q.len() < 5 {
        return out;
    }
    let increments: Vec<_> = prior_q
        .windows(2)
        .map(|w| (mid(w[1]) - mid(w[0])).abs())
        .collect();
    // Explicit heuristic floor plus spread/previous quote movement, not exchange tick size.
    let tolerance = median(spreads).max(median(increments) * 2.).max(0.01);
    let mut groups: BTreeMap<u64, Vec<&Trade>> = BTreeMap::new();
    for t in &current {
        groups
            .entry((t.price * 100. + 1e-8).floor() as u64)
            .or_default()
            .push(t);
    }
    for (key, ts) in groups {
        if ts.len() < 5 {
            continue;
        }
        let first = ts[0].at;
        let last = ts[ts.len() - 1].at;
        if last - first < 2000 || now - last > 2000 {
            continue;
        }
        let shares: f64 = ts.iter().map(|t| t.shares).sum();
        let valid: Vec<_> = ts
            .iter()
            .filter(|t| {
                t.eligible && t.quote.is_some() && t.depth_available && matches!(t.side, -1 | 1)
            })
            .collect();
        let covered: f64 = valid.iter().map(|t| t.shares).sum();
        if !shares.is_finite() || covered / shares < 0.8 {
            continue;
        }
        let buy: f64 = valid.iter().filter(|t| t.side == 1).map(|t| t.shares).sum();
        let sell = covered - buy;
        let side = if buy >= sell { 1 } else { -1 };
        let dominant = buy.max(sell);
        if dominant / covered < 0.7 {
            continue;
        }
        let pressure: Vec<_> = valid.iter().filter(|t| t.side == side).collect();
        if pressure.len() < 5 || pressure.last().unwrap().at - pressure[0].at < 2000 {
            continue;
        }
        let sizes: Vec<_> = valid
            .iter()
            .filter(|t| t.side == side)
            .filter_map(|t| {
                t.quote.as_ref().and_then(|q| {
                    if side == 1 {
                        q.ask_shares
                    } else {
                        q.bid_shares
                    }
                })
            })
            .collect();
        if sizes.is_empty() {
            continue;
        }
        let depth = median(sizes);
        let activity = dominant / 10. / baseline;
        if activity < 1.5 || dominant / depth < 3. {
            continue;
        }
        let first_q = ts[0].quote.as_ref();
        let Some(first_q) = first_q else {
            continue;
        };
        if quotes
            .iter()
            .any(|q| q.at >= first && q.at <= now && (!q.valid || (q.ask - q.bid) / q.ask > 0.01))
        {
            continue;
        }
        let origin = mid(first_q);
        let path: Vec<_> = qs.iter().filter(|q| q.at >= first).collect();
        if path.len() < 5
            || path.windows(2).any(|w| w[1].at - w[0].at > 2000)
            || path.first().is_some_and(|q| q.at - first > 2000)
        {
            continue;
        }
        let excursion = path
            .iter()
            .map(|q| (mid(q) - origin) * side as f64)
            .fold(0., f64::max);
        if excursion > tolerance + 1e-9 {
            continue;
        }
        out.candidates.push(Candidate {
            price: key as f64 / 100.,
            first_ms: first,
            last_ms: last,
            aggressive_side: if side == 1 { "buy" } else { "sell" }.into(),
            absorbing_side: if side == 1 { "sell" } else { "buy" }.into(),
            trades: ts.len(),
            shares,
            dominance_pct: dominant / covered * 100.,
            classified_pct: covered / shares * 100.,
            activity_ratio: activity,
            displayed_size_ratio: dominant / depth,
            adverse_excursion: excursion,
            allowed_excursion: tolerance,
            observation_ms: now - first,
        });
    }
    out.candidates.sort_by(|a, b| b.shares.total_cmp(&a.shares));
    out.candidates.truncate(10);
    out.status = if out.candidates.is_empty() {
        "observing"
    } else {
        "possible-absorption"
    }
    .into();
    out.reason="Provisional thresholds: >=5 same-side executions over >=2s, >=80% classified coverage, >=70% same-side pressure, >=1.5x preceding activity and >=3x displayed size; NBBO hold is not identified order replenishment".into();
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(side: i32, movement: f64) -> (VecDeque<Trade>, VecDeque<Quote>) {
        let mut ts = VecDeque::new();
        let mut qs = VecDeque::new();
        for i in 0..31 {
            let at = 1000 + i * 1000;
            let offset = if i >= 25 { movement } else { 0. };
            let q = Quote {
                at,
                bid: 100. + offset,
                ask: 100.02 + offset,
                bid_shares: Some(100.),
                ask_shares: Some(100.),
                valid: true,
            };
            qs.push_back(q.clone());
            ts.push_back(Trade {
                at,
                sequence: i + 1,
                price: if side == 1 { 100.02 } else { 100. },
                shares: if i >= 21 { 1000. } else { 100. },
                side,
                conditions: vec![0],
                eligible: true,
                quote: Some(q),
                quote_age_ms: Some(0),
                depth_available: true,
            });
        }
        (ts, qs)
    }
    #[test]
    fn both_sides_require_pressure_without_whole_path_continuation() {
        for side in [-1, 1] {
            let (t, q) = fixture(side, 0.);
            let r = evaluate(&t, &q, 31000, true);
            assert_eq!(r.candidates.len(), 1);
            assert_eq!(
                r.candidates[0].absorbing_side,
                if side == 1 { "sell" } else { "buy" }
            );
            let (t, q) = fixture(side, side as f64 * 0.2);
            assert!(evaluate(&t, &q, 31000, true).candidates.is_empty());
        }
    }
    #[test]
    fn coverage_disconnect_stale_and_future_are_not_signals() {
        let (mut t, q) = fixture(1, 0.);
        assert!(evaluate(&t, &q, 31000, false).candidates.is_empty());
        assert!(evaluate(&t, &q, 40000, true).candidates.is_empty());
        assert!(evaluate(&t, &q, 20000, true).candidates.is_empty());
        for x in &mut t {
            x.side = 0;
        }
        assert_eq!(evaluate(&t, &q, 31000, true).status, "pending");
    }
    #[test]
    fn missing_depth_ineligible_or_single_print_cannot_qualify() {
        for mode in 0..3 {
            let (mut t, q) = fixture(1, 0.);
            for x in &mut t {
                if x.at >= 21000 {
                    match mode {
                        0 => x.depth_available = false,
                        1 => x.eligible = false,
                        _ => x.price += x.sequence as f64 * 0.01,
                    }
                }
            }
            assert!(evaluate(&t, &q, 31000, true).candidates.is_empty());
        }
    }
    #[test]
    fn normal_activity_is_not_absorption() {
        let (mut t, q) = fixture(1, 0.);
        for x in &mut t {
            x.shares = 100.;
        }
        assert!(evaluate(&t, &q, 31000, true).candidates.is_empty());
    }
    #[test]
    fn invalid_or_sparse_quote_path_cannot_claim_a_hold() {
        let (t, mut q) = fixture(1, 0.);
        q.iter_mut().find(|q| q.at == 25000).unwrap().valid = false;
        assert!(evaluate(&t, &q, 31000, true).candidates.is_empty());
        let (t, mut q) = fixture(1, 0.);
        q.retain(|q| q.at < 25000 || q.at > 29000);
        assert!(evaluate(&t, &q, 31000, true).candidates.is_empty());
    }
    #[test]
    fn lost_hold_revokes_estimate_and_no_future_quote_can_restore_it() {
        let (t, mut q) = fixture(1, 0.);
        assert_eq!(evaluate(&t, &q, 31000, true).candidates.len(), 1);
        q.back_mut().unwrap().bid += 0.1;
        q.back_mut().unwrap().ask += 0.1;
        assert!(evaluate(&t, &q, 31000, true).candidates.is_empty());
        let mut future = q.back().unwrap().clone();
        future.at = 32000;
        future.bid = 100.;
        future.ask = 100.02;
        q.push_back(future);
        assert!(evaluate(&t, &q, 31000, true).candidates.is_empty());
    }
    #[test]
    fn a_single_dominant_print_with_small_ambiguous_prints_is_not_repetition() {
        let (mut t, q) = fixture(1, 0.);
        for x in &mut t {
            if x.at >= 21000 {
                x.side = 0;
                x.shares = 1.;
            }
        }
        t.back_mut().unwrap().side = 1;
        t.back_mut().unwrap().shares = 100000.;
        assert!(evaluate(&t, &q, 31000, true).candidates.is_empty());
    }
}
