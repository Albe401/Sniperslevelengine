//! Experimental gross-premium observation. Inputs must come from certified reported trades.
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
    pub gross_premium: Option<f64>,
    pub median_multiple: Option<f64>,
    pub robust_z: Option<f64>,
    pub above_p95: Option<bool>,
    pub calibrated: bool,
    pub execution_enabled: bool,
}
fn pending(reason: &str) -> Reading {
    Reading {
        version: "option-premium-detector-v1".into(),
        status: "PENDING".into(),
        reason: reason.into(),
        key: None,
        sessions: 0,
        gross_premium: None,
        median_multiple: None,
        robust_z: None,
        above_p95: None,
        calibrated: false,
        execution_enabled: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reference::{Distribution, Sample};
    fn fixture() -> (Baseline, Sample) {
        let start = calendar::legacy_eastern_ms("2026-10-12 09:35:00").unwrap();
        let sample = Sample {
            underlying: "QQQ".into(),
            ticker: "O:QQQ261012C00600000".into(),
            kind: "call".into(),
            expiration: "2026-10-12".into(),
            strike: 600.,
            start_ms: start,
            end_ms: start + 300_000,
            underlying_price: 600.,
            price_available_ms: start,
            contracts: 1.,
            premium: Some(25_000.),
            measurement: "reported-trades".into(),
            source_collected_ms: start + 300_000,
            quality_complete: true,
        };
        let mut history = vec![];
        for day in 1..=30 {
            let date = format!("2026-09-{day:02}");
            let t = calendar::legacy_eastern_ms(&format!("{date} 09:35:00")).unwrap();
            if !matches!(
                calendar::schedule(calendar::date_at(t).unwrap()),
                calendar::Day::Open(_)
            ) {
                continue;
            }
            let mut s = sample.clone();
            s.ticker = format!("O:QQQ2609{day:02}C00600000");
            s.expiration = date;
            s.start_ms = t;
            s.end_ms = t + 300_000;
            s.price_available_ms = t;
            s.source_collected_ms = s.end_ms + 2000;
            s.premium = Some(10_000.);
            history.push(s);
            if history.len() == 20 {
                break;
            }
        }
        let built = calendar::legacy_eastern_ms("2026-10-10 10:00:00").unwrap();
        let mut baseline = reference::build(&history, start - 300_000, 20, built).unwrap();
        assert_eq!(baseline.cohorts.len(), 1);
        baseline.cohorts[0].premium = Some(Distribution {
            p25: 8000.,
            median: 10_000.,
            p75: 12_000.,
            p90: 15_000.,
            p95: 20_000.,
            mad: 2000.,
        });
        (baseline, sample)
    }
    #[test]
    fn detects_premium_without_requiring_unusual_volume() {
        let (b, s) = fixture();
        let p = evaluate(&b, &s, s.end_ms);
        assert_eq!(p.status, "UNUSUAL_PREMIUM_EXPERIMENTAL");
        assert_eq!(p.gross_premium, Some(25_000.));
        assert_eq!(p.median_multiple, Some(2.5));
        assert!(p.robust_z.unwrap() > 5.);
        assert!(!p.execution_enabled && !p.calibrated);
        assert_eq!(
            crate::option_activity_detector::evaluate(&b, &s, s.end_ms).status,
            "WITHIN_REFERENCE_EXPERIMENTAL"
        );
    }
    #[test]
    fn volume_spike_does_not_create_premium_anomaly() {
        let (b, mut s) = fixture();
        s.contracts = 200.;
        s.premium = Some(1000.);
        assert_eq!(
            evaluate(&b, &s, s.end_ms).status,
            "WITHIN_REFERENCE_EXPERIMENTAL"
        );
        assert_eq!(
            crate::option_activity_detector::evaluate(&b, &s, s.end_ms).status,
            "UNUSUAL_VOLUME_EXPERIMENTAL"
        );
    }
    #[test]
    fn threshold_boundary_requires_both_conditions() {
        let (mut b, mut s) = fixture();
        s.premium = Some(20_000.);
        assert_eq!(evaluate(&b, &s, s.end_ms).above_p95, Some(false));
        b.cohorts[0].premium.as_mut().unwrap().p95 = 15_000.;
        s.premium = Some(19_000.);
        assert_eq!(
            evaluate(&b, &s, s.end_ms).status,
            "WITHIN_REFERENCE_EXPERIMENTAL"
        );
        s.premium = Some(20_000.);
        assert_eq!(
            evaluate(&b, &s, s.end_ms).status,
            "UNUSUAL_PREMIUM_EXPERIMENTAL"
        );
    }
    #[test]
    fn bars_missing_premium_and_uncertified_source_are_pending() {
        let (b, mut s) = fixture();
        s.measurement = "eligible-option-bars".into();
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "unsupported-premium-measurement"
        );
        s.measurement = "reported-trades".into();
        s.premium = None;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "missing-certified-premium"
        );
        s.premium = Some(25_000.);
        s.quality_complete = false;
        assert_eq!(evaluate(&b, &s, s.end_ms).reason, "incomplete-source");
    }
    #[test]
    fn premium_readiness_does_not_use_volume_readiness() {
        let (mut b, s) = fixture();
        b.cohorts[0].volume_reference_ready = false;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).status,
            "UNUSUAL_PREMIUM_EXPERIMENTAL"
        );
        b.cohorts[0].premium_reference_ready = false;
        b.cohorts[0].volume_reference_ready = true;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "insufficient-or-zero-reference"
        );
        b.cohorts[0].premium = None;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "missing-premium-reference"
        );
    }
    #[test]
    fn zero_premium_and_zero_mad_never_invent_a_scale() {
        let (mut b, mut s) = fixture();
        b.cohorts[0].premium.as_mut().unwrap().mad = 0.;
        assert_eq!(evaluate(&b, &s, s.end_ms).robust_z, None);
        s.premium = Some(0.);
        assert_eq!(evaluate(&b, &s, s.end_ms).median_multiple, Some(0.));
        b.cohorts[0].premium = Some(Distribution {
            p25: 0.,
            median: 0.,
            p75: 0.,
            p90: 0.,
            p95: 0.,
            mad: 0.,
        });
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "insufficient-or-zero-reference"
        );
    }
    #[test]
    fn historical_availability_and_future_prices_remain_strict() {
        let (mut b, mut s) = fixture();
        b.built_ms = s.start_ms + 1;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "reference-not-available-before-window"
        );
        b.built_ms = b.cutoff_ms - 1;
        s.price_available_ms = s.start_ms + 1;
        assert_eq!(evaluate(&b, &s, s.end_ms).status, "PENDING");
        s.price_available_ms = s.start_ms;
        assert_eq!(evaluate(&b, &s, s.end_ms - 1).status, "PENDING");
        b.cohorts[0].dates[0] = "2026-10-12".into();
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "target-or-future-reference-session"
        );
    }
    #[test]
    fn invalid_distribution_duplicate_dates_and_low_sample_count_are_pending() {
        let (mut b, s) = fixture();
        b.cohorts[0].premium.as_mut().unwrap().mad = f64::NAN;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "invalid-reference-distribution"
        );
        b.cohorts[0].premium.as_mut().unwrap().mad = 2000.;
        b.cohorts[0].dates[0] = b.cohorts[0].dates[1].clone();
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "invalid-reference-distribution"
        );
        let (mut b, s) = fixture();
        b.cohorts[0].dates.pop();
        b.cohorts[0].sessions = 19;
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "insufficient-or-zero-reference"
        );
    }
    #[test]
    fn different_side_and_ambiguous_cohorts_do_not_compare() {
        let (mut b, mut s) = fixture();
        s.kind = "put".into();
        s.ticker = "O:QQQ261012P00600000".into();
        assert_eq!(evaluate(&b, &s, s.end_ms).reason, "no-comparable-cohort");
        s.kind = "call".into();
        s.ticker = "O:QQQ261012C00600000".into();
        b.cohorts.push(b.cohorts[0].clone());
        assert_eq!(
            evaluate(&b, &s, s.end_ms).reason,
            "ambiguous-reference-cohort"
        );
    }
}
/// Certified gross premium for a closed window, with a prior comparable reference.
/// No contract-volume distribution or option-bar VWAP is used.
/// Provisional premium policy: strictly above premium p95 AND twice premium median.
/// No z threshold: zero MAD leaves robust_z absent rather than inventing a scale.
pub fn evaluate(baseline: &Baseline, sample: &Sample, now_ms: u64) -> Reading {
    if sample.measurement != "reported-trades" {
        return pending("unsupported-premium-measurement");
    }
    let Some(amount) = sample.premium else {
        return pending("missing-certified-premium");
    };
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
    let Some(v) = c.premium.as_ref() else {
        return pending("missing-premium-reference");
    };
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
    if !c.premium_reference_ready || c.sessions < baseline.minimum_sessions || v.median <= 0. {
        return pending("insufficient-or-zero-reference");
    }
    let multiple = amount / v.median;
    if !multiple.is_finite() {
        return pending("nonfinite-comparison");
    }
    let z = if v.mad > 0. {
        let z = 0.6744897501960817 * ((amount - v.median) / v.mad);
        z.is_finite().then_some(z)
    } else {
        None
    };
    let above = amount > v.p95;
    let unusual = above && multiple >= 2.;
    Reading {
        version: "option-premium-detector-v1".into(),
        status: if unusual {
            "UNUSUAL_PREMIUM_EXPERIMENTAL"
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
        gross_premium: Some(amount),
        median_multiple: Some(multiple),
        robust_z: z,
        above_p95: Some(above),
        calibrated: false,
        execution_enabled: false,
    }
}
