//! Verified research metrics; the investment policy assigns their score weights after enrichment.
use serde_json::Value;
use std::collections::BTreeMap;

pub fn beta(stock: &Value, benchmark: &Value, ticker: &str, now: u64) -> Option<(f64, usize)> {
    fn closes(v: &Value, symbol: &str, now: u64) -> Option<BTreeMap<String, f64>> {
        if v["ticker"] != symbol || v["adjusted"] != true {
            return None;
        }
        let today = crate::calendar::date_at(now)?;
        let mut out = BTreeMap::new();
        for r in v["results"].as_array()? {
            let t = r["t"].as_u64()?;
            let d = crate::calendar::date_at(t)?;
            if d >= today || now.saturating_sub(t) > 400 * 86400000 {
                continue;
            }
            let c = r["c"].as_f64()?;
            if !c.is_finite() || c <= 0. || out.insert(d.to_string(), c).is_some() {
                return None;
            }
        }
        Some(out)
    }
    let a = closes(stock, ticker, now)?;
    let b = closes(benchmark, "SPY", now)?;
    let last = b.last_key_value()?.0;
    if a.last_key_value()?.0 != last {
        return None;
    }
    let last_date = chrono::NaiveDate::parse_from_str(last, "%Y-%m-%d").ok()?;
    if (crate::calendar::date_at(now)? - last_date).num_days() > 7 {
        return None;
    }
    let days = b.iter().collect::<Vec<_>>();
    let pairs = days
        .windows(2)
        .rev()
        .take(252)
        .filter_map(|w| {
            let x0 = a.get(w[0].0)?;
            let x1 = a.get(w[1].0)?;
            Some((x1 / x0 - 1., w[1].1 / w[0].1 - 1.))
        })
        .collect::<Vec<_>>();
    if pairs.len() < 200 {
        return None;
    }
    let n = pairs.len() as f64;
    let mx = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let var = pairs.iter().map(|p| (p.1 - my).powi(2)).sum::<f64>();
    if var <= 1e-12 {
        return None;
    }
    let value = pairs.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / var;
    value.is_finite().then_some((value, pairs.len()))
}

/// Four pending consecutive quarters. Not a fiscal-year P/E, nor annualized last quarter.
pub fn forward_pe(data: &Value, ticker: &str, price: f64, now: u64) -> Option<(f64, String)> {
    if !price.is_finite() || price <= 0. || data["next_url"].as_str().is_some_and(|s| !s.is_empty())
    {
        return None;
    }
    let today = crate::calendar::date_at(now)?;
    let mut qs = BTreeMap::new();
    for r in data["results"].as_array()? {
        if r["ticker"] != ticker || r["currency"] != "USD" || !r["actual_eps"].is_null() {
            continue;
        }
        let date = chrono::NaiveDate::parse_from_str(r["date"].as_str()?, "%Y-%m-%d").ok()?;
        let days = (date - today).num_days();
        if !(1..=400).contains(&days) {
            continue;
        }
        let q = match r["fiscal_period"].as_str() {
            Some("Q1") => 1,
            Some("Q2") => 2,
            Some("Q3") => 3,
            Some("Q4") => 4,
            _ => continue,
        };
        let y = r["fiscal_year"].as_i64()?;
        let updated = chrono::DateTime::parse_from_rfc3339(r["last_updated"].as_str()?)
            .ok()?
            .timestamp_millis();
        if updated < 0 || updated as u64 > now || now - updated as u64 > 90 * 86400000 {
            continue;
        }
        let eps = r["estimated_eps"].as_f64()?;
        let method = r["eps_method"].as_str()?.to_ascii_lowercase();
        if !eps.is_finite() || !matches!(method.as_str(), "gaap" | "adj") {
            continue;
        }
        if qs.insert(y * 4 + q, (eps, method, date)).is_some() {
            return None;
        }
    }
    let four = qs.iter().take(4).collect::<Vec<_>>();
    if four.len() != 4
        || four
            .windows(2)
            .any(|w| *w[1].0 != *w[0].0 + 1 || w[1].1 .1 != w[0].1 .1 || w[1].1 .2 <= w[0].1 .2)
    {
        return None;
    }
    let sum = four.iter().map(|(_, v)| v.0).sum::<f64>();
    let pe = price / sum;
    (sum>0. && pe.is_finite()).then(||(pe,format!("Consenso Benzinga / Massive: 4 trimestres pendientes, {}. Estimaciones actualizadas <=90 dias; no probabilidad.",four[0].1.1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn beta_self_and_wrong_identity() {
        let now = 1790467200000;
        let rows = (1..=300)
            .map(|i| serde_json::json!({"t":now-(301-i)*86400000,"c":100.+i as f64+(i%3) as f64}))
            .collect::<Vec<_>>();
        let b = serde_json::json!({"ticker":"SPY","adjusted":true,"results":rows});
        assert!((beta(&b, &b, "SPY", now).unwrap().0 - 1.).abs() < 1e-10);
        assert!(beta(&b, &b, "AVGO", now).is_none());
    }
    #[test]
    fn forward_requires_four_consistent_pending_quarters() {
        let now = 1790467200000;
        let mut v = serde_json::json!({"results":[
            {"ticker":"T","currency":"USD","date":"2026-10-20","fiscal_period":"Q3","fiscal_year":2026,"last_updated":"2026-09-25T12:00:00Z","estimated_eps":2.,"eps_method":"gaap"},
            {"ticker":"T","currency":"USD","date":"2027-01-20","fiscal_period":"Q4","fiscal_year":2026,"last_updated":"2026-09-25T12:00:00Z","estimated_eps":2.,"eps_method":"gaap"},
            {"ticker":"T","currency":"USD","date":"2027-04-20","fiscal_period":"Q1","fiscal_year":2027,"last_updated":"2026-09-25T12:00:00Z","estimated_eps":2.,"eps_method":"gaap"},
            {"ticker":"T","currency":"USD","date":"2027-07-20","fiscal_period":"Q2","fiscal_year":2027,"last_updated":"2026-09-25T12:00:00Z","estimated_eps":2.,"eps_method":"gaap"}]});
        assert_eq!(forward_pe(&v, "T", 80., now).unwrap().0, 10.);
        v["results"][0]["actual_eps"] = serde_json::json!(2.);
        assert!(forward_pe(&v, "T", 80., now).is_none());
    }
}
