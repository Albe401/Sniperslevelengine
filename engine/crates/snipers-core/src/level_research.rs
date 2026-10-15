//! Frozen point-in-time levels and conservative RTH labels. Not a predictor.
use crate::calendar::{self, Day};
use chrono::{Datelike, Duration};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
const MINUTE: u64 = 60_000;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    pub low: f64,
    pub high: f64,
}
impl Zone {
    fn valid(&self) -> bool {
        self.low.is_finite() && self.high.is_finite() && self.low > 0. && self.low <= self.high
    }
    fn touches(&self, b: &ObservedBar) -> bool {
        b.low <= self.high && b.high >= self.low
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    pub id: String,
    pub symbol: String,
    pub horizon: String,
    pub role: String,
    pub source: String,
    pub observed_ms: u64,
    pub available_ms: u64,
    pub expirations: Vec<String>,
    pub zone: Zone,
    pub structural_score: Option<f64>,
    pub complete: bool,
    pub eligible: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Case {
    pub id: String,
    pub level: Level,
    pub captured_ms: u64,
    pub start_ms: u64,
    pub deadline_ms: u64,
    pub spot_at_capture: f64,
    pub direction: String,
    /// Explicit competing level; never fabricate a stop from distance.
    pub barrier: Option<Zone>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub version: String,
    pub observation_id: String,
    pub levels: Vec<Level>,
    pub cases: Vec<Case>,
    pub blockers: Vec<String>,
    pub prediction_enabled: bool,
}
/// DAY=current RTH close; WEEK=last open RTH session through this Friday.
/// NEXT4 remains an expiration universe, never a weekly outcome horizon.
pub fn deadline(at: u64, horizon: &str) -> Option<u64> {
    let date = calendar::date_at(at)?;
    let Day::Open(today) = calendar::schedule(date) else {
        return None;
    };
    if at < today.open_ms || at >= today.close_ms {
        return None;
    }
    match horizon {
        "DAY" => Some(today.close_ms),
        "WEEK" => {
            let friday = date
                + Duration::days((4 - date.weekday().num_days_from_monday() as i64).rem_euclid(7));
            let mut d = date;
            let mut end = today.close_ms;
            while d <= friday {
                match calendar::schedule(d) {
                    Day::Open(s) => end = s.close_ms,
                    Day::Closed => {}
                    Day::Unknown => return None,
                }
                d = d.succ_opt()?;
            }
            Some(end)
        }
        _ => None,
    }
}
pub fn catalog(row: &Value) -> Catalog {
    let id = row["id"].as_str().unwrap_or("").to_owned();
    let mut out = Catalog {
        version: "level-contract-v1".into(),
        observation_id: id.clone(),
        levels: vec![],
        cases: vec![],
        blockers: vec![],
        prediction_enabled: false,
    };
    let symbol = row["symbol"].as_str().unwrap_or("");
    let captured = row["capturedMs"].as_u64().unwrap_or(0);
    let available = row["availableMs"].as_u64().unwrap_or(0);
    let tick = &row["evidence"]["tick"];
    let spot = tick["price"].as_f64().filter(|p| p.is_finite() && *p > 0.);
    let price_at = tick["tradeMs"].as_u64().unwrap_or(0);
    let input_ok = row["version"] == "research-capture-v1"
        && !id.is_empty()
        && crate::universe::stock_asset(symbol).is_some()
        && available == captured
        && row["quality"]["priceObservationEligible"] == true
        && row["quality"]["bars"] == "available"
        && tick["symbol"].as_str() == Some(symbol)
        && spot.is_some()
        && price_at > 0
        && price_at <= captured
        && captured - price_at <= 15_000;
    if !input_ok {
        out.blockers.push("observation-not-eligible".into());
    }
    // Future path starts at the NEXT full minute. The partly observed candle is excluded.
    let start = captured
        .checked_add(MINUTE - 1)
        .map(|t| t / MINUTE * MINUTE)
        .unwrap_or(u64::MAX);
    for h in row["evidence"]["optionSnapshotStatus"]["horizons"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let mode = h["mode"].as_str().unwrap_or("");
        if !matches!(mode, "DAY" | "WEEK") {
            out.blockers
                .push(format!("unsupported-outcome-horizon:{mode}"));
            continue;
        }
        let observed = h["observedMs"].as_u64().unwrap_or(0);
        let complete = h["structure"]["complete"] == true;
        let end = deadline(captured, mode);
        let eligible = input_ok
            && complete
            && observed > 0
            && observed <= captured
            && captured - observed <= 300_000
            && end.is_some_and(|e| start < e);
        let expirations: Vec<String> = h["expirations"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let s = &h["structure"];
        let mut candidates: BTreeMap<String, (f64, Option<f64>)> = BTreeMap::new();
        for (role, score) in [
            ("magnet", "mainMagnetScore"),
            ("dominantCall", ""),
            ("dominantPut", ""),
        ] {
            if let Some(p) = s[role].as_f64() {
                candidates.insert(role.into(), (p, s[score].as_f64()));
            }
        }
        for role in ["callWall", "putWall"] {
            if let Some(p) = s[role]["strike"].as_f64() {
                candidates.insert(role.into(), (p, s[role]["score"].as_f64()));
            }
        }
        if let Some(selected) = s["selected"].as_object() {
            for (role, p) in selected {
                if let Some(p) = p.as_f64() {
                    candidates.insert(format!("selected:{role}"), (p, None));
                }
            }
        }
        if let Some(top) = s["top"].as_array() {
            for (i, p) in top.iter().enumerate() {
                if let Some(p0) = p["strike"].as_f64() {
                    candidates.insert(
                        format!("rank:{i}:{}", p["kind"].as_str().unwrap_or("level")),
                        (p0, p["score"].as_f64()),
                    );
                }
            }
        }
        if let Some(routes) = s["routes"].as_array() {
            for (i, r) in routes.iter().enumerate() {
                for role in ["barrier", "target", "extended", "pullTarget"] {
                    if let Some(p) = r[role].as_f64() {
                        candidates.insert(format!("route:{i}:{role}"), (p, r["score"].as_f64()));
                    }
                }
            }
        }
        for (role, (p, score)) in candidates {
            if !p.is_finite() || p <= 0. {
                continue;
            }
            let level = Level {
                id: format!("{id}:{mode}:{role}:{p}"),
                symbol: symbol.into(),
                horizon: mode.into(),
                role,
                source: "massive-option-structure".into(),
                observed_ms: observed,
                available_ms: available,
                expirations: expirations.clone(),
                zone: Zone { low: p, high: p },
                structural_score: score.filter(|s| s.is_finite()),
                complete,
                eligible,
            };
            if eligible {
                let spot = spot.unwrap();
                if p != spot {
                    out.cases.push(Case {
                        id: level.id.clone(),
                        level: level.clone(),
                        captured_ms: captured,
                        start_ms: start,
                        deadline_ms: end.unwrap(),
                        spot_at_capture: spot,
                        direction: if p > spot { "up" } else { "down" }.into(),
                        barrier: None,
                    });
                }
            }
            out.levels.push(level);
        }
        if !eligible {
            out.blockers
                .push(format!("{mode}:coverage-time-or-session-not-eligible"));
        }
    }
    // Session VWAP keeps its own source clock and is a DAY reference, not weekly VWAP.
    let metrics = &row["evidence"]["metrics"];
    if let (Some(p), Some(at)) = (metrics["vwap"].as_f64(), metrics["asofMs"].as_u64()) {
        if p.is_finite() && p > 0. {
            let session_matches = calendar::date_at(captured)
                .is_some_and(|d| metrics["session"].as_str() == Some(d.to_string().as_str()));
            let eligible =
                input_ok && session_matches && at > 0 && at <= captured && captured - at <= 180_000;
            add_reference(
                &mut out,
                symbol,
                &id,
                "DAY",
                "session-vwap",
                "session-metrics",
                Zone { low: p, high: p },
                at,
                captured,
                start,
                spot,
                eligible,
            );
        }
    }
    let gaps = &row["evidence"]["gap"];
    let at = gaps["at"].as_u64().unwrap_or(0);
    let gap_ok = input_ok
        && gaps["symbol"].as_str() == Some(symbol)
        && gaps["actionsVerified"] == true
        && gaps["coveragePct"].as_f64() == Some(100.)
        && at > 0
        && at <= captured
        && captured - at <= 300_000;
    for g in gaps["gaps"].as_array().into_iter().flatten() {
        if g["verified"] != true
            || !matches!(g["state"].as_str(), Some("OPEN" | "PARTIAL"))
            || g["exDividend"] == true
        {
            continue;
        }
        if let (Some(low), Some(high)) =
            (g["remainingLower"].as_f64(), g["remainingUpper"].as_f64())
        {
            let zone = Zone { low, high };
            if !zone.valid() {
                continue;
            }
            for horizon in ["DAY", "WEEK"] {
                add_reference(
                    &mut out,
                    symbol,
                    &id,
                    horizon,
                    &format!("price-gap:{}", g["id"].as_str().unwrap_or("unknown")),
                    "price-gap-engine",
                    zone.clone(),
                    at,
                    captured,
                    start,
                    spot,
                    gap_ok,
                );
            }
        }
    }
    if out.levels.is_empty() {
        out.blockers.push("no-supported-levels".into());
    }
    out
}
#[allow(clippy::too_many_arguments)]
fn add_reference(
    out: &mut Catalog,
    symbol: &str,
    id: &str,
    horizon: &str,
    role: &str,
    source: &str,
    zone: Zone,
    observed: u64,
    captured: u64,
    start: u64,
    spot: Option<f64>,
    source_ok: bool,
) {
    let end = deadline(captured, horizon);
    let eligible = source_ok && end.is_some_and(|e| start < e);
    let level = Level {
        id: format!("{id}:{horizon}:{source}:{role}"),
        symbol: symbol.into(),
        horizon: horizon.into(),
        role: role.into(),
        source: source.into(),
        observed_ms: observed,
        available_ms: captured,
        expirations: vec![],
        zone,
        structural_score: None,
        complete: source_ok,
        eligible,
    };
    if eligible {
        if let Some(spot) = spot {
            if spot < level.zone.low || spot > level.zone.high {
                out.cases.push(Case {
                    id: level.id.clone(),
                    level: level.clone(),
                    captured_ms: captured,
                    start_ms: start,
                    deadline_ms: end.unwrap(),
                    spot_at_capture: spot,
                    direction: if spot < level.zone.low { "up" } else { "down" }.into(),
                    barrier: None,
                });
            }
        }
    }
    out.levels.push(level);
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObservedBar {
    pub symbol: String,
    pub timestamp_ms: u64,
    pub available_ms: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}
impl ObservedBar {
    fn valid(&self) -> bool {
        [self.open, self.high, self.low, self.close]
            .iter()
            .all(|v| v.is_finite() && *v > 0.)
            && self.high >= self.open.max(self.close)
            && self.low <= self.open.min(self.close)
            && self.timestamp_ms % MINUTE == 0
            && self.available_ms >= self.timestamp_ms.saturating_add(MINUTE)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub version: String,
    pub case_id: String,
    pub status: String,
    pub reason: String,
    pub evaluated_ms: u64,
    pub event_interval_ms: Option<[u64; 2]>,
    pub binary_arrival: Option<bool>,
    pub reaction: String,
}
fn result(c: &Case, now: u64, status: &str, reason: &str, event: Option<u64>) -> Label {
    Label {
        version: "level-outcome-v1".into(),
        case_id: c.id.clone(),
        status: status.into(),
        reason: reason.into(),
        evaluated_ms: now,
        event_interval_ms: event.map(|t| [t, t + MINUTE]),
        binary_arrival: match status {
            "reached" => Some(true),
            "not-reached" => Some(false),
            _ => None,
        },
        reaction: "unobserved".into(),
    }
}
fn trading_minute(t: u64) -> Option<bool> {
    match calendar::schedule(calendar::date_at(t)?) {
        Day::Open(s) => Some(t >= s.open_ms && t + MINUTE <= s.close_ms),
        Day::Closed => Some(false),
        Day::Unknown => None,
    }
}
/// Full RTH coverage is required before an event or a non-arrival label.
pub fn evaluate(c: &Case, bars: &[ObservedBar], now: u64) -> Label {
    if !c.level.eligible
        || !c.level.zone.valid()
        || c.level.available_ms != c.captured_ms
        || c.level.available_ms > c.start_ms
        || c.captured_ms > c.start_ms
        || c.level.observed_ms == 0
        || c.level.observed_ms > c.captured_ms
        || c.captured_ms - c.level.observed_ms > 300_000
        || c.start_ms % MINUTE != 0
        || c.start_ms >= c.deadline_ms
        || deadline(c.captured_ms, &c.level.horizon) != Some(c.deadline_ms)
        || !c.spot_at_capture.is_finite()
        || c.spot_at_capture <= 0.
        || !matches!(c.direction.as_str(), "up" | "down")
        || c.barrier.as_ref().is_some_and(|z| !z.valid())
    {
        return result(c, now, "invalid", "invalid-frozen-contract", None);
    }
    let mut by_time: BTreeMap<u64, &ObservedBar> = BTreeMap::new();
    for b in bars.iter().filter(|b| {
        b.symbol == c.level.symbol
            && b.timestamp_ms >= c.start_ms
            && b.timestamp_ms < c.deadline_ms
            && b.available_ms <= now
            && trading_minute(b.timestamp_ms) == Some(true)
    }) {
        if !b.valid() {
            return result(c, now, "censored", "invalid-bar", None);
        }
        if let Some(old) = by_time.insert(b.timestamp_ms, b) {
            if old.open != b.open || old.high != b.high || old.low != b.low || old.close != b.close
            {
                return result(c, now, "censored", "conflicting-bars", None);
            }
        }
    }
    let mut t = c.start_ms;
    while t < c.deadline_ms {
        match trading_minute(t) {
            None => return result(c, now, "censored", "calendar-unknown", None),
            Some(false) => {
                t += MINUTE;
                continue;
            }
            Some(true) => {}
        }
        if t + MINUTE > now {
            return result(c, now, "pending", "horizon-in-progress", None);
        }
        let Some(b) = by_time.get(&t) else {
            return result(
                c,
                now,
                if now < c.deadline_ms {
                    "pending"
                } else {
                    "censored"
                },
                "missing-minute",
                None,
            );
        };
        let hit = c.level.zone.touches(b);
        let barrier = c.barrier.as_ref().is_some_and(|z| z.touches(b));
        if hit && barrier {
            return result(
                c,
                now,
                "ambiguous",
                "target-and-barrier-in-same-minute",
                Some(t),
            );
        }
        if barrier {
            return result(c, now, "barrier-first", "competing-level-reached", Some(t));
        }
        if hit {
            let mut label = result(
                c,
                now,
                "reached",
                "ohlc-range-touch-minute-resolution",
                Some(t),
            );
            // FIVE subsequent closes; excludes the touch candle and unobserved future.
            let follow: Vec<_> = (1..=5).map(|i| t + i * MINUTE).collect();
            if follow.iter().all(|t| {
                *t < c.deadline_ms && trading_minute(*t) == Some(true) && by_time.contains_key(t)
            }) {
                let up = c.direction == "up";
                let accepted = follow.iter().all(|t| {
                    if up {
                        by_time[t].close > c.level.zone.high
                    } else {
                        by_time[t].close < c.level.zone.low
                    }
                });
                let rejected = follow.iter().all(|t| {
                    if up {
                        by_time[t].close < c.level.zone.low
                    } else {
                        by_time[t].close > c.level.zone.high
                    }
                });
                label.reaction = if accepted {
                    "accepted-five-closes"
                } else if rejected {
                    "rejected-five-closes"
                } else {
                    "mixed-five-closes"
                }
                .into();
            } else {
                label.reaction = "pending-or-insufficient-five-closes".into();
            }
            return label;
        }
        if (c.direction == "up" && b.low > c.level.zone.high)
            || (c.direction == "down" && b.high < c.level.zone.low)
        {
            return result(
                c,
                now,
                "censored",
                "gap-beyond-level-without-observed-touch",
                Some(t),
            );
        }
        t += MINUTE;
    }
    result(
        c,
        now,
        "not-reached",
        "complete-rth-coverage-through-deadline",
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn ms(s: &str) -> u64 {
        calendar::rfc3339_ms(s).unwrap()
    }
    #[test]
    fn sources_share_contract_without_losing_clock_or_horizon() {
        let mut r = row();
        let t = r["capturedMs"].as_u64().unwrap();
        r["evidence"]["metrics"] = json!({"vwap":102.,"asofMs":t-1000,"session":"2026-10-09"});
        r["evidence"]["gap"] = json!({"symbol":"QQQ","at":t-1000,"coveragePct":100.,"actionsVerified":true,"gaps":[{"id":"gap-one","verified":true,"state":"OPEN","exDividend":false,"remainingLower":108.,"remainingUpper":109.}]});
        let c = catalog(&r);
        assert_eq!(
            c.cases
                .iter()
                .filter(|c| c.level.source == "session-metrics")
                .count(),
            1
        );
        assert_eq!(
            c.cases
                .iter()
                .filter(|c| c.level.source == "price-gap-engine")
                .count(),
            2
        );
        assert!(c
            .cases
            .iter()
            .filter(|c| c.level.source != "massive-option-structure")
            .all(|c| c.level.structural_score.is_none()));
        r["evidence"]["metrics"]["session"] = json!("2026-10-08");
        r["evidence"]["gap"]["symbol"] = json!("SPY");
        assert!(catalog(&r)
            .cases
            .iter()
            .all(|c| c.level.source == "massive-option-structure"));
    }
    #[test]
    fn downside_rejection_is_based_on_future_closes() {
        let mut r = row();
        r["evidence"]["optionSnapshotStatus"]["horizons"][0]["structure"]["magnet"] = json!(95.);
        let c = catalog(&r)
            .cases
            .into_iter()
            .find(|c| c.level.horizon == "DAY")
            .unwrap();
        let mut bars = path(&c);
        bars[0] = bar(c.start_ms, 94., 101., 96.);
        let l = evaluate(&c, &bars, c.deadline_ms);
        assert_eq!(l.status, "reached");
        assert_eq!(l.reaction, "rejected-five-closes");
    }
    fn row() -> Value {
        let t = ms("2026-10-09T19:50:01Z");
        json!({"version":"research-capture-v1","id":"QQQ:one","symbol":"QQQ","capturedMs":t,"availableMs":t,
            "quality":{"priceObservationEligible":true,"bars":"available"},
            "evidence":{"tick":{"symbol":"QQQ","price":100.,"tradeMs":t-1000},"optionSnapshotStatus":{"horizons":[
                {"mode":"DAY","observedMs":t-1000,"expirations":["2026-10-09"],"structure":{"complete":true,"magnet":105.,"mainMagnetScore":87.}},
                {"mode":"WEEK","observedMs":t-1000,"expirations":["2026-10-09"],"structure":{"complete":true,"magnet":110.}},
                {"mode":"NEXT4","observedMs":t-1000,"structure":{"complete":true,"magnet":115.}}]}}})
    }
    fn case() -> Case {
        catalog(&row())
            .cases
            .into_iter()
            .find(|c| c.level.horizon == "DAY")
            .unwrap()
    }
    fn bar(t: u64, low: f64, high: f64, close: f64) -> ObservedBar {
        ObservedBar {
            symbol: "QQQ".into(),
            timestamp_ms: t,
            available_ms: t + MINUTE,
            open: 100.,
            high,
            low,
            close,
        }
    }
    fn path(c: &Case) -> Vec<ObservedBar> {
        (c.start_ms..c.deadline_ms)
            .step_by(MINUTE as usize)
            .map(|t| bar(t, 99., 101., 100.))
            .collect()
    }
    #[test]
    fn calendar_week_holiday_early_close_dst_and_unknown() {
        assert_eq!(
            deadline(ms("2026-11-23T15:00:00Z"), "WEEK"),
            Some(ms("2026-11-27T18:00:00Z"))
        );
        assert_eq!(
            deadline(ms("2026-04-02T14:00:00Z"), "WEEK"),
            Some(ms("2026-04-02T20:00:00Z"))
        );
        assert_eq!(
            deadline(ms("2026-03-06T15:00:00Z"), "DAY"),
            Some(ms("2026-03-06T21:00:00Z"))
        );
        assert_eq!(
            deadline(ms("2026-03-09T14:00:00Z"), "DAY"),
            Some(ms("2026-03-09T20:00:00Z"))
        );
        assert_eq!(deadline(ms("2029-10-09T14:00:00Z"), "WEEK"), None);
        assert_eq!(deadline(ms("2026-10-09T21:00:00Z"), "DAY"), None);
    }
    #[test]
    fn contract_is_frozen_and_does_not_treat_next4_as_week() {
        let mut r = row();
        let cat = catalog(&r);
        assert_eq!(cat.cases.len(), 2);
        assert!(cat
            .blockers
            .contains(&"unsupported-outcome-horizon:NEXT4".into()));
        let c = cat.cases[0].clone();
        r["evidence"]["optionSnapshotStatus"]["horizons"][0]["structure"]["magnet"] = json!(120.);
        assert_eq!(c.level.zone.low, 105.);
        assert_eq!(c.level.structural_score, Some(87.));
        assert!(c.start_ms > c.captured_ms);
        assert_eq!(c.start_ms % MINUTE, 0);
    }
    #[test]
    fn source_future_stale_partial_or_foreign_never_becomes_case() {
        for kind in ["future", "stale", "partial", "foreign", "after-hours"] {
            let mut r = row();
            let t = r["capturedMs"].as_u64().unwrap();
            match kind {
                "future" => {
                    r["evidence"]["optionSnapshotStatus"]["horizons"][0]["observedMs"] =
                        json!(t + 1)
                }
                "stale" => {
                    r["evidence"]["optionSnapshotStatus"]["horizons"][0]["observedMs"] =
                        json!(t - 300001)
                }
                "partial" => {
                    r["evidence"]["optionSnapshotStatus"]["horizons"][0]["structure"]["complete"] =
                        json!(false)
                }
                "foreign" => r["evidence"]["tick"]["symbol"] = json!("SPY"),
                _ => r["quality"]["priceObservationEligible"] = json!(false),
            }
            assert!(
                !catalog(&r).cases.iter().any(|c| c.level.horizon == "DAY"),
                "{kind}"
            );
        }
    }
    #[test]
    fn no_future_high_from_partly_observed_candle() {
        let c = case();
        let mut bars = path(&c);
        bars.push(bar(c.start_ms - MINUTE, 99., 110., 100.));
        assert_eq!(
            evaluate(&c, &bars, c.deadline_ms).binary_arrival,
            Some(false)
        );
    }
    #[test]
    fn positive_label_has_minute_interval_and_reaction_requires_later_closes() {
        let c = case();
        let mut bars = path(&c);
        bars[0] = bar(c.start_ms, 99., 106., 104.);
        let label = evaluate(&c, &bars, c.start_ms + MINUTE);
        assert_eq!(label.status, "reached");
        assert_eq!(
            label.event_interval_ms,
            Some([c.start_ms, c.start_ms + MINUTE])
        );
        assert!(label.reaction.starts_with("pending"));
        for b in bars.iter_mut().skip(1).take(5) {
            b.open = 106.;
            b.low = 105.5;
            b.high = 107.;
            b.close = 106.;
        }
        assert_eq!(
            evaluate(&c, &bars, c.deadline_ms).reaction,
            "accepted-five-closes"
        );
    }
    #[test]
    fn missing_before_touch_is_censored_not_positive_or_negative() {
        let c = case();
        let mut bars = path(&c);
        bars[1] = bar(c.start_ms + MINUTE, 99., 106., 100.);
        bars.remove(0);
        let l = evaluate(&c, &bars, c.deadline_ms);
        assert_eq!(l.status, "censored");
        assert_eq!(l.binary_arrival, None);
    }
    #[test]
    fn no_arrival_requires_complete_closed_horizon() {
        let c = case();
        let bars = path(&c);
        assert_eq!(
            evaluate(&c, &bars, c.deadline_ms - MINUTE).status,
            "pending"
        );
        assert_eq!(
            evaluate(&c, &bars, c.deadline_ms).binary_arrival,
            Some(false)
        );
    }
    #[test]
    fn target_and_barrier_same_bar_is_ambiguous() {
        let mut c = case();
        c.barrier = Some(Zone {
            low: 95.,
            high: 95.,
        });
        let mut bars = path(&c);
        bars[0] = bar(c.start_ms, 94., 106., 100.);
        let l = evaluate(&c, &bars, c.deadline_ms);
        assert_eq!(l.status, "ambiguous");
        assert_eq!(l.binary_arrival, None);
    }
    #[test]
    fn barrier_before_target_remains_competing_event() {
        let mut c = case();
        c.barrier = Some(Zone {
            low: 95.,
            high: 95.,
        });
        let mut bars = path(&c);
        bars[0] = bar(c.start_ms, 94., 101., 100.);
        bars[1] = bar(c.start_ms + MINUTE, 99., 106., 100.);
        assert_eq!(evaluate(&c, &bars, c.deadline_ms).status, "barrier-first");
    }
    #[test]
    fn revisions_are_censored_identical_overlap_and_other_assets_are_safe() {
        let c = case();
        let mut bars = path(&c);
        bars.push(bars[0].clone());
        let mut foreign = bars[0].clone();
        foreign.symbol = "SPY".into();
        foreign.high = 110.;
        bars.push(foreign);
        assert_eq!(
            evaluate(&c, &bars, c.deadline_ms).binary_arrival,
            Some(false)
        );
        let mut revision = bars[0].clone();
        revision.high = 106.;
        bars.push(revision);
        assert_eq!(
            evaluate(&c, &bars, c.deadline_ms).reason,
            "conflicting-bars"
        );
    }
    #[test]
    fn delayed_or_future_bar_is_not_available_and_gap_is_not_exact_touch() {
        let c = case();
        let mut bars = path(&c);
        bars[0].available_ms = c.deadline_ms + 1;
        assert_eq!(evaluate(&c, &bars, c.deadline_ms).status, "censored");
        bars[0] = ObservedBar {
            open: 106.,
            low: 106.,
            high: 107.,
            close: 106.,
            ..bars[0].clone()
        };
        bars[0].available_ms = c.start_ms + MINUTE;
        assert_eq!(
            evaluate(&c, &bars, c.deadline_ms).reason,
            "gap-beyond-level-without-observed-touch"
        );
    }
    #[test]
    fn weekly_missing_overnight_is_not_rth_missing() {
        let mut r = row();
        let t = ms("2026-10-08T19:59:01Z");
        r["capturedMs"] = json!(t);
        r["availableMs"] = json!(t);
        r["evidence"]["tick"]["tradeMs"] = json!(t);
        r["evidence"]["optionSnapshotStatus"]["horizons"][1]["observedMs"] = json!(t);
        let c = catalog(&r)
            .cases
            .into_iter()
            .find(|c| c.level.horizon == "WEEK")
            .unwrap();
        let bars: Vec<_> = (c.start_ms..c.deadline_ms)
            .step_by(MINUTE as usize)
            .filter(|t| trading_minute(*t) == Some(true))
            .map(|t| bar(t, 99., 101., 100.))
            .collect();
        assert_eq!(
            evaluate(&c, &bars, c.deadline_ms).binary_arrival,
            Some(false)
        );
    }
}
