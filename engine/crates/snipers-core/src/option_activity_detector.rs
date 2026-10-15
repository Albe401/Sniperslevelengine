//! Experimental observation only. Comparable volume is not trade direction or probability.
use crate::{
    calendar,
    option_activity_baseline::{self as reference, Baseline, CohortKey, Sample},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub version: String,
    pub status: String,
    pub reason: String,
    pub key: Option<CohortKey>,
    pub sessions: usize,
    pub contracts: Option<f64>,
    pub median_multiple: Option<f64>,
    pub robust_z: Option<f64>,
    pub above_p95: Option<bool>,
    pub calibrated: bool,
    pub execution_enabled: bool,
}
fn pending(reason: &str) -> Reading {
    Reading {
        version: "option-activity-detector-v1".into(),
        status: "PENDING".into(),
        reason: reason.into(),
        key: None,
        sessions: 0,
        contracts: None,
        median_multiple: None,
        robust_z: None,
        above_p95: None,
        calibrated: false,
        execution_enabled: false,
    }
}

/// Closed-window observation, using only a reference available before that window.
/// Provisional unusual policy: strictly above p95 AND at least twice median.
/// No z threshold: zero MAD leaves robust_z absent rather than inventing a scale.
pub fn evaluate(baseline: &Baseline, sample: &Sample, now_ms: u64) -> Reading {
    let Some(validation_cutoff) = sample.end_ms.checked_add(86_400_000) else {
        return pending("invalid-window-clock");
    };
    let (key, date) = match reference::cohort(sample, validation_cutoff, now_ms) {
        Ok(value) => value,
        Err(reason) => return pending(reason),
    };
    if baseline.version != "option-activity-baseline-v1"
        || baseline.execution_enabled
        || baseline.minimum_sessions < 20
        || baseline.minimum_sessions > 252
    {
        return pending("invalid-reference-policy");
    }
    if !baseline.available_before_cutoff
        || baseline.built_ms >= baseline.cutoff_ms
        || baseline.cutoff_ms > sample.start_ms
        || baseline.built_ms > sample.start_ms
    {
        return pending("reference-not-available-before-window");
    }
    let Some(cutoff_date) = calendar::date_at(baseline.cutoff_ms) else {
        return pending("invalid-reference-cutoff");
    };
    if !matches!(calendar::schedule(cutoff_date), calendar::Day::Open(s)
        if baseline.cutoff_ms >= s.open_ms && baseline.cutoff_ms < s.close_ms)
    {
        return pending("invalid-reference-cutoff");
    }
    let cutoff_date = cutoff_date.to_string();
    // Validate the entire reference's dates, not only the matched cohort.
    if baseline.cohorts.iter().any(|c| {
        c.dates
            .iter()
            .any(|d| calendar::day_bounds(d).is_none() || d >= &date || d >= &cutoff_date)
    }) {
        return pending("target-or-future-reference-session");
    }
    let matches: Vec<_> = baseline.cohorts.iter().filter(|c| c.key == key).collect();
    if matches.len() != 1 {
        return pending(if matches.is_empty() {
            "no-comparable-cohort"
        } else {
            "ambiguous-reference-cohort"
        });
    }
    let c = matches[0];
    let dates: BTreeSet<_> = c.dates.iter().collect();
    let v = &c.volume;
    let ordered = [v.p25, v.median, v.p75, v.p90, v.p95];
    if c.sessions != dates.len()
        || c.sessions != c.dates.len()
        || ordered.iter().any(|n| !n.is_finite() || *n < 0.)
        || ordered.windows(2).any(|w| w[0] > w[1])
        || !v.mad.is_finite()
        || v.mad < 0.
    {
        return pending("invalid-reference-distribution");
    }
    if !c.volume_reference_ready || c.sessions < baseline.minimum_sessions || v.median <= 0. {
        return pending("insufficient-or-zero-reference");
    }
    let multiple = sample.contracts / v.median;
    if !multiple.is_finite() {
        return pending("nonfinite-comparison");
    }
    let z = if v.mad > 0. {
        let z = 0.6744897501960817 * ((sample.contracts - v.median) / v.mad);
        z.is_finite().then_some(z)
    } else {
        None
    };
    let above = sample.contracts > v.p95;
    let unusual = above && multiple >= 2.;
    Reading {
        version: "option-activity-detector-v1".into(),
        status: if unusual {
            "UNUSUAL_VOLUME_EXPERIMENTAL"
        } else {
            "WITHIN_REFERENCE_EXPERIMENTAL"
        }
        .into(),
        reason: if unusual {
            "above-p95-and-at-least-two-times-median"
        } else {
            "provisional-rule-not-met"
        }
        .into(),
        key: Some(key),
        sessions: c.sessions,
        contracts: Some(sample.contracts),
        median_multiple: Some(multiple),
        robust_z: z,
        above_p95: Some(above),
        calibrated: false,
        execution_enabled: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reference::{Cohort, Distribution};
    fn fixture() -> (Baseline, Sample) {
        let start = calendar::legacy_eastern_ms("2026-10-12 09:35:00").unwrap();
        let s = Sample {
            underlying: "QQQ".into(),
            ticker: "O:QQQ261012C00600000".into(),
            kind: "call".into(),
            expiration: "2026-10-12".into(),
            strike: 600.,
            start_ms: start,
            end_ms: start + 300_000,
            underlying_price: 600.,
            price_available_ms: start,
            contracts: 250.,
            premium: None,
            measurement: "eligible-option-bars".into(),
            source_collected_ms: start + 300_000,
            quality_complete: true,
        };
        let key = reference::cohort(&s, s.end_ms + 86_400_000, s.end_ms)
            .unwrap()
            .0;
        let dates = (1..=30)
            .filter_map(|day| {
                let d = format!("2026-09-{day:02}");
                let (open, _) = calendar::day_bounds(&d)?;
                match calendar::schedule(calendar::date_at(open)?) {
                    calendar::Day::Open(_) => Some(d),
                    _ => None,
                }
            })
            .take(20)
            .collect::<Vec<_>>();
        let b = Baseline {
            version: "option-activity-baseline-v1".into(),
            built_ms: start - 86_400_000,
            cutoff_ms: start - 300_000,
            available_before_cutoff: true,
            minimum_sessions: 20,
            policy: "experimental".into(),
            status: "EXPERIMENTAL_REFERENCE".into(),
            calibrated: false,
            execution_enabled: false,
            accepted_samples: 20,
            duplicates: 0,
            conflicting_samples: 0,
            excluded: Default::default(),
            cohorts: vec![Cohort {
                key,
                sessions: dates.len(),
                dates,
                contract_windows: 20,
                status: "REFERENCE_AVAILABLE_EXPERIMENTAL".into(),
                volume_reference_ready: true,
                premium_reference_ready: false,
                premium: None,
                volume: Distribution {
                    p25: 80.,
                    median: 100.,
                    p75: 120.,
                    p90: 150.,
                    p95: 200.,
                    mad: 20.,
                },
            }],
        };
        (b, s)
    }
    #[test]
    fn unusual_is_observation_only() {
        let (b, s) = fixture();
        let r = evaluate(&b, &s, s.end_ms);
        assert_eq!(r.status, "UNUSUAL_VOLUME_EXPERIMENTAL");
        assert_eq!(r.median_multiple, Some(2.5));
        assert!(r.robust_z.unwrap() > 5.);
        assert!(!r.execution_enabled && !r.calibrated);
    }
    #[test]
    fn p95_boundary_and_double_median_are_both_required() {
        let (mut b, mut s) = fixture();
        s.contracts = 200.;
        assert_eq!(evaluate(&b, &s, s.end_ms).above_p95, Some(false));
        b.cohorts[0].volume.p95 = 150.;
        s.contracts = 190.;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).status,
            "WITHIN_REFERENCE_EXPERIMENTAL"
        );
        s.contracts = 200.;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).status,
            "UNUSUAL_VOLUME_EXPERIMENTAL"
        );
    }
    #[test]
    fn zero_mad_and_zero_volume_remain_finite() {
        let (mut b, mut s) = fixture();
        b.cohorts[0].volume.mad = 0.;
        assert_eq!(evaluate(&b, &s, s.end_ms).robust_z, None);
        s.contracts = 0.;
        assert_eq!(evaluate(&b, &s, s.end_ms).median_multiple, Some(0.));
    }
    #[test]
    fn wrong_measurement_side_and_missing_cohort_are_pending() {
        let (b, mut s) = fixture();
        s.measurement = "reported-trades".into();
        assert_eq!(evaluate(&b, &s, s.end_ms).reason, "no-comparable-cohort");
        s.measurement = "eligible-option-bars".into();
        s.kind = "put".into();
        s.ticker = "O:QQQ261012P00600000".into();
        assert_eq!(evaluate(&b, &s, s.end_ms).reason, "no-comparable-cohort");
    }
    #[test]
    fn incomplete_future_price_and_open_window_are_pending() {
        let (b, mut s) = fixture();
        s.quality_complete = false;
        assert_eq!(evaluate(&b, &s, s.end_ms).status, "PENDING");
        s.quality_complete = true;
        s.price_available_ms = s.start_ms + 1;
        assert_eq!(evaluate(&b, &s, s.end_ms).status, "PENDING");
        s.price_available_ms = s.start_ms;
        assert_eq!(evaluate(&b, &s, s.end_ms - 1).status, "PENDING");
    }
    #[test]
    fn future_reference_and_target_contamination_are_pending() {
        let (mut b, s) = fixture();
        b.built_ms = s.start_ms + 1;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "reference-not-available-before-window"
        );
        b.built_ms = s.start_ms - 86_400_000;
        b.cohorts[0].dates[0] = "2026-10-12".into();
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "target-or-future-reference-session"
        );
    }
    #[test]
    fn malformed_insufficient_and_duplicate_reference_are_pending() {
        let (mut b, s) = fixture();
        b.cohorts[0].volume.mad = f64::NAN;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "invalid-reference-distribution"
        );
        b.cohorts[0].volume.mad = 20.;
        b.minimum_sessions = 21;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "insufficient-or-zero-reference"
        );
        b.minimum_sessions = 20;
        b.cohorts.push(b.cohorts[0].clone());
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "ambiguous-reference-cohort"
        );
    }
    #[test]
    fn weekend_cutoff_is_not_a_valid_reference() {
        let (mut b, s) = fixture();
        b.cutoff_ms = calendar::legacy_eastern_ms("2026-10-11 09:30:00").unwrap();
        b.built_ms = b.cutoff_ms - 1;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "invalid-reference-cutoff"
        );
    }
}
