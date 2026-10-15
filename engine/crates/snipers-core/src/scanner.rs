//! Activity preselection only; options, trigger and entry approval remain separate.
use crate::{
    calendar::{self, Day},
    universe,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub symbol: String,
    pub bar_end_ms: u64,
    pub price: f64,
    pub volume_2m: f64,
    pub vwap: Option<f64>,
    pub rvol: Option<f64>,
    pub source_verified: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub symbol: String,
    pub sector: Option<String>,
    pub price: f64,
    pub volume_2m: f64,
    pub notional_2m: f64,
    pub vwap: Option<f64>,
    pub rvol: Option<f64>,
    pub volume_class: String,
    pub direction: String,
    pub base_ready: bool,
    pub bar_end_ms: u64,
    pub entry_confirmed: bool,
}
pub fn classify_rvol(v: Option<f64>) -> &'static str {
    match v {
        Some(v) if v.is_finite() && v >= 0.0 => {
            if v < 0.7 {
                "BAJO"
            } else if v < 1.0 {
                "MEDIO"
            } else if v < 1.5 {
                "BUENO"
            } else {
                "FUERTE"
            }
        }
        _ => "SIN DATOS",
    }
}
pub fn candidate(i: &Input, now: u64) -> Option<Candidate> {
    let asset = universe::stock_asset(&i.symbol)?;
    let today = calendar::date_at(now)?;
    let Day::Open(session) = calendar::schedule(today) else {
        return None;
    };
    if !i.source_verified
        || now < session.open_ms
        || now >= session.close_ms
        || i.bar_end_ms > now
        || now - i.bar_end_ms > 120_000
        || i.bar_end_ms < session.open_ms + 120_000
        || i.bar_end_ms > session.close_ms
        || !(i.bar_end_ms - session.open_ms).is_multiple_of(120_000)
    {
        return None;
    }
    if [i.price, i.volume_2m]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0)
    {
        return None;
    }
    let notional = i.price * i.volume_2m;
    if !notional.is_finite() {
        return None;
    }
    let vwap = i.vwap.filter(|v| v.is_finite() && *v > 0.0);
    let rvol = i.rvol.filter(|v| v.is_finite() && *v >= 0.0);
    let price: f64 = format!("{:.4}", i.price).parse().ok()?;
    let direction = match vwap {
        Some(v) if price > v => "ALCISTA",
        Some(v) if price < v => "BAJISTA",
        _ => "NO VERIFICADA",
    };
    Some(Candidate {
        symbol: i.symbol.clone(),
        sector: asset.sector.clone(),
        price,
        volume_2m: i.volume_2m,
        notional_2m: format!("{notional:.2}").parse().ok()?,
        vwap,
        rvol,
        volume_class: classify_rvol(rvol).into(),
        direction: direction.into(),
        base_ready: rvol.is_some_and(|v| v >= 1.0) && direction != "NO VERIFICADA",
        bar_end_ms: i.bar_end_ms,
        entry_confirmed: false,
    })
}
pub fn rank(rows: &[Input], now: u64) -> Vec<Candidate> {
    let mut out = rank_all(rows, now);
    out.truncate(5);
    out
}
pub fn rank_all(rows: &[Input], now: u64) -> Vec<Candidate> {
    // Duplicate identities are ambiguous; never let an older copy win ranking.
    let mut counts = std::collections::BTreeMap::new();
    for r in rows {
        *counts.entry(&r.symbol).or_insert(0) += 1;
    }
    let mut out: Vec<_> = rows
        .iter()
        .filter(|r| counts[&r.symbol] == 1)
        .filter_map(|r| candidate(r, now))
        .collect();
    out.sort_by(|a, b| {
        b.notional_2m
            .total_cmp(&a.notional_2m)
            .then(a.symbol.cmp(&b.symbol))
    });
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activity_ranking_excludes_stale_indices_duplicates_and_closed_sessions() {
        let Day::Open(day) = calendar::schedule(
            calendar::date_at(calendar::day_bounds("2026-09-24").unwrap().0).unwrap(),
        ) else {
            panic!()
        };
        let now = day.open_ms + 120_000;
        let row = Input {
            symbol: "QQQ".into(),
            bar_end_ms: now,
            price: 100.0,
            volume_2m: 10.0,
            vwap: Some(99.0),
            rvol: Some(1.0),
            source_verified: true,
        };
        assert!(candidate(&row, now).unwrap().base_ready);
        assert!(!candidate(&row, now).unwrap().entry_confirmed);
        assert!(candidate(&row, now + 120_001).is_none());
        assert!(candidate(&row, now - 1).is_none());
        assert!(candidate(&row, day.close_ms).is_none());
        let mut other = row.clone();
        other.symbol = "SPY".into();
        other.volume_2m = 20.0;
        let ranked = rank(&[row.clone(), other.clone()], now);
        assert_eq!(ranked[0].symbol, "SPY");
        other.volume_2m = 10.0;
        assert_eq!(rank(&[other.clone(), row.clone()], now)[0].symbol, "QQQ");
        assert!(rank(&[row.clone(), row.clone()], now).is_empty());
        other.symbol = "SPX".into();
        assert!(candidate(&other, now).is_none());
        other = row.clone();
        other.volume_2m = f64::MAX;
        assert!(candidate(&other, now).is_none());
        other = row.clone();
        other.source_verified = false;
        assert!(candidate(&other, now).is_none());
        other = row;
        other.rvol = None;
        assert!(!candidate(&other, now).unwrap().base_ready);
        for (v, label) in [
            (0.699, "BAJO"),
            (0.7, "MEDIO"),
            (1.0, "BUENO"),
            (1.5, "FUERTE"),
        ] {
            assert_eq!(classify_rvol(Some(v)), label);
        }
        let holiday = calendar::day_bounds("2026-12-25").unwrap().0 + 36_000_000;
        other.bar_end_ms = holiday;
        assert!(candidate(&other, holiday).is_none());
    }
}
