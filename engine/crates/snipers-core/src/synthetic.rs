//! Legacy approximations with explicit provenance. These are not index ticks.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum Side {
    Call,
    Put,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Quote {
    pub root: String,
    pub expiration: String,
    pub side: Side,
    pub strike: f64,
    pub bid: f64,
    pub ask: f64,
    pub quote_ms: u64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pair {
    pub strike: f64,
    pub call_mid: f64,
    pub put_mid: f64,
    pub combined_spread: f64,
    pub relative_spread: f64,
    pub synthetic: f64,
    pub quality_score: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    pub source: &'static str,
    pub approximate: bool,
    pub usable_for_analysis: bool,
    pub expiration: String,
    pub root: String,
    pub price: f64,
    pub candidate_pairs: usize,
    pub pairs: Vec<Pair>,
    pub oldest_quote_ms: u64,
}
fn fresh(ms: u64, now: u64) -> bool {
    ms <= now
        && now - ms <= 15_000
        && crate::calendar::date_at(now).is_some()
        && crate::calendar::date_at(ms) == crate::calendar::date_at(now)
}
fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        values[middle - 1] / 2.0 + values[middle] / 2.0
    } else {
        values[middle]
    }
}
/// Same option root AND expiration are mandatory: SPX AM and SPXW PM are never
/// paired just because they share a calendar date and strike.
pub fn spx(
    root: &str,
    expiration: &str,
    quotes: &[Quote],
    now: u64,
    verified: bool,
) -> Result<Estimate, &'static str> {
    if !verified {
        return Err("UNVERIFIED_SOURCE");
    }
    if !["SPX", "SPXW"].contains(&root) || quotes.is_empty() || quotes.len() > 10_000 {
        return Err("INVALID_UNIVERSE");
    }
    let today = crate::calendar::date_at(now).ok_or("INVALID_DATE")?;
    let expiry = crate::calendar::day_bounds(expiration)
        .and_then(|(start, _)| crate::calendar::date_at(start))
        .ok_or("INVALID_DATE")?;
    if root == "SPX" && expiry == today {
        return Err("AM_SETTLEMENT_DAY");
    }
    if root == "SPXW" {
        let crate::calendar::Day::Open(session) = crate::calendar::schedule(expiry) else {
            return Err("INVALID_PM_SESSION");
        };
        if now >= session.close_ms {
            return Err("PM_CONTRACT_EXPIRED");
        }
    }
    if !(0..=10).contains(&expiry.signed_duration_since(today).num_days()) {
        return Err("EXPIRATION_OUT_OF_RANGE");
    }
    let mut by_strike: BTreeMap<u64, (Option<&Quote>, Option<&Quote>)> = BTreeMap::new();
    let mut oldest = now;
    for quote in quotes {
        if quote.root != root || quote.expiration != expiration {
            return Err("MIXED_CONTRACTS");
        }
        if !quote.strike.is_finite()
            || quote.strike <= 0.0
            || !quote.bid.is_finite()
            || !quote.ask.is_finite()
            || quote.bid < 0.0
            || quote.ask < quote.bid
        {
            return Err("INVALID_QUOTE");
        }
        if !fresh(quote.quote_ms, now) {
            return Err("STALE_QUOTE");
        }
        oldest = oldest.min(quote.quote_ms);
        let entry = by_strike.entry(quote.strike.to_bits()).or_default();
        let slot = if quote.side == Side::Call {
            &mut entry.0
        } else {
            &mut entry.1
        };
        if slot.replace(quote).is_some() {
            return Err("DUPLICATE_CONTRACT");
        }
    }
    let mut candidates = Vec::new();
    for (call, put) in by_strike.values() {
        let (Some(call), Some(put)) = (call, put) else {
            continue;
        };
        if call.quote_ms.abs_diff(put.quote_ms) > 2_000 {
            continue;
        }
        let call_mid = call.bid / 2.0 + call.ask / 2.0;
        let put_mid = put.bid / 2.0 + put.ask / 2.0;
        let combined_spread = (call.ask - call.bid) + (put.ask - put.bid);
        let relative_spread = combined_spread / (call_mid + put_mid).max(1.0);
        let difference = (call_mid - put_mid).abs();
        let synthetic = call.strike + call_mid - put_mid;
        let score = difference + 2.0 * combined_spread + 10.0 * relative_spread;
        if ![
            call_mid,
            put_mid,
            combined_spread,
            relative_spread,
            synthetic,
            score,
        ]
        .iter()
        .all(|v| v.is_finite())
        {
            return Err("NUMERIC_OVERFLOW");
        }
        if call_mid <= 0.0
            || put_mid <= 0.0
            || synthetic <= 0.0
            || difference > 75.0
            || combined_spread > 25.0
            || relative_spread > 0.45
        {
            continue;
        }
        candidates.push(Pair {
            strike: call.strike,
            call_mid,
            put_mid,
            combined_spread,
            relative_spread,
            synthetic,
            quality_score: score,
        });
    }
    candidates.sort_by(|a, b| a.quality_score.total_cmp(&b.quality_score));
    let count = candidates.len();
    candidates.truncate(15);
    if candidates.is_empty() {
        return Err("NO_VALID_PAIRS");
    }
    let initial = median(candidates.iter().map(|p| p.synthetic).collect());
    let mad = median(
        candidates
            .iter()
            .map(|p| (p.synthetic - initial).abs())
            .collect(),
    );
    let tolerance = (mad * 3.5).max(3.0);
    let filtered: Vec<_> = candidates
        .iter()
        .filter(|p| (p.synthetic - initial).abs() <= tolerance)
        .cloned()
        .collect();
    if filtered.len() >= 3 {
        candidates = filtered;
    }
    let price = median(candidates.iter().map(|p| p.synthetic).collect());
    Ok(Estimate {
        source: "LEGACY_PUT_CALL_PARITY_APPROXIMATION",
        approximate: true,
        usable_for_analysis: candidates.len() >= 3,
        expiration: expiration.into(),
        root: root.into(),
        price,
        candidate_pairs: count,
        pairs: candidates,
        oldest_quote_ms: oldest,
    })
}

#[derive(Clone, Debug)]
pub struct ReferencePrice {
    pub symbol: String,
    pub price: f64,
    pub timestamp_ms: u64,
    pub verified: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversion {
    pub source: &'static str,
    pub approximate: bool,
    pub nq_level: f64,
    pub qqq_level: f64,
    pub ratio: f64,
    pub oldest_price_ms: u64,
}
pub fn nq_to_qqq(
    level: f64,
    nq: &ReferencePrice,
    qqq: &ReferencePrice,
    now: u64,
) -> Result<Conversion, &'static str> {
    if nq.symbol != "NQ" || qqq.symbol != "QQQ" || !nq.verified || !qqq.verified {
        return Err("UNVERIFIED_SOURCE");
    }
    if ![level, nq.price, qqq.price]
        .iter()
        .all(|p| p.is_finite() && *p > 0.0)
    {
        return Err("INVALID_PRICE");
    }
    if !fresh(nq.timestamp_ms, now)
        || !fresh(qqq.timestamp_ms, now)
        || nq.timestamp_ms.abs_diff(qqq.timestamp_ms) > 2_000
    {
        return Err("STALE_OR_ASYNCHRONOUS_PRICES");
    }
    let ratio = qqq.price / nq.price;
    let value = level * ratio;
    if !value.is_finite() || !ratio.is_finite() || value <= 0.0 {
        return Err("NUMERIC_OVERFLOW");
    }
    let qqq_level: f64 = format!("{value:.2}")
        .parse()
        .map_err(|_| "NUMERIC_OVERFLOW")?;
    if qqq_level <= 0.0 || !qqq_level.is_finite() {
        return Err("NUMERIC_OVERFLOW");
    }
    Ok(Conversion {
        source: "NQ_QQQ_OBSERVED_RATIO",
        approximate: true,
        nq_level: level,
        qqq_level,
        ratio,
        oldest_price_ms: nq.timestamp_ms.min(qqq.timestamp_ms),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn settlement_cutoffs_reject_am_day_and_pm_after_half_day_close() {
        let now = crate::calendar::legacy_eastern_ms("2026-11-27 13:00:00").unwrap();
        for root in ["SPX", "SPXW"] {
            let mut q = quote(Side::Call);
            q.root = root.into();
            q.expiration = "2026-11-27".into();
            q.quote_ms = now;
            assert!(spx(root, "2026-11-27", &[q], now, true).is_err());
        }
    }
    use super::*;
    fn clock() -> u64 {
        crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap()
    }
    fn quote(side: Side) -> Quote {
        Quote {
            root: "SPXW".into(),
            expiration: "2026-09-25".into(),
            side,
            strike: 6000.0,
            bid: 19.5,
            ask: 20.5,
            quote_ms: clock(),
        }
    }
    #[test]
    fn eight_legacy_synthetic_fixtures_match_with_explicit_low_sample_quality() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-synthetic.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let mut quotes = Vec::new();
            for pair in case["pairs"].as_array().unwrap() {
                let p: Vec<f64> = pair
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect();
                for (side, mid, spread) in [(Side::Call, p[1], p[3]), (Side::Put, p[2], p[4])] {
                    let mut q = quote(side);
                    q.strike = p[0];
                    q.bid = mid - spread / 2.0;
                    q.ask = mid + spread / 2.0;
                    quotes.push(q);
                }
            }
            let result = spx("SPXW", "2026-09-25", &quotes, clock(), true).unwrap();
            assert!(
                (result.price - case["expected"]["spot"].as_f64().unwrap()).abs() < 1e-9,
                "{}",
                case["name"]
            );
            assert_eq!(
                result.candidate_pairs,
                case["expected"]["candidates"].as_u64().unwrap() as usize
            );
            assert_eq!(
                result.pairs.len(),
                case["expected"]["used"].as_u64().unwrap() as usize
            );
            assert_eq!(result.usable_for_analysis, result.pairs.len() >= 3);
            assert!(result.approximate);
        }
    }
    #[test]
    fn mixed_expiry_root_stale_future_crossed_and_duplicate_quotes_are_rejected() {
        let base = vec![quote(Side::Call), quote(Side::Put)];
        for mode in 0..7 {
            let mut quotes = base.clone();
            match mode {
                0 => quotes[1].root = "SPX".into(),
                1 => quotes[1].expiration = "2026-09-26".into(),
                2 => quotes[1].quote_ms -= 15_001,
                3 => quotes[1].quote_ms += 1,
                4 => quotes[1].bid = 100.0,
                5 => quotes.push(quotes[0].clone()),
                _ => quotes[0].strike = f64::NAN,
            };
            assert!(spx("SPXW", "2026-09-25", &quotes, clock(), true).is_err());
        }
        assert!(spx("SPXW", "2026-09-25", &base, clock(), false).is_err());
    }
    #[test]
    fn nq_conversion_requires_two_current_verified_prices() {
        let nq = ReferencePrice {
            symbol: "NQ".into(),
            price: 21000.0,
            timestamp_ms: clock(),
            verified: true,
        };
        let mut qqq = ReferencePrice {
            symbol: "QQQ".into(),
            price: 500.0,
            timestamp_ms: clock(),
            verified: true,
        };
        assert_eq!(
            nq_to_qqq(21100.0, &nq, &qqq, clock()).unwrap().qqq_level,
            502.38
        );
        qqq.timestamp_ms -= 2001;
        assert!(nq_to_qqq(21100.0, &nq, &qqq, clock()).is_err());
        qqq.timestamp_ms = clock();
        qqq.verified = false;
        assert!(nq_to_qqq(21100.0, &nq, &qqq, clock()).is_err());
        qqq.verified = true;
        assert!(nq_to_qqq(f64::INFINITY, &nq, &qqq, clock()).is_err());
    }
}
