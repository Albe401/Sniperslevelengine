//! Read-only local sensitivities of one long option. Never an execution signal.
use crate::options::{ContractSnapshot, Observation};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub ticker: String,
    pub expiration: String,
    pub kind: String,
    pub strike: f64,
    pub observed_ms: u64,
    pub quote_ms: Option<u64>,
    pub recent: bool,
    pub theta: Option<f64>,
    pub vega: Option<f64>,
    pub iv: Option<f64>,
    pub multiplier: Option<f64>,
    pub theta_dollars_day: Option<f64>,
    pub vega_dollars_point: Option<f64>,
    pub theta_pct_day: Option<f64>,
    pub vega_pct_point: Option<f64>,
}
pub fn read(c: &ContractSnapshot, observed_ms: u64, now: u64) -> Read {
    let theta = c.theta.filter(|n| n.is_finite());
    let vega = c.vega.filter(|n| n.is_finite() && *n >= 0.);
    let multiplier = c.premium_multiplier.filter(|n| n.is_finite() && *n > 0.);
    let mid = c
        .bid
        .zip(c.ask)
        .filter(|(b, a)| b.is_finite() && a.is_finite() && *b > 0. && a >= b)
        .map(|(b, a)| b / 2. + a / 2.);
    let product =
        |a: Option<f64>, b: Option<f64>| a.zip(b).map(|(a, b)| a * b).filter(|v| v.is_finite());
    let recent = observed_ms > 0
        && observed_ms <= now
        && now - observed_ms <= 900_000
        && c.quote_timeframe.as_deref() == Some("REAL-TIME")
        && c.quote_ms
            .is_some_and(|t| t > 0 && t <= now && now - t <= 60_000);
    Read {
        ticker: c.ticker.clone(),
        expiration: c.expiration.clone(),
        kind: c.kind.clone(),
        strike: c.strike,
        observed_ms,
        quote_ms: c.quote_ms,
        recent,
        theta,
        vega,
        iv: c.iv.filter(|n| n.is_finite() && *n >= 0.),
        multiplier,
        theta_dollars_day: product(theta, multiplier),
        vega_dollars_point: product(vega, multiplier),
        theta_pct_day: product(theta, mid.map(|m| 100. / m)),
        vega_pct_point: product(vega, mid.map(|m| 100. / m)),
    }
}
/// One nearest-to-reference CALL and PUT per expiry, up to four expiries.
pub fn overview(o: &Observation, symbol: &str, now: u64) -> Vec<Read> {
    if o.symbol != symbol {
        return vec![];
    }
    let mut groups = std::collections::BTreeMap::new();
    for c in o.contracts.values() {
        if !matches!(c.kind.as_str(), "call" | "put") {
            continue;
        }
        let Some(spot) = c.underlying_price.filter(|s| s.is_finite() && *s > 0.) else {
            continue;
        };
        let distance = (c.strike - spot).abs();
        let key = (c.expiration.clone(), c.kind.clone());
        let entry = groups.entry(key).or_insert((distance, c));
        if distance < entry.0 {
            *entry = (distance, c);
        }
    }
    let expiries: std::collections::BTreeSet<_> = groups.keys().map(|(e, _)| e.clone()).collect();
    let expiries: Vec<_> = expiries.into_iter().take(4).collect();
    groups
        .into_iter()
        .filter(|((e, _), _)| expiries.contains(e))
        .map(|(_, (_, c))| read(c, o.observed_ms, now))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn contract() -> ContractSnapshot {
        let mut c=crate::options::normalize(&serde_json::json!({"details":{"ticker":"O:QQQ261002C00745000","expiration_date":"2026-10-02","contract_type":"call","strike_price":745},"greeks":{"theta":-0.1,"vega":0.2},"last_quote":{"bid":1.9,"ask":2.1,"last_updated":1000000000_u64,"timeframe":"REAL-TIME"},"underlying_asset":{"price":745}}),"QQQ").unwrap();
        c.premium_multiplier = Some(100.);
        c
    }
    #[test]
    fn units_signs_and_zero() {
        let c = contract();
        let r = read(&c, 1000, 1100);
        assert_eq!(r.theta_dollars_day, Some(-10.));
        assert_eq!(r.vega_dollars_point, Some(20.));
        assert_eq!(r.theta_pct_day, Some(-5.));
        assert_eq!(r.vega_pct_point, Some(10.));
        assert!(r.recent);
        let mut c = c;
        c.theta = Some(0.);
        assert_eq!(read(&c, 1000, 1100).theta, Some(0.));
    }
    #[test]
    fn absent_multiplier_and_stale_quotes() {
        let mut c = contract();
        c.premium_multiplier = None;
        c.theta = None;
        c.vega = Some(-1.);
        let r = read(&c, 1000, 100000);
        assert!(!r.recent);
        assert_eq!(r.theta_dollars_day, None);
        assert_eq!(r.vega, None);
        assert!(!read(&c, 2000, 1000).recent);
        c.quote_timeframe = Some("DELAYED".into());
        assert!(!read(&c, 1000, 1100).recent);
    }
    #[test]
    fn rejects_overflow_and_crossed_mid() {
        let mut c = contract();
        c.theta = Some(f64::MAX);
        c.bid = Some(3.);
        let r = read(&c, 1000, 1100);
        assert_eq!(r.theta_dollars_day, None);
        assert_eq!(r.theta_pct_day, None);
    }
    #[test]
    fn overview_is_symbol_specific() {
        let c = contract();
        let o = Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: 1000,
            observed_date: "2026-09-25".into(),
            oi_reference_date: None,
            contracts: [(c.ticker.clone(), c)].into(),
        };
        assert_eq!(overview(&o, "QQQ", 1100).len(), 1);
        assert!(overview(&o, "SPY", 1100).is_empty());
    }
}
