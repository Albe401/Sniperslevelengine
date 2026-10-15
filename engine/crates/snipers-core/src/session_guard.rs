//! Paper-entry admission only. This module never creates a trade or contacts a broker.
use crate::calendar::{self, Day};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub symbol: String,
    pub decision: String,
    pub data_timestamp_ms: Option<u64>,
    pub decision_ready: bool,
    pub allow_new_entries: bool,
    /// Loaded transactionally from the paper ledger, never inferred from UI state.
    pub active_trade_exists: Option<bool>,
    pub daily_trade_exists: Option<bool>,
    pub ledger_session: Option<String>,
    pub max_data_age_seconds: u64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Result {
    pub allowed: bool,
    pub reason: String,
    pub session_state: String,
    pub session: Option<String>,
    pub data_fresh: bool,
    pub data_age_ms: Option<u64>,
    pub symbol: String,
    pub decision: String,
}
pub fn evaluate(i: &Input, now: u64) -> Result {
    let date = calendar::date_at(now);
    let session = date.map(|d| d.to_string());
    let phase = match date.map(calendar::schedule) {
        Some(Day::Open(day)) if now < day.open_ms => "PREMARKET",
        Some(Day::Open(day)) if now >= day.close_ms => "AFTER_HOURS",
        Some(Day::Open(_)) => "RTH_OPEN",
        Some(Day::Closed) => "MARKET_CLOSED",
        _ => "CALENDAR_UNAVAILABLE",
    };
    let age = i.data_timestamp_ms.and_then(|t| now.checked_sub(t));
    let max_age = i.max_data_age_seconds.clamp(15, 120) * 1000;
    let same_day = i
        .data_timestamp_ms
        .is_some_and(|t| calendar::date_at(t) == date && date.is_some());
    let fresh = same_day && age.is_some_and(|age| age <= max_age);
    let symbol = i.symbol.trim().to_ascii_uppercase();
    let decision = i.decision.trim().to_ascii_uppercase();
    let reason = if !i.allow_new_entries {
        "NEW_ENTRIES_DISABLED"
    } else if phase != "RTH_OPEN" {
        "OUTSIDE_RTH"
    } else if symbol != "QQQ" {
        "SYMBOL_NOT_ENABLED"
    } else if i.active_trade_exists == Some(true) {
        "ACTIVE_TRADE_ALREADY_EXISTS"
    } else if session.is_none()
        || i.ledger_session != session
        || i.active_trade_exists.is_none()
        || i.daily_trade_exists.is_none()
    {
        "LEDGER_NOT_RECOVERED"
    } else if i.daily_trade_exists == Some(true) {
        "DAILY_TRADE_LIMIT_REACHED"
    } else if !i.decision_ready || !["CALL", "PUT"].contains(&decision.as_str()) {
        "NO_ACTIONABLE_DECISION"
    } else if i.data_timestamp_ms.is_none() {
        "MISSING_DATA_TIMESTAMP"
    } else if age.is_none() {
        "FUTURE_DATA_TIMESTAMP"
    } else if !fresh {
        "STALE_DATA"
    } else {
        "ENTRY_ALLOWED"
    };
    Result {
        allowed: reason == "ENTRY_ALLOWED",
        reason: reason.into(),
        session_state: phase.into(),
        session,
        data_fresh: fresh,
        data_age_ms: age,
        symbol,
        decision,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_session_admission_matches_1440_legacy_cases() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-session-guard.json"))
                .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 1440);
        for (index, case) in cases.iter().enumerate() {
            let input: Input = serde_json::from_value(case["input"].clone()).unwrap();
            let result = evaluate(&input, case["now"].as_u64().unwrap());
            assert_eq!(
                serde_json::json!({"allowed": result.allowed,"reason":result.reason,"dataFresh":result.data_fresh,"sessionState":result.session_state}),
                case["expected"],
                "legacy scenario {index}"
            );
        }
    }
    fn open(date: &str) -> u64 {
        let Day::Open(day) =
            calendar::schedule(calendar::date_at(calendar::day_bounds(date).unwrap().0).unwrap())
        else {
            panic!()
        };
        day.open_ms
    }
    fn input(now: u64) -> Input {
        Input {
            symbol: "QQQ".into(),
            decision: "CALL".into(),
            data_timestamp_ms: Some(now),
            decision_ready: true,
            allow_new_entries: true,
            active_trade_exists: Some(false),
            daily_trade_exists: Some(false),
            ledger_session: calendar::date_at(now).map(|d| d.to_string()),
            max_data_age_seconds: 120,
        }
    }
    #[test]
    fn admission_requires_recovered_current_session_and_preserves_daily_limit_after_close() {
        let now = open("2026-09-24") + 900_000;
        let mut i = input(now);
        assert!(evaluate(&i, now).allowed);
        i.allow_new_entries = false;
        assert_eq!(evaluate(&i, now).reason, "NEW_ENTRIES_DISABLED");
        i.allow_new_entries = true;
        i.active_trade_exists = Some(true);
        assert_eq!(evaluate(&i, now).reason, "ACTIVE_TRADE_ALREADY_EXISTS");
        i.active_trade_exists = Some(false);
        i.daily_trade_exists = Some(true);
        assert_eq!(evaluate(&i, now).reason, "DAILY_TRADE_LIMIT_REACHED");
        i.daily_trade_exists = None;
        assert_eq!(evaluate(&i, now).reason, "LEDGER_NOT_RECOVERED");
        i.daily_trade_exists = Some(false);
        i.ledger_session = Some("2026-09-23".into());
        assert_eq!(evaluate(&i, now).reason, "LEDGER_NOT_RECOVERED");
        i.ledger_session = Some("2026-09-24".into());
        i.symbol = "SPY".into();
        assert_eq!(evaluate(&i, now).reason, "SYMBOL_NOT_ENABLED");
        i.symbol = "QQQ".into();
        i.decision = "WAIT".into();
        assert_eq!(evaluate(&i, now).reason, "NO_ACTIONABLE_DECISION");
    }
    #[test]
    fn time_guards_cover_boundaries_dst_holidays_early_closes_and_unknown_calendar() {
        for date in ["2026-01-05", "2026-07-06", "2026-11-27"] {
            let start = open(date);
            let Day::Open(day) = calendar::schedule(calendar::date_at(start).unwrap()) else {
                panic!()
            };
            assert!(!evaluate(&input(start - 1), start - 1).allowed);
            assert!(evaluate(&input(start), start).allowed);
            assert!(evaluate(&input(day.close_ms - 1), day.close_ms - 1).allowed);
            assert!(!evaluate(&input(day.close_ms), day.close_ms).allowed);
        }
        for date in ["2026-12-25", "2026-09-26", "2029-01-02"] {
            let now = calendar::day_bounds(date).unwrap().0 + 36_000_000;
            assert!(!evaluate(&input(now), now).allowed);
        }
        let now = open("2026-09-24") + 900_000;
        let mut i = input(now);
        i.data_timestamp_ms = Some(now + 1);
        assert_eq!(evaluate(&i, now).reason, "FUTURE_DATA_TIMESTAMP");
        i.data_timestamp_ms = None;
        assert_eq!(evaluate(&i, now).reason, "MISSING_DATA_TIMESTAMP");
        i.data_timestamp_ms = Some(now - 120_000);
        assert!(evaluate(&i, now).allowed);
        i.data_timestamp_ms = Some(now - 120_001);
        assert_eq!(evaluate(&i, now).reason, "STALE_DATA");
        i.max_data_age_seconds = u64::MAX;
        assert!(!evaluate(&i, now).allowed);
        i.max_data_age_seconds = 0;
        i.data_timestamp_ms = Some(now - 15_001);
        assert!(!evaluate(&i, now).allowed);
    }
}
