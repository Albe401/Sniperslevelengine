//! Display-only volume context for Top 5. Never affects selection or entry rules.
use crate::live::Bar;
use serde::Serialize;
use snipers_core::calendar::{self, Day};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeMetrics {
    pub notional_usd: Option<f64>,
    pub change10_pct: Option<f64>,
    pub change30_pct: Option<f64>,
    pub rvol_premarket: Option<f64>,
    pub rvol_sessions: usize,
}

fn sum(bars: &BTreeMap<u64, Bar>, start: u64, end: u64) -> Option<f64> {
    if start >= end {
        return None;
    }
    let mut count = 0;
    let mut volume = 0.;
    for (_, b) in bars.range(start..end) {
        if b.timestamp_ms + 60_000 > end || !b.volume.is_finite() || b.volume < 0. {
            return None;
        }
        count += 1;
        volume += b.volume;
    }
    (count > 0 && volume.is_finite()).then_some(volume)
}

fn change(bars: &BTreeMap<u64, Bar>, session_start: u64, end: u64, minutes: u64) -> Option<f64> {
    let width = minutes * 60_000;
    if end < session_start + 2 * width {
        return None;
    }
    let current = sum(bars, end - width, end)?;
    let previous = sum(bars, end - 2 * width, end - width)?;
    if previous <= 0. {
        return None;
    }
    let value = (current / previous - 1.) * 100.;
    value.is_finite().then_some(value)
}

/// Same local ET interval, up to the five immediately preceding trading sessions.
/// Missing sessions are not zero-filled. Require at least three observed sessions.
pub(crate) fn premarket_rvol(
    bars: &BTreeMap<u64, Bar>,
    cutoff: u64,
    current: f64,
) -> (Option<f64>, usize) {
    let Some(date) = calendar::date_at(cutoff) else {
        return (None, 0);
    };
    let Day::Open(day) = calendar::schedule(date) else {
        return (None, 0);
    };
    let Some(start) = calendar::legacy_eastern_ms(&format!("{date} 04:00:00")) else {
        return (None, 0);
    };
    let end = cutoff.min(day.open_ms);
    if end <= start || !current.is_finite() || current < 0. {
        return (None, 0);
    }
    let mut previous_date = date;
    let mut sessions = 0;
    let mut values = vec![];
    for _ in 0..14 {
        let Some(d) = previous_date.pred_opt() else {
            break;
        };
        previous_date = d;
        if !matches!(calendar::schedule(d), Day::Open(_)) {
            continue;
        }
        sessions += 1;
        let Some(from) = calendar::legacy_eastern_ms(&format!("{d} 04:00:00")) else {
            continue;
        };
        // A truncated live tape must not count its earliest partial session.
        if bars
            .first_key_value()
            .is_some_and(|(first, _)| *first <= from)
        {
            if let Some(v) = sum(bars, from, from + end - start) {
                values.push(v);
            }
        }
        if sessions == 5 {
            break;
        }
    }
    let count = values.len();
    let average = values.iter().sum::<f64>() / count.max(1) as f64;
    let value = current / average;
    (
        (count >= 3 && average > 0. && value.is_finite()).then_some(value),
        count,
    )
}

pub(crate) fn calculate(bars: &BTreeMap<u64, Bar>, start: u64, end: u64) -> VolumeMetrics {
    let mut result = VolumeMetrics::default();
    if start >= end {
        return result;
    }
    let mut notional = 0.;
    let mut count = 0;
    for (_, b) in bars.range(start..end) {
        let price = b
            .vwap
            .filter(|p| p.is_finite() && *p > 0.)
            .unwrap_or(b.close);
        if !price.is_finite() || price <= 0. || !b.volume.is_finite() || b.volume < 0. {
            return result;
        }
        count += 1;
        notional += price * b.volume;
    }
    result.notional_usd = (count > 0 && notional.is_finite()).then_some(notional);
    let Some(date) = calendar::date_at(end) else {
        return result;
    };
    let Day::Open(day) = calendar::schedule(date) else {
        return result;
    };
    let Some(morning) = calendar::legacy_eastern_ms(&format!("{date} 04:00:00")) else {
        return result;
    };
    result.change10_pct = change(bars, morning, end, 10);
    result.change30_pct = change(bars, morning, end, 30);
    if let Some(volume) = sum(bars, morning, end.min(day.open_ms)) {
        (result.rvol_premarket, result.rvol_sessions) = premarket_rvol(bars, end, volume);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(s: &str) -> u64 {
        calendar::legacy_eastern_ms(s).unwrap()
    }
    fn bar(t: u64, volume: f64) -> Bar {
        Bar {
            timestamp_ms: t,
            open: 10.,
            high: 12.,
            low: 9.,
            close: 11.,
            volume,
            vwap: Some(10.5),
        }
    }
    #[test]
    fn dollars_and_consecutive_windows_exclude_unfinished_minutes() {
        let start = at("2026-09-28 04:00:00");
        let mut rows = BTreeMap::new();
        for i in 0..61 {
            rows.insert(
                start + i * 60_000,
                bar(start + i * 60_000, if i < 30 { 10. } else { 20. }),
            );
        }
        let m = calculate(&rows, start, start + 60 * 60_000);
        assert_eq!(m.notional_usd, Some(9450.));
        assert_eq!(m.change30_pct, Some(100.));
        assert_eq!(m.change10_pct, Some(0.));
        assert_eq!(
            calculate(&rows, start, start + 19 * 60_000).change10_pct,
            None
        );
        rows.retain(|t, _| *t >= start + 30 * 60_000);
        assert_eq!(
            calculate(&rows, start, start + 60 * 60_000).change30_pct,
            None
        );
    }
    #[test]
    fn premarket_compares_same_et_cutoff_skips_weekend_and_excludes_rth() {
        let mut rows = BTreeMap::new();
        for day in [
            "2026-09-21",
            "2026-09-22",
            "2026-09-23",
            "2026-09-24",
            "2026-09-25",
            "2026-09-28",
        ] {
            let start = at(&format!("{day} 04:00:00"));
            rows.insert(
                start,
                bar(start, if day.ends_with("28") { 200. } else { 100. }),
            );
            let later = at(&format!("{day} 08:00:00"));
            rows.insert(later, bar(later, 9999.));
        }
        let start = at("2026-09-28 04:00:00");
        let end = at("2026-09-28 07:00:00");
        let m = calculate(&rows, start, end);
        assert_eq!(m.rvol_premarket, Some(2.));
        assert_eq!(m.rvol_sessions, 5);
        rows.retain(|t, _| *t >= at("2026-09-24 04:00:00"));
        assert_eq!(calculate(&rows, start, end).rvol_premarket, None);
        assert_eq!(calculate(&rows, start, end).rvol_sessions, 2);
    }
    #[test]
    fn zero_baseline_and_invalid_notional_never_create_infinite_values() {
        let start = at("2026-09-28 04:00:00");
        let mut rows = BTreeMap::new();
        rows.insert(start, bar(start, 0.));
        rows.insert(start + 600_000, bar(start + 600_000, 10.));
        assert_eq!(
            calculate(&rows, start, start + 1_200_000).change10_pct,
            None
        );
        rows.get_mut(&start).unwrap().volume = f64::NAN;
        assert_eq!(
            calculate(&rows, start, start + 1_200_000).notional_usd,
            None
        );
    }
}
