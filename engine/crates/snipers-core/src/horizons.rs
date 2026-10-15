//! Legacy horizons: FOUR_WEEKS actually means next four expirations, not 28 days.
use crate::{
    calendar::date_at,
    exposure::{Contract, Exposure, Model},
    options::Observation,
};
use chrono::{Datelike, NaiveDate, TimeZone};
use chrono_tz::America::New_York;
use serde::Serialize;
use std::collections::BTreeSet;

pub fn friday_at(ms: u64) -> Option<String> {
    let today = date_at(ms)?;
    Some(
        (today
            + chrono::Duration::days(
                (4 - today.weekday().num_days_from_monday() as i64).rem_euclid(7),
            ))
        .to_string(),
    )
}
/// Model convention inherited from Python, not a contract's official trading cutoff.
/// UTC duration handles DST transitions; expired/invalid dates never get a minimum T.
pub fn model_years(expiration: &str, now_ms: u64) -> Option<f64> {
    let date = NaiveDate::parse_from_str(expiration, "%Y-%m-%d").ok()?;
    let expiry = New_York
        .from_local_datetime(&date.and_hms_opt(16, 0, 0)?)
        .single()?
        .timestamp_millis();
    let remaining = expiry.checked_sub(i64::try_from(now_ms).ok()?)?;
    (remaining > 0).then_some(remaining as f64 / (365.0 * 24.0 * 60.0 * 60.0 * 1000.0))
}
/// SPXW PM settles at the core session close, including 13:00 ET half days.
pub fn spx_pm_years(expiration: &str, now_ms: u64) -> Option<f64> {
    let day = NaiveDate::parse_from_str(expiration, "%Y-%m-%d").ok()?;
    let crate::calendar::Day::Open(session) = crate::calendar::schedule(day) else {
        return None;
    };
    let remaining = session.close_ms.checked_sub(now_ms)?;
    (remaining > 0).then_some(remaining as f64 / (365. * 24. * 60. * 60. * 1000.))
}
pub fn select(expirations: &[String], mode: &str, now_ms: u64) -> Vec<String> {
    let Some(today) = date_at(now_ms).map(|d| d.to_string()) else {
        return vec![];
    };
    let friday = friday_at(now_ms).unwrap();
    let dates: BTreeSet<_> = expirations
        .iter()
        .filter(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok() && *d >= &today)
        .cloned()
        .collect();
    match mode {
        "DAY" | "0DTE" => dates.into_iter().filter(|d| d == &today).collect(),
        "NEXT_SESSION" => dates.into_iter().filter(|d| d > &today).take(1).collect(),
        "WEEK" => dates.into_iter().filter(|d| d <= &friday).collect(),
        "NEXT4" | "FOUR_WEEKS" => dates.into_iter().take(4).collect(),
        _ => vec![],
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Horizon {
    pub mode: String,
    pub label: String,
    pub expirations: Vec<String>,
    pub observed_ms: u64,
    pub reference_source: String,
    pub exposure: Exposure,
    pub structure: crate::structure::Structure,
}
pub fn calculate(
    observation: &Observation,
    spot: f64,
    source: &str,
) -> Result<Vec<Horizon>, &'static str> {
    calculate_with_history(observation, spot, source, None)
}
pub fn calculate_with_history(
    observation: &Observation,
    spot: f64,
    source: &str,
    previous: Option<&Observation>,
) -> Result<Vec<Horizon>, &'static str> {
    let dates: Vec<_> = observation
        .contracts
        .values()
        .map(|c| c.expiration.clone())
        .collect();
    let mut result = Vec::new();
    for requested in ["DAY", "WEEK", "NEXT4"] {
        let mut mode = requested;
        let mut selected = select(&dates, mode, observation.observed_ms);
        if mode == "DAY" && selected.is_empty() {
            mode = "NEXT_SESSION";
            selected = select(&dates, mode, observation.observed_ms);
        }
        let contracts: Vec<_> = observation
            .contracts
            .values()
            .filter(|c| selected.contains(&c.expiration))
            .map(|c| Contract {
                kind: c.kind.clone(),
                strike: c.strike,
                oi: c.open_interest,
                gamma: c.gamma,
                delta: c.delta,
                iv: c.iv,
                multiplier: c.premium_multiplier,
                years: if observation.symbol == "SPX" {
                    spx_pm_years(&c.expiration, observation.observed_ms)
                } else {
                    model_years(&c.expiration, observation.observed_ms)
                },
            })
            .collect();
        let structure = crate::structure::calculate(observation, previous, &selected, spot)?;
        result.push(Horizon {
            mode: mode.into(),
            label: match mode {
                "DAY" => "Hoy / 0DTE",
                "NEXT_SESSION" => "Próximo vencimiento (sin 0DTE)",
                "WEEK" => "Hasta el viernes",
                _ => "Próximos 4 vencimientos",
            }
            .into(),
            expirations: selected,
            observed_ms: observation.observed_ms,
            reference_source: source.into(),
            exposure: Model::default().calculate(&contracts, spot)?,
            structure,
        });
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    #[test]
    fn spx_pm_respects_half_day_settlement_and_unknown_dates() {
        let t = crate::calendar::legacy_eastern_ms("2026-11-27 13:00:00").unwrap();
        assert!(spx_pm_years("2026-11-27", t - 1).is_some());
        assert_eq!(spx_pm_years("2026-11-27", t), None);
        assert_eq!(spx_pm_years("2026-11-28", t), None);
        assert_eq!(spx_pm_years("2029-11-27", t), None);
    }
    use super::*;
    fn time(date: &str, hour: u32) -> u64 {
        New_York
            .from_local_datetime(
                &NaiveDate::parse_from_str(date, "%Y-%m-%d")
                    .unwrap()
                    .and_hms_opt(hour, 0, 0)
                    .unwrap(),
            )
            .single()
            .unwrap()
            .timestamp_millis() as u64
    }
    #[test]
    fn horizons_use_next_four_expirations_and_weekend_friday_not_twenty_eight_days() {
        let dates: Vec<_> = [
            "2026-09-23",
            "2026-09-24",
            "2026-09-25",
            "2026-09-28",
            "2026-09-29",
            "2026-09-30",
        ]
        .map(str::to_owned)
        .into();
        let now = time("2026-09-24", 10);
        assert_eq!(select(&dates, "DAY", now), vec!["2026-09-24"]);
        assert_eq!(
            select(&dates, "WEEK", now),
            vec!["2026-09-24", "2026-09-25"]
        );
        assert_eq!(
            select(&dates, "FOUR_WEEKS", now),
            vec!["2026-09-24", "2026-09-25", "2026-09-28", "2026-09-29"]
        );
        assert_eq!(select(&dates, "NEXT_SESSION", now), vec!["2026-09-25"]);
        assert_eq!(friday_at(time("2026-09-26", 10)), Some("2026-10-02".into()));
    }
    #[test]
    fn expiry_clock_is_explicit_and_dst_uses_elapsed_time() {
        assert_eq!(model_years("2026-09-24", time("2026-09-24", 16)), None);
        let hours = model_years("2026-11-02", time("2026-10-30", 16)).unwrap() * 365.0 * 24.0;
        assert!((hours - 73.0).abs() < 1e-10);
        assert_eq!(model_years("invalid", 0), None);
    }
    #[test]
    fn legacy_horizon_selection_matches_twelve_fixed_date_cases() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-exposure.json")).unwrap();
        for case in fixture["horizons"].as_array().unwrap() {
            let dates: Vec<String> = serde_json::from_value(case["dates"].clone()).unwrap();
            let expected: Vec<String> = serde_json::from_value(case["expected"].clone()).unwrap();
            assert_eq!(
                select(
                    &dates,
                    case["mode"].as_str().unwrap(),
                    time(case["today"].as_str().unwrap(), 10)
                ),
                expected
            );
        }
    }
}
