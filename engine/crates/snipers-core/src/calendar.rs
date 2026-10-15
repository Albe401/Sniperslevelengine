//! Published NYSE core equity schedule, 2024–2028. Outside coverage is unknown.
//! Historical calendars: https://www.nyse.com/publicdocs/ICE_NYSE_2024_Yearly_Trading_Calendar.pdf
//! https://www.nyse.com/publicdocs/ICE_NYSE_2025_Yearly_Trading_Calendar.pdf
//! Jan 9 mourning closure: https://www.nyse.com/publicdocs/nyse/markets/american-options/rule-interpretations/2025/National_Day_of_Mourning_20250102.pdf
//! Source: https://www.nyse.com/trade/hours-calendars (consulted 2026-09-24).
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc, Weekday};
use chrono_tz::America::New_York;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub date: String,
    pub open_ms: u64,
    pub close_ms: u64,
    pub early_close: bool,
}
#[derive(Debug, PartialEq)]
pub enum Day {
    Open(Session),
    Closed,
    Unknown,
}

pub fn date_at(ms: u64) -> Option<NaiveDate> {
    Some(
        DateTime::<Utc>::from_timestamp_millis(i64::try_from(ms).ok()?)?
            .with_timezone(&New_York)
            .date_naive(),
    )
}
/// Timestamp carrying an explicit offset; never interpret a vendor time locally.
pub fn rfc3339_ms(text: &str) -> Option<u64> {
    u64::try_from(DateTime::parse_from_rfc3339(text).ok()?.timestamp_millis()).ok()
}
/// Legacy local ET timestamp. Ambiguous or nonexistent DST times are rejected.
pub fn legacy_eastern_ms(text: &str) -> Option<u64> {
    let local = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S").ok()?;
    u64::try_from(
        New_York
            .from_local_datetime(&local)
            .single()?
            .timestamp_millis(),
    )
    .ok()
}
/// Half-open ET calendar day; unlike adding 24h, this preserves DST boundaries.
pub fn day_bounds(date: &str) -> Option<(u64, u64)> {
    let day = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let midnight = |d: NaiveDate| {
        u64::try_from(
            New_York
                .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
                .single()?
                .timestamp_millis(),
        )
        .ok()
    };
    Some((midnight(day)?, midnight(day.succ_opt()?)?))
}
pub fn schedule(date: NaiveDate) -> Day {
    if !(2024..=2028).contains(&date.year()) {
        return Day::Unknown;
    }
    if matches!(date.weekday(), Weekday::Sat | Weekday::Sun) {
        return Day::Closed;
    }
    let label = date.to_string();
    const CLOSED: &[&str] = &[
        "2024-01-01", "2024-01-15", "2024-02-19", "2024-03-29",
        "2024-05-27", "2024-06-19", "2024-07-04", "2024-09-02",
        "2024-11-28", "2024-12-25",
        "2025-01-01", "2025-01-09", "2025-01-20", "2025-02-17",
        "2025-04-18", "2025-05-26", "2025-06-19", "2025-07-04",
        "2025-09-01", "2025-11-27", "2025-12-25",
        "2026-01-01",
        "2026-01-19",
        "2026-02-16",
        "2026-04-03",
        "2026-05-25",
        "2026-06-19",
        "2026-07-03",
        "2026-09-07",
        "2026-11-26",
        "2026-12-25",
        "2027-01-01",
        "2027-01-18",
        "2027-02-15",
        "2027-03-26",
        "2027-05-31",
        "2027-06-18",
        "2027-07-05",
        "2027-09-06",
        "2027-11-25",
        "2027-12-24",
        "2028-01-17",
        "2028-02-21",
        "2028-04-14",
        "2028-05-29",
        "2028-06-19",
        "2028-07-04",
        "2028-09-04",
        "2028-11-23",
        "2028-12-25",
    ];
    if CLOSED.contains(&label.as_str()) {
        return Day::Closed;
    }
    let early = [
        "2024-07-03", "2024-11-29", "2024-12-24",
        "2025-07-03", "2025-11-28", "2025-12-24",
        "2026-11-27",
        "2026-12-24",
        "2027-11-26",
        "2028-07-03",
        "2028-11-24",
    ]
    .contains(&label.as_str());
    let open = New_York
        .from_local_datetime(&date.and_hms_opt(9, 30, 0).unwrap())
        .single();
    let close = New_York
        .from_local_datetime(&date.and_hms_opt(if early { 13 } else { 16 }, 0, 0).unwrap())
        .single();
    match (open, close) {
        (Some(open), Some(close)) => Day::Open(Session {
            date: label,
            open_ms: open.timestamp_millis() as u64,
            close_ms: close.timestamp_millis() as u64,
            early_close: early,
        }),
        _ => Day::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn day(s: &str) -> Day {
        schedule(NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap())
    }
    #[test]
    fn published_holidays_early_close_and_unknown_year() {
        assert_eq!(day("2026-07-03"), Day::Closed);
        assert_eq!(day("2026-09-26"), Day::Closed);
        assert_eq!(day("2029-01-02"), Day::Unknown);
        let Day::Open(s) = day("2026-11-27") else {
            panic!()
        };
        assert!(s.early_close);
        assert_eq!(s.close_ms - s.open_ms, 210 * 60_000);
        // NYSE explicitly does not observe Jan 1 2028 on Dec 31 2027.
        assert!(matches!(day("2027-12-31"), Day::Open(_)));
    }
    #[test]
    fn historical_calendar_preserves_special_closure_and_half_days() {
        assert_eq!(day("2025-01-09"), Day::Closed);
        assert_eq!(day("2024-11-28"), Day::Closed);
        assert_eq!(day("2023-12-29"), Day::Unknown);
        for date in ["2024-07-03", "2024-11-29", "2024-12-24", "2025-07-03", "2025-11-28", "2025-12-24"] {
            let Day::Open(s)=day(date) else {panic!("{date}")};
            assert_eq!(s.close_ms-s.open_ms,210*60_000);
        }
        assert!(matches!(day("2024-10-09"),Day::Open(_)));
    }
    #[test]
    fn dst_changes_utc_open_but_not_local_session() {
        let Day::Open(w) = day("2026-01-23") else {
            panic!()
        };
        let Day::Open(s) = day("2026-09-23") else {
            panic!()
        };
        assert_eq!(w.open_ms % 86_400_000, 14 * 3_600_000 + 30 * 60_000);
        assert_eq!(s.open_ms % 86_400_000, 13 * 3_600_000 + 30 * 60_000);
        assert_eq!(w.close_ms - w.open_ms, 390 * 60_000);
        assert_eq!(date_at(s.open_ms).unwrap().to_string(), "2026-09-23");
    }
}
