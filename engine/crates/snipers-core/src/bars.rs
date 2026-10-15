//! Session boundaries come from the calendar, never guessed by these calculations.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
pub struct Minute {
    pub t: u64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: f64,
    pub vw: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionBar {
    pub timestamp: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub vwap: Option<f64>,
    pub minutes_received: usize,
}

fn valid(m: &Minute) -> bool {
    [m.o, m.h, m.l, m.c]
        .into_iter()
        .all(|p| p.is_finite() && p > 0.0)
        && m.v.is_finite()
        && m.v >= 0.0
        && m.h >= m.o.max(m.c)
        && m.l <= m.o.min(m.c)
}

/// Matches legacy OHLC/session VWAP on canonical (deduplicated) minute inputs.
pub fn session_bars(input: &[Minute], open: u64, close: u64, period: u64) -> Vec<SessionBar> {
    if ![1, 2, 5].contains(&period) || open >= close {
        return vec![];
    }
    let mut minutes = BTreeMap::new();
    for m in input {
        if valid(m) && m.t >= open && m.t < close && m.t.is_multiple_of(60_000) {
            minutes.insert(m.t, m);
        }
    }
    let mut groups: BTreeMap<u64, Vec<&Minute>> = BTreeMap::new();
    for m in minutes.into_values() {
        groups
            .entry(open + (m.t - open) / (period * 60_000) * (period * 60_000))
            .or_default()
            .push(m);
    }
    let (mut cumulative, mut volume, mut known) = (0.0, 0.0, true);
    groups
        .into_iter()
        .map(|(timestamp, rows)| {
            for m in &rows {
                if m.v > 0.0 {
                    if let Some(vw) = m.vw.filter(|p| p.is_finite()) {
                        cumulative += vw * m.v;
                        volume += m.v;
                    } else {
                        known = false;
                    }
                }
            }
            SessionBar {
                timestamp,
                open: rows[0].o,
                close: rows.last().unwrap().c,
                high: rows.iter().map(|m| m.h).fold(f64::NEG_INFINITY, f64::max),
                low: rows.iter().map(|m| m.l).fold(f64::INFINITY, f64::min),
                volume: rows.iter().map(|m| m.v).sum(),
                vwap: if known && volume > 0.0 {
                    Some(cumulative / volume)
                } else {
                    None
                },
                minutes_received: rows.len(),
            }
        })
        .collect()
}

/// Scanner's stricter VWAP: every minute through the final 2m slot must exist.
pub fn vwap_at_slot(minutes: &BTreeMap<u32, Minute>, slot: u32) -> Option<f64> {
    let (mut numerator, mut denominator) = (0.0, 0.0);
    for i in 0..slot.checked_add(2)? {
        let m = minutes.get(&i)?;
        if !valid(m) {
            return None;
        }
        if m.v > 0.0 {
            numerator += m.vw.filter(|p| p.is_finite())? * m.v;
            denominator += m.v;
        }
    }
    if denominator > 0.0 {
        Some(numerator / denominator)
    } else {
        None
    }
}

pub fn cumulative_at_slot(minutes: &BTreeMap<u32, Minute>, slot: u32) -> Option<f64> {
    let expected = slot.checked_add(2)?;
    if expected < 30 || !minutes.contains_key(&slot) || !minutes.contains_key(&(slot + 1)) {
        return None;
    }
    let rows: Vec<_> = (0..expected).filter_map(|i| minutes.get(&i)).collect();
    if rows.len() as f64 / (expected as f64) < 0.95 || rows.iter().any(|m| !valid(m)) {
        return None;
    }
    let volume = rows.iter().map(|m| m.v).sum::<f64>();
    (volume > 0.0).then_some(volume)
}

#[derive(Debug, Serialize)]
pub struct Rvol {
    pub value: f64,
    pub baseline_average: f64,
    pub class: &'static str,
}
pub fn relative_volume(
    target: &BTreeMap<u32, Minute>,
    history: &[BTreeMap<u32, Minute>],
    slot: u32,
) -> Option<Rvol> {
    let target = cumulative_at_slot(target, slot)?;
    let samples: Vec<_> = history
        .iter()
        .filter_map(|m| cumulative_at_slot(m, slot))
        .take(5)
        .collect();
    if samples.len() < 5 {
        return None;
    }
    let baseline_average = samples.iter().sum::<f64>() / 5.0;
    let raw = target / baseline_average;
    Some(Rvol {
        value: (raw * 10_000.0).round() / 10_000.0,
        baseline_average,
        class: if raw < 0.7 {
            "BAJO"
        } else if raw < 1.0 {
            "MEDIO"
        } else if raw < 1.5 {
            "BUENO"
        } else {
            "FUERTE"
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scanner_metrics_match_python_fixtures() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/legacy-scanner-metrics.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let convert = |value: &serde_json::Value| -> BTreeMap<u32, Minute> {
                value
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| {
                        let slot = k.parse::<u32>().unwrap();
                        (
                            slot,
                            Minute {
                                t: u64::from(slot) * 60_000,
                                o: 10.0,
                                h: 12.0,
                                l: 9.0,
                                c: 11.0,
                                v: v["volume"].as_f64().unwrap(),
                                vw: v["vwap"].as_f64(),
                            },
                        )
                    })
                    .collect()
            };
            let target = convert(&case["target"]);
            let baseline = convert(&case["baseline"]);
            let history = vec![baseline; case["baselineCount"].as_u64().unwrap() as usize];
            assert_eq!(
                vwap_at_slot(&target, 28),
                case["expectedVwap"].as_f64(),
                "{}",
                case["name"]
            );
            assert_eq!(
                relative_volume(&target, &history, 28).map(|v| v.value),
                case["expectedRvol"].as_f64(),
                "{}",
                case["name"]
            );
        }
    }
    #[test]
    fn python_fixtures_match_ohlcv_missing_vwap_and_regular_session_filter() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-bars.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let input: Vec<Minute> = serde_json::from_value(case["minutes"].clone()).unwrap();
            let actual = session_bars(
                &input,
                case["openMs"].as_u64().unwrap(),
                case["closeMs"].as_u64().unwrap(),
                2,
            );
            let expected = case["expected"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{}", case["name"]);
            for (a, e) in actual.iter().zip(expected) {
                let a = serde_json::to_value(a).unwrap();
                for key in [
                    "timestamp",
                    "open",
                    "high",
                    "low",
                    "close",
                    "volume",
                    "minutes_received",
                    "vwap",
                ] {
                    if e[key].is_null() {
                        assert!(a[key].is_null());
                    } else {
                        assert!(
                            (a[key].as_f64().unwrap() - e[key].as_f64().unwrap()).abs() < 1e-9,
                            "{} {key}",
                            case["name"]
                        );
                    }
                }
            }
        }
    }
    fn session(volume: f64) -> BTreeMap<u32, Minute> {
        (0..30)
            .map(|i| {
                (
                    i,
                    Minute {
                        t: u64::from(i) * 60_000,
                        o: 10.0,
                        h: 12.0,
                        l: 9.0,
                        c: 11.0,
                        v: volume,
                        vw: Some(10.5),
                    },
                )
            })
            .collect()
    }
    #[test]
    fn scanner_requires_coverage_final_bar_and_five_baselines() {
        let target = session(20.0);
        let history = vec![session(10.0); 5];
        assert_eq!(relative_volume(&target, &history, 28).unwrap().value, 2.0);
        assert!(relative_volume(&target, &history[..4], 28).is_none());
        let mut gap = target.clone();
        gap.remove(&29);
        assert!(cumulative_at_slot(&gap, 28).is_none());
        gap = target.clone();
        gap.remove(&1);
        assert!(cumulative_at_slot(&gap, 28).is_some());
        assert!(vwap_at_slot(&gap, 28).is_none());
        gap.remove(&2);
        assert!(cumulative_at_slot(&gap, 28).is_none());
        assert_eq!(vwap_at_slot(&target, 28), Some(10.5));
        assert!(cumulative_at_slot(&target, 26).is_none());
    }
}
