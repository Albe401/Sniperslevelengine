//! Observed premarket activity only; neither institutional levels nor entry permission.
use crate::{bars::Minute, calendar};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub symbol: String,
    pub asof_ms: u64,
    pub historical: bool,
    pub status: String,
    pub price: Option<f64>,
    pub premarket_volume: Option<f64>,
    pub volume_5m: Option<f64>,
    pub premarket_high: Option<f64>,
    pub premarket_low: Option<f64>,
    pub notional_5m_approx: Option<f64>,
    pub last_bar_end_ms: Option<u64>,
    pub minutes_in_last_5m: usize,
    pub activity_ready: bool,
    pub entry_confirmed: bool,
}

/// `recovered` means the requested minute interval was completely retrieved.
/// Missing minutes stay missing: no activity or zero volume is fabricated.
pub fn read(symbol: &str, cutoff: u64, now: u64, minutes: &[Minute], recovered: bool) -> Read {
    let date = calendar::date_at(cutoff);
    let today = calendar::date_at(now);
    let mut out = Read {
        symbol: symbol.into(),
        asof_ms: cutoff,
        historical: date.zip(today).is_some_and(|(d, t)| d < t),
        status: "PREMARKET_UNAVAILABLE".into(),
        price: None,
        premarket_volume: None,
        volume_5m: None,
        premarket_high: None,
        premarket_low: None,
        notional_5m_approx: None,
        last_bar_end_ms: None,
        minutes_in_last_5m: 0,
        activity_ready: false,
        entry_confirmed: false,
    };
    if crate::universe::stock_asset(symbol).is_none()
        || today.is_none()
        || !recovered
        || minutes.len() > 50_000
        || cutoff > now
        || !cutoff.is_multiple_of(60_000)
    {
        return out;
    }
    let Some(date) = date else {
        return out;
    };
    let calendar::Day::Open(session) = calendar::schedule(date) else {
        return out;
    };
    let Some(start) = calendar::legacy_eastern_ms(&format!("{date} 04:00:00")) else {
        return out;
    };
    if cutoff <= start || cutoff > session.open_ms {
        return out;
    }
    let mut bars = BTreeMap::new();
    for bar in minutes {
        if bar.t < start
            || bar.t >= cutoff
            || !bar.t.is_multiple_of(60_000)
            || bar.t.saturating_add(60_000) > cutoff
        {
            continue;
        }
        // Corrupt in-range data makes the result unknown, not artificially quiet.
        if ![bar.o, bar.h, bar.l, bar.c]
            .iter()
            .all(|p| p.is_finite() && *p > 0.0)
            || !bar.v.is_finite()
            || bar.v < 0.0
            || bar.h < bar.o.max(bar.c)
            || bar.l > bar.o.min(bar.c)
        {
            return out;
        }
        bars.insert(bar.t, bar);
    }
    let Some((&last_t, last)) = bars.last_key_value() else {
        out.status = "NO_PREMARKET_BARS".into();
        return out;
    };
    let recent: Vec<_> = bars
        .range(cutoff.saturating_sub(300_000)..cutoff)
        .map(|(_, b)| *b)
        .collect();
    let volume: f64 = bars.values().map(|b| b.v).sum();
    let volume_5m: f64 = recent.iter().fold(0.0, |sum, b| sum + b.v);
    let notional = last.c * volume_5m;
    if ![volume, volume_5m, notional].iter().all(|n| n.is_finite()) {
        return out;
    }
    let end = last_t + 60_000;
    out.price = Some(last.c);
    out.premarket_volume = Some(volume);
    out.volume_5m = Some(volume_5m);
    out.notional_5m_approx = Some(notional);
    out.premarket_high = bars.values().map(|b| b.h).reduce(f64::max);
    out.premarket_low = bars.values().map(|b| b.l).reduce(f64::min);
    out.last_bar_end_ms = Some(end);
    out.minutes_in_last_5m = recent.len();
    out.activity_ready = cutoff - end <= 180_000 && recent.len() >= 3 && volume_5m > 0.0;
    out.status = match (out.historical, out.activity_ready) {
        (true, true) => "PREMARKET_HISTORICAL",
        (true, false) => "PREMARKET_HISTORICAL_INSUFFICIENT",
        (false, true) => "PREMARKET_ACTIVITY_AT_CUTOFF",
        (false, false) => "PREMARKET_INSUFFICIENT",
    }
    .into();
    out
}

/// Descriptive morning watchlist from the active legacy dashboard worker.
/// It expires at the RTH open; it is never reused as a current RTH candidate.
pub fn watch(rows: &[Read], now: u64, connected: bool) -> Vec<Read> {
    let Some(date) = calendar::date_at(now) else {
        return vec![];
    };
    let calendar::Day::Open(day) = calendar::schedule(date) else {
        return vec![];
    };
    if !connected || now < day.open_ms - 900_000 || now >= day.open_ms || rows.len() > 256 {
        return vec![];
    }
    let mut counts = BTreeMap::new();
    for row in rows {
        *counts.entry(row.symbol.as_str()).or_insert(0usize) += 1;
    }
    let mut selected: Vec<_> = rows
        .iter()
        .filter(|row| {
            counts[row.symbol.as_str()] == 1
                && crate::universe::stock_asset(&row.symbol).is_some()
                && row.activity_ready
                && !row.entry_confirmed
                && !row.historical
                && row.asof_ms <= now
                && calendar::date_at(row.asof_ms) == Some(date)
                && row
                    .last_bar_end_ms
                    .is_some_and(|t| t <= row.asof_ms && now.saturating_sub(t) <= 180_000)
                && row.minutes_in_last_5m >= 3
                && row
                    .notional_5m_approx
                    .is_some_and(|v| v.is_finite() && v > 0.0)
        })
        .cloned()
        .collect();
    selected.sort_by(|a, b| {
        b.notional_5m_approx
            .unwrap()
            .total_cmp(&a.notional_5m_approx.unwrap())
            .then(a.symbol.cmp(&b.symbol))
    });
    selected.truncate(5);
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nine_legacy_premarket_cases_match_observed_activity() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-premarket.json")).unwrap();
        let now = fixture["now"].as_u64().unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let rows: Vec<Minute> = serde_json::from_value(case["rows"].clone()).unwrap();
            let r = read("QQQ", now, now, &rows, true);
            let actual = serde_json::json!({"price":r.price,"premarket_volume":r.premarket_volume,"volume_5m":r.volume_5m,"premarket_high":r.premarket_high,"premarket_low":r.premarket_low,"notional_5m_approx":r.notional_5m_approx,"activity_ready":r.activity_ready,"entry_confirmed":r.entry_confirmed});
            for (key, expected) in case["expected"].as_object().unwrap() {
                if expected.is_number() {
                    assert_eq!(
                        actual[key].as_f64(),
                        expected.as_f64(),
                        "{}: {key}",
                        case["name"]
                    );
                } else {
                    assert_eq!(&actual[key], expected, "{}: {key}", case["name"]);
                }
            }
        }
    }
    fn bar(t: u64, v: f64) -> Minute {
        Minute {
            t,
            o: 100.0,
            h: 102.0,
            l: 99.0,
            c: 101.0,
            v,
            vw: None,
        }
    }
    #[test]
    fn sparse_minutes_completed_at_cutoff_are_activity_never_entry_permission() {
        let cutoff = calendar::legacy_eastern_ms("2026-09-24 09:15:00").unwrap();
        let mut rows = vec![
            bar(cutoff - 300_000, 10.0),
            bar(cutoff - 180_000, 20.0),
            bar(cutoff - 60_000, 30.0),
            bar(cutoff, 999.0),
        ];
        rows.push(bar(cutoff - 60_000, 40.0));
        let result = read("QQQ", cutoff, cutoff, &rows, true);
        assert_eq!(result.volume_5m, Some(70.0));
        assert_eq!(result.minutes_in_last_5m, 3);
        assert!(result.activity_ready);
        assert!(!result.entry_confirmed);
        assert_eq!(result.notional_5m_approx, Some(7070.0));
        assert!(read("QQQ", cutoff, cutoff + 86_400_000, &rows, true).historical);
        rows.remove(0);
        assert!(!read("QQQ", cutoff, cutoff, &rows, true).activity_ready);
    }
    #[test]
    fn recovery_calendar_future_invalid_and_overflow_do_not_publish_activity() {
        let cutoff = calendar::legacy_eastern_ms("2026-09-24 09:15:00").unwrap();
        let rows = vec![bar(cutoff - 60_000, 1.0)];
        assert!(read("QQQ", cutoff, cutoff, &rows, false).price.is_none());
        assert!(read("QQQ", cutoff, cutoff - 1, &rows, true).price.is_none());
        assert!(read("SPX", cutoff, cutoff, &rows, true).price.is_none());
        for date in ["2026-12-25", "2029-09-24"] {
            let cutoff = calendar::legacy_eastern_ms(&format!("{date} 09:15:00")).unwrap();
            assert!(
                read("QQQ", cutoff, cutoff, &[bar(cutoff - 60_000, 1.0)], true)
                    .price
                    .is_none()
            );
        }
        assert!(read(
            "QQQ",
            cutoff,
            cutoff,
            &[bar(cutoff - 60_000, f64::MAX)],
            true
        )
        .price
        .is_none());
        let mut invalid = bar(cutoff - 60_000, 1.0);
        invalid.h = 90.0;
        assert!(read("QQQ", cutoff, cutoff, &[invalid], true)
            .price
            .is_none());
    }
    #[test]
    fn morning_watch_ranks_notionals_expires_and_never_becomes_rth_evidence() {
        let cutoff = calendar::legacy_eastern_ms("2026-09-24 09:15:00").unwrap();
        let bars = vec![
            bar(cutoff - 300_000, 10.0),
            bar(cutoff - 180_000, 10.0),
            bar(cutoff - 60_000, 10.0),
        ];
        let mut rows: Vec<_> = ["SPY", "QQQ", "NVDA", "MSFT", "AMD", "AAPL"]
            .iter()
            .map(|s| read(s, cutoff, cutoff, &bars, true))
            .collect();
        assert_eq!(
            watch(&rows, cutoff, true)
                .iter()
                .map(|r| r.symbol.as_str())
                .collect::<Vec<_>>(),
            ["AAPL", "AMD", "MSFT", "NVDA", "QQQ"]
        );
        for (now, connected) in [
            (cutoff - 1, true),
            (cutoff + 180_001, true),
            (cutoff + 900_000, true),
            (cutoff, false),
        ] {
            assert!(watch(&rows, now, connected).is_empty());
        }
        rows.push(rows[5].clone());
        assert!(!watch(&rows, cutoff, true)
            .iter()
            .any(|r| r.symbol == "AAPL"));
        rows[0].notional_5m_approx = Some(f64::INFINITY);
        assert!(!watch(&rows, cutoff, true).iter().any(|r| r.symbol == "SPY"));
    }
}
