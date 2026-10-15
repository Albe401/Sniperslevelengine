//! Quote coverage filter for selected contracts; never an execution guarantee.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub ticker: String,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub bid_size: Option<f64>,
    pub ask_size: Option<f64>,
    pub timestamp_ms: Option<u64>,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub status: String,
    pub ready: bool,
    pub contracts_tracked: usize,
    pub quoted: usize,
    pub recent: usize,
    pub depth: usize,
    pub quote_coverage_pct: f64,
    pub recent_coverage_pct: f64,
    pub depth_coverage_pct: f64,
    pub average_spread_pct: Option<f64>,
    pub entry_confirmed: bool,
}
pub fn evaluate(rows: &[Quote], now: u64, connected: bool) -> Read {
    let mut out = Read {
        contracts_tracked: rows.len(),
        ..Default::default()
    };
    let mut seen = BTreeSet::new();
    if rows
        .iter()
        .any(|q| q.ticker.is_empty() || !seen.insert(&q.ticker))
    {
        out.status = "UNIVERSO AMBIGUO".into();
        return out;
    }
    let positive = |v: Option<f64>| v.filter(|v| v.is_finite() && *v > 0.0);
    let mut spreads = vec![];
    for q in rows {
        let (Some(bid), Some(ask)) = (positive(q.bid), positive(q.ask)) else {
            continue;
        };
        if ask < bid {
            continue;
        }
        out.quoted += 1;
        // Halving before addition avoids overflowing two individually valid prices.
        let midpoint = bid / 2.0 + ask / 2.0;
        spreads.push((ask - bid) / midpoint * 100.0);
        if q.timestamp_ms
            .is_some_and(|t| t > 0 && t <= now && now - t <= 12_000)
        {
            out.recent += 1;
        }
        if positive(q.bid_size).is_some() && positive(q.ask_size).is_some() {
            out.depth += 1;
        }
    }
    let pct = |n: usize| {
        if rows.is_empty() {
            0.0
        } else {
            n as f64 / rows.len() as f64 * 100.0
        }
    };
    let quote = pct(out.quoted);
    let recent = pct(out.recent);
    let depth = pct(out.depth);
    let spread = if spreads.is_empty() {
        None
    } else {
        Some(spreads.iter().sum::<f64>() / spreads.len() as f64)
    };
    out.status = if !connected {
        "SIN CONEXION VIGENTE"
    } else if out.quoted < 2 {
        "POR VERIFICAR"
    } else if out.recent < 2 || recent < 70.0 {
        "QUOTE ANTIGUA / ESPERAR"
    } else if quote < 70.0 || depth < 50.0 || spread.is_none_or(|s| !s.is_finite() || s > 15.0) {
        "LIQUIDEZ DEBIL"
    } else {
        out.ready = true;
        "LIQUIDEZ ACEPTABLE"
    }
    .into();
    out.quote_coverage_pct = round(quote, 1);
    out.recent_coverage_pct = round(recent, 1);
    out.depth_coverage_pct = round(depth, 1);
    out.average_spread_pct = spread.filter(|s| s.is_finite()).map(|s| round(s, 2));
    out
}
fn round(v: f64, digits: usize) -> f64 {
    format!("{v:.digits$}").parse().unwrap_or(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_liquidity_fixtures_and_future_disconnect_duplicate_guards() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/legacy-option-liquidity.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let rows: Vec<Quote> = serde_json::from_value(case["rows"].clone()).unwrap();
            let now = case["now"].as_u64().unwrap();
            let result = evaluate(&rows, now, true);
            let expected = &case["expected"];
            assert_eq!(
                result.ready,
                expected["liquidity_filter_pass"].as_bool().unwrap(),
                "{rows:?}"
            );
            assert_eq!(result.status, expected["status"].as_str().unwrap());
            assert_eq!(
                result.quoted,
                expected["contracts_with_bid_ask"].as_u64().unwrap() as usize
            );
            assert_eq!(
                result.recent,
                expected["contracts_with_recent_bid_ask"].as_u64().unwrap() as usize
            );
            assert_eq!(
                result.depth,
                expected["contracts_with_depth"].as_u64().unwrap() as usize
            );
            assert_eq!(
                result.average_spread_pct,
                expected["average_spread_pct"].as_f64()
            );
            assert!(!result.entry_confirmed);
            assert!(!evaluate(&rows, now, false).ready);
            let mut future = rows.clone();
            for row in &mut future {
                row.timestamp_ms = Some(now + 1);
            }
            assert!(!evaluate(&future, now, true).ready);
            if let Some(row) = rows.first() {
                assert!(!evaluate(&[row.clone(), row.clone()], now, true).ready);
            }
        }
    }
}
