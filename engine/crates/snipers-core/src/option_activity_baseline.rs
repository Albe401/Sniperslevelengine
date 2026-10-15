//! Versioned historical reference only. No anomaly thresholds, directional weights or entries.
use crate::{
    calendar::{self, Day},
    options,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub underlying: String,
    pub ticker: String,
    pub kind: String,
    pub expiration: String,
    pub strike: f64,
    pub start_ms: u64,
    pub end_ms: u64,
    pub underlying_price: f64,
    /// End of a closed stock minute, never a future/in-progress bar.
    pub price_available_ms: u64,
    pub contracts: f64,
    pub premium: Option<f64>,
    pub measurement: String,
    pub source_collected_ms: u64,
    pub quality_complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct CohortKey {
    pub underlying: String,
    pub kind: String,
    pub days_to_expiration: i64,
    pub minute_offset: u64,
    pub window_minutes: u64,
    pub session_minutes: u64,
    /// Signed strike / contemporaneous underlying price distance, in percent.
    pub moneyness_band: String,
    pub measurement: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Distribution {
    pub p25: f64,
    pub median: f64,
    pub p75: f64,
    pub p90: f64,
    pub p95: f64,
    pub mad: f64,
}
fn quantile(values: &[f64], p: f64) -> f64 {
    let position = (values.len() - 1) as f64 * p;
    let low = position.floor() as usize;
    let high = position.ceil() as usize;
    values[low] + (values[high] - values[low]) * (position - low as f64)
}
fn distribution(mut values: Vec<f64>) -> Distribution {
    values.sort_by(f64::total_cmp);
    let median = quantile(&values, 0.5);
    let mut deviations: Vec<_> = values.iter().map(|v| (v - median).abs()).collect();
    deviations.sort_by(f64::total_cmp);
    Distribution {
        p25: quantile(&values, 0.25),
        median,
        p75: quantile(&values, 0.75),
        p90: quantile(&values, 0.9),
        p95: quantile(&values, 0.95),
        mad: quantile(&deviations, 0.5),
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cohort {
    pub key: CohortKey,
    pub dates: Vec<String>,
    pub sessions: usize,
    pub contract_windows: usize,
    pub status: String,
    pub volume_reference_ready: bool,
    pub premium_reference_ready: bool,
    pub volume: Distribution,
    pub premium: Option<Distribution>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Baseline {
    pub version: String,
    pub built_ms: u64,
    pub cutoff_ms: u64,
    pub available_before_cutoff: bool,
    pub minimum_sessions: usize,
    pub policy: String,
    pub status: String,
    pub calibrated: bool,
    pub execution_enabled: bool,
    pub accepted_samples: usize,
    pub duplicates: usize,
    pub conflicting_samples: usize,
    pub excluded: BTreeMap<String, usize>,
    pub cohorts: Vec<Cohort>,
}
pub(crate) fn cohort(sample: &Sample, cutoff: u64, now: u64) -> Result<(CohortKey, String), &'static str> {
    if !sample.quality_complete {
        return Err("incomplete-source");
    }
    if sample.source_collected_ms < sample.end_ms || sample.source_collected_ms > now {
        return Err("future-or-missing-collection-clock");
    }
    let date = calendar::date_at(sample.start_ms).ok_or("invalid-session")?;
    let session = match calendar::schedule(date) {
        Day::Open(s) => s,
        _ => return Err("invalid-session"),
    };
    let target_date = calendar::date_at(cutoff).ok_or("invalid-cutoff")?;
    if date >= target_date || sample.end_ms >= cutoff {
        return Err("target-or-future-session");
    }
    if sample.start_ms < session.open_ms
        || sample.end_ms > session.close_ms
        || sample.start_ms >= sample.end_ms
        || !(sample.start_ms - session.open_ms).is_multiple_of(60_000)
        || ![60_000, 300_000, 900_000, 1_800_000].contains(&(sample.end_ms - sample.start_ms))
    {
        return Err("invalid-window");
    }
    if sample.price_available_ms > sample.start_ms
        || sample.start_ms - sample.price_available_ms > 60_000
        || sample.price_available_ms <= session.open_ms
    {
        return Err("future-stale-or-missing-underlying-price");
    }
    if options::underlying(&sample.ticker) != Some(sample.underlying.as_str())
        || !["call", "put"].contains(&sample.kind.as_str())
        || ![sample.strike, sample.underlying_price]
            .iter()
            .all(|n| n.is_finite() && *n > 0.)
        || !sample.contracts.is_finite()
        || sample.contracts < 0.
        || sample.premium.is_some_and(|n| !n.is_finite() || n < 0.)
        || !["reported-trades", "eligible-option-bars"].contains(&sample.measurement.as_str())
        || (sample.measurement == "eligible-option-bars" && sample.premium.is_some())
    {
        return Err("invalid-contract-or-metrics");
    }
    let raw = sample
        .ticker
        .strip_prefix("O:")
        .ok_or("invalid-contract-or-metrics")?;
    let tail = &raw[raw.len() - 15..];
    let ticker_expiry = format!("20{}-{}-{}", &tail[..2], &tail[2..4], &tail[4..6]);
    let ticker_kind = if &tail[6..7] == "C" { "call" } else { "put" };
    let ticker_strike = tail[7..]
        .parse::<u64>()
        .map_err(|_| "invalid-contract-or-metrics")? as f64
        / 1000.;
    if sample.expiration != ticker_expiry
        || sample.kind != ticker_kind
        || sample.strike != ticker_strike
    {
        return Err("contract-metadata-mismatch");
    }
    let (_, expiry_end) = calendar::day_bounds(&sample.expiration).ok_or("invalid-expiration")?;
    let expiry = calendar::date_at(expiry_end - 1).ok_or("invalid-expiration")?;
    let dte = expiry.signed_duration_since(date).num_days();
    if !(0..=30).contains(&dte) {
        return Err("expiration-outside-cohort-policy");
    }
    let distance = 100. * (sample.strike / sample.underlying_price - 1.);
    if !distance.is_finite() || distance.abs() > 3. {
        return Err("moneyness-outside-cohort-policy");
    }
    let band = if distance < -1. {
        "[-3,-1)"
    } else if distance < -0.25 {
        "[-1,-0.25)"
    } else if distance < 0.25 {
        "[-0.25,0.25)"
    } else if distance < 1. {
        "[0.25,1)"
    } else {
        "[1,3]"
    };
    Ok((
        CohortKey {
            underlying: sample.underlying.clone(),
            kind: sample.kind.clone(),
            days_to_expiration: dte,
            minute_offset: (sample.start_ms - session.open_ms) / 60_000,
            window_minutes: (sample.end_ms - sample.start_ms) / 60_000,
            session_minutes: (session.close_ms - session.open_ms) / 60_000,
            moneyness_band: band.into(),
            measurement: sample.measurement.clone(),
        },
        date.to_string(),
    ))
}
fn same_observation(a: &Sample, b: &Sample) -> bool {
    a.underlying == b.underlying
        && a.kind == b.kind
        && a.expiration == b.expiration
        && a.strike == b.strike
        && a.underlying_price == b.underlying_price
        && a.price_available_ms == b.price_available_ms
        && a.contracts == b.contracts
        && a.premium == b.premium
        && a.measurement == b.measurement
}
/// One median observation per distinct session prevents overweighting repeated contracts/retries.
pub fn build(
    samples: &[Sample],
    cutoff: u64,
    minimum_sessions: usize,
    now: u64,
) -> Result<Baseline, &'static str> {
    let cutoff_valid=calendar::date_at(cutoff).is_some_and(|date|
        matches!(calendar::schedule(date),Day::Open(s) if cutoff>=s.open_ms&&cutoff<s.close_ms));
    if !(20..=252).contains(&minimum_sessions) || !cutoff_valid || now == 0 {
        return Err("Invalid baseline configuration");
    }
    let mut excluded = BTreeMap::new();
    let mut unique: BTreeMap<(String, String, u64, u64), Sample> = BTreeMap::new();
    let mut conflicts = BTreeSet::new();
    let mut duplicates = 0;
    for s in samples {
        if let Err(reason) = cohort(s, cutoff, now) {
            *excluded.entry(reason.into()).or_default() += 1;
            continue;
        }
        let id = (
            s.ticker.clone(),
            s.measurement.clone(),
            s.start_ms,
            s.end_ms,
        );
        if conflicts.contains(&id) {
            continue;
        }
        if let Some(old) = unique.get_mut(&id) {
            if same_observation(old, s) {
                duplicates += 1;
                old.source_collected_ms = old.source_collected_ms.min(s.source_collected_ms);
            } else {
                unique.remove(&id);
                conflicts.insert(id);
            }
        } else {
            unique.insert(id, s.clone());
        }
    }
    let mut grouped: BTreeMap<CohortKey, BTreeMap<String, Vec<Sample>>> = BTreeMap::new();
    for s in unique.values() {
        let (key, date) = cohort(s, cutoff, now)?;
        grouped
            .entry(key)
            .or_default()
            .entry(date)
            .or_default()
            .push(s.clone());
    }
    let mut cohorts = vec![];
    for (key, dates) in grouped {
        let volume = distribution(
            dates
                .values()
                .map(|rows| distribution(rows.iter().map(|s| s.contracts).collect()).median)
                .collect(),
        );
        let premium = if dates
            .values()
            .all(|rows| rows.iter().all(|s| s.premium.is_some()))
        {
            Some(distribution(
                dates
                    .values()
                    .map(|rows| {
                        distribution(rows.iter().map(|s| s.premium.unwrap()).collect()).median
                    })
                    .collect(),
            ))
        } else {
            None
        };
        let enough = dates.len() >= minimum_sessions;
        let status = if !enough {
            "PENDING_SAMPLES"
        } else if volume.median <= 0. && premium.as_ref().is_none_or(|p| p.median <= 0.) {
            "PENDING_ZERO_REFERENCE"
        } else {
            "REFERENCE_AVAILABLE_EXPERIMENTAL"
        };
        cohorts.push(Cohort {
            key,
            sessions: dates.len(),
            contract_windows: dates.values().map(Vec::len).sum(),
            dates: dates.keys().cloned().collect(),
            status: status.into(),
            volume_reference_ready: enough && volume.median > 0.,
            premium_reference_ready: enough && premium.as_ref().is_some_and(|p| p.median > 0.),
            volume,
            premium,
        });
    }
    let available = now < cutoff && unique.values().all(|s| s.source_collected_ms < cutoff);
    let any = cohorts
        .iter()
        .any(|c| c.volume_reference_ready || c.premium_reference_ready);
    Ok(Baseline {version:"option-activity-baseline-v1".into(),built_ms:now,cutoff_ms:cutoff,available_before_cutoff:available,
        minimum_sessions,policy:"signed-moneyness-v1; exact-DTE; exact-ET-window; exact-session-length; session-median; SIP-RTH".into(),
        status:if any{"EXPERIMENTAL_REFERENCE"}else{"PENDING_SAMPLES"}.into(),calibrated:false,execution_enabled:false,
        accepted_samples:unique.len(),duplicates,conflicting_samples:conflicts.len(),excluded,cohorts})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn samples() -> Vec<Sample> {
        let mut result = vec![];
        let mut date =
            calendar::date_at(calendar::rfc3339_ms("2026-09-01T14:00:00Z").unwrap()).unwrap();
        while result.len() < 20 {
            if let Day::Open(s) = calendar::schedule(date) {
                result.push(Sample {
                    underlying: "QQQ".into(),
                    ticker: format!("O:QQQ{}C00500000", s.date[2..].replace('-', "")),
                    kind: "call".into(),
                    expiration: s.date.clone(),
                    strike: 500.,
                    start_ms: s.open_ms + 300_000,
                    end_ms: s.open_ms + 600_000,
                    underlying_price: 500.,
                    price_available_ms: s.open_ms + 300_000,
                    contracts: 10.,
                    premium: Some(1000.),
                    measurement: "reported-trades".into(),
                    source_collected_ms: s.close_ms + 1000,
                    quality_complete: true,
                });
            }
            date = date.succ_opt().unwrap();
        }
        result
    }
    fn now() -> u64 {
        calendar::rfc3339_ms("2026-10-10T12:00:00Z").unwrap()
    }
    fn cutoff() -> u64 {
        calendar::legacy_eastern_ms("2026-10-12 09:30:00").unwrap()
    }
    #[test]
    fn enough_distinct_sessions_produce_robust_reference_without_enabling_execution() {
        let mut s = samples();
        s[19].contracts = 10000.;
        s[19].premium = Some(1000000.);
        let r = build(&s, cutoff(), 20, now()).unwrap();
        assert_eq!(r.cohorts[0].volume.median, 10.);
        assert_eq!(r.cohorts[0].volume.mad, 0.);
        assert!(r.cohorts[0].volume_reference_ready);
        assert!(r.available_before_cutoff);
        assert!(!r.calibrated);
        assert!(!r.execution_enabled);
    }
    #[test]
    fn retries_do_not_inflate_session_count_and_conflicts_are_excluded() {
        let s = samples();
        let mut repeated = s.clone();
        repeated.extend(s.clone());
        let r = build(&repeated, cutoff(), 20, now()).unwrap();
        assert_eq!(r.duplicates, 20);
        assert_eq!(r.cohorts[0].sessions, 20);
        let mut bad = s[0].clone();
        bad.premium = Some(2.);
        repeated.push(bad);
        let r = build(&repeated, cutoff(), 20, now()).unwrap();
        assert_eq!(r.conflicting_samples, 1);
        assert_eq!(r.cohorts[0].sessions, 19);
        assert!(!r.cohorts[0].volume_reference_ready);
    }
    #[test]
    fn rejects_future_prices_incomplete_sources_and_target_session() {
        let mut s = samples();
        s[0].price_available_ms = s[0].start_ms + 1;
        s[1].quality_complete = false;
        let r = build(&s, s[19].start_ms, 20, now()).unwrap();
        assert_eq!(r.accepted_samples, 17);
        assert!(!r.available_before_cutoff);
        assert!(r.excluded.contains_key("target-or-future-session"));
    }
    #[test]
    fn separates_side_dte_moneyness_and_time_of_day() {
        let s = samples();
        let mut more = s.clone();
        let mut extra = s[0].clone();
        extra.ticker = extra.ticker.replace('C', "P");
        extra.kind = "put".into();
        more.push(extra);
        let r = build(&more, cutoff(), 20, now()).unwrap();
        assert_eq!(r.cohorts.len(), 2);
        assert_eq!(
            r.cohorts
                .iter()
                .find(|c| c.key.kind == "put")
                .unwrap()
                .sessions,
            1
        );
    }
    #[test]
    fn zero_reference_and_short_samples_remain_pending() {
        let mut s = samples();
        for row in &mut s {
            row.contracts = 0.;
            row.premium = Some(0.);
        }
        let r = build(&s, cutoff(), 20, now()).unwrap();
        assert!(!r.cohorts[0].volume_reference_ready);
        assert_eq!(r.cohorts[0].status, "PENDING_ZERO_REFERENCE");
        assert!(build(&s, cutoff(), 5, now()).is_err());
    }
    #[test]
    fn aggregate_volume_does_not_invent_premium_or_mix_with_prints() {
        let s = samples();
        let mut bars = s.clone();
        for row in &mut bars {
            row.measurement = "eligible-option-bars".into();
            row.premium = None;
            row.contracts = 20.;
        }
        bars.extend(s);
        let r = build(&bars, cutoff(), 20, now()).unwrap();
        assert_eq!(r.cohorts.len(), 2);
        let c = r
            .cohorts
            .iter()
            .find(|c| c.key.measurement == "eligible-option-bars")
            .unwrap();
        assert!(c.volume_reference_ready);
        assert!(!c.premium_reference_ready);
        assert!(c.premium.is_none());
        assert_eq!(c.volume.median, 20.);
        assert_eq!(r.conflicting_samples, 0);
    }
    #[test]
    fn invalid_cutoff_and_contract_metadata_are_rejected() {
        let s = samples();
        assert!(build(
            &s,
            calendar::rfc3339_ms("2026-10-10T14:00:00Z").unwrap(),
            20,
            now()
        )
        .is_err());
        let mut bad = s;
        bad[0].strike = 501.;
        let r = build(&bad, cutoff(), 20, now()).unwrap();
        assert_eq!(r.excluded["contract-metadata-mismatch"], 1);
    }
    #[test]
    fn several_contracts_per_day_do_not_replace_distinct_sessions() {
        let s = samples();
        let mut both = s.clone();
        for mut row in s {
            row.ticker = row.ticker.replace("C00500000", "C00500500");
            row.strike = 500.5;
            row.contracts = 100.;
            both.push(row);
        }
        let r = build(&both, cutoff(), 20, now()).unwrap();
        assert_eq!(r.cohorts.len(), 1);
        assert_eq!(r.cohorts[0].sessions, 20);
        assert_eq!(r.cohorts[0].contract_windows, 40);
        assert_eq!(r.cohorts[0].volume.median, 55.);
    }
}
