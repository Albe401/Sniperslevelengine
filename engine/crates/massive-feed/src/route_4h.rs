//! Session-anchored structure and chronological study. No orders or inferred dealer inventory.
use serde::{Deserialize, Serialize};
use snipers_core::calendar::{self, Day};
use std::collections::BTreeMap;
pub const VERSION: &str = "route-4h-v1";
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Minute {
    pub timestamp_ms: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}
impl From<&crate::live::Bar> for Minute {
    fn from(b: &crate::live::Bar) -> Self {
        Self {
            timestamp_ms: b.timestamp_ms,
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close,
            volume: b.volume,
        }
    }
}
fn valid(b: &Minute) -> bool {
    b.timestamp_ms.is_multiple_of(60000)
        && [b.open, b.high, b.low, b.close, b.volume]
            .iter()
            .all(|v| v.is_finite())
        && b.low > 0.
        && b.high >= b.open.max(b.close)
        && b.low <= b.open.min(b.close)
        && b.volume >= 0.
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub start: u64,
    pub end: u64,
    pub date: String,
    pub duration_minutes: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub complete: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quality {
    pub input_minutes: usize,
    pub invalid_minutes: usize,
    pub duplicate_minutes: usize,
    pub unknown_calendar: usize,
    pub incomplete_blocks: usize,
    pub closed_blocks: usize,
}
/// Invalid/duplicate minutes poison their bucket. Missing bars never become synthetic prices.
pub fn aggregate(input: &[Minute], now: u64) -> (Vec<Block>, Quality) {
    let mut q = Quality {
        input_minutes: input.len(),
        invalid_minutes: 0,
        duplicate_minutes: 0,
        unknown_calendar: 0,
        incomplete_blocks: 0,
        closed_blocks: 0,
    };
    let mut minutes = BTreeMap::<u64, Option<&Minute>>::new();
    for b in input {
        if b.timestamp_ms >= now {
            continue;
        }
        let value = if valid(b) {
            Some(b)
        } else {
            q.invalid_minutes += 1;
            None
        };
        if let std::collections::btree_map::Entry::Vacant(e) = minutes.entry(b.timestamp_ms) {
            e.insert(value);
        } else {
            q.duplicate_minutes += 1;
            minutes.insert(b.timestamp_ms, None);
        }
    }
    let mut buckets = BTreeMap::<u64, (u64, String, Vec<&Minute>)>::new();
    for (&t, b) in &minutes {
        let Some(date) = calendar::date_at(t) else {
            q.unknown_calendar += 1;
            continue;
        };
        let session = match calendar::schedule(date) {
            Day::Open(s) => s,
            Day::Closed => continue,
            Day::Unknown => {
                q.unknown_calendar += 1;
                continue;
            }
        };
        if t < session.open_ms || t >= session.close_ms {
            continue;
        }
        let start = session.open_ms + (t - session.open_ms) / 14_400_000 * 14_400_000;
        let end = (start + 14_400_000).min(session.close_ms);
        if end > now {
            continue;
        }
        let entry = buckets.entry(start).or_insert((end, session.date, vec![]));
        if let Some(b) = b {
            entry.2.push(b);
        }
    }
    if let (Some((&first, _)), Some((&last, _))) =
        (minutes.first_key_value(), minutes.last_key_value())
    {
        let mut t = first;
        for _ in 0..367 {
            let Some(date) = calendar::date_at(t) else {
                break;
            };
            if let Day::Open(session) = calendar::schedule(date) {
                for start in (session.open_ms..session.close_ms).step_by(14_400_000) {
                    let end = (start + 14_400_000).min(session.close_ms);
                    if end <= now {
                        buckets
                            .entry(start)
                            .or_insert((end, session.date.clone(), vec![]));
                    }
                }
            }
            let Some((_, next)) = calendar::day_bounds(&date.to_string()) else {
                break;
            };
            if next > last {
                break;
            }
            t = next;
        }
    }
    let mut out = vec![];
    for (start, (end, date, rows)) in buckets {
        let expected = (end - start) / 60000;
        let complete = rows.len() as u64 == expected
            && rows
                .iter()
                .enumerate()
                .all(|(i, b)| b.timestamp_ms == start + i as u64 * 60000);
        if !complete {
            q.incomplete_blocks += 1;
        }
        if rows.is_empty() {
            out.push(Block {
                start,
                end,
                date,
                duration_minutes: expected,
                open: 0.,
                high: 0.,
                low: 0.,
                close: 0.,
                volume: 0.,
                complete: false,
            });
            continue;
        }
        out.push(Block {
            start,
            end,
            date,
            duration_minutes: expected,
            open: rows[0].open,
            high: rows
                .iter()
                .map(|b| b.high)
                .fold(f64::NEG_INFINITY, f64::max),
            low: rows.iter().map(|b| b.low).fold(f64::INFINITY, f64::min),
            close: rows.last().unwrap().close,
            volume: rows.iter().map(|b| b.volume).sum(),
            complete,
        });
    }
    q.closed_blocks = out.iter().filter(|b| b.complete).count();
    (out, q)
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pivot {
    pub at: u64,
    pub confirmed_at: u64,
    pub price: f64,
    pub kind: String,
}
/// Pivots become available only at the close of their second right-hand bar.
pub fn pivots(bars: &[Block]) -> Vec<Pivot> {
    let mut out = vec![];
    for w in bars.windows(5) {
        if w.iter().any(|b| !b.complete) {
            continue;
        }
        let p = &w[2];
        for (high, kind) in [(true, "high"), (false, "low")] {
            let yes = w
                .iter()
                .enumerate()
                .all(|(i, b)| i == 2 || if high { p.high > b.high } else { p.low < b.low });
            if yes {
                out.push(Pivot {
                    at: p.end,
                    confirmed_at: w[4].end,
                    price: if high { p.high } else { p.low },
                    kind: kind.into(),
                });
            }
        }
    }
    out
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub id: String,
    pub symbol: String,
    pub at: u64,
    pub side: i8,
    pub setup: String,
    pub structure: String,
    pub origin: f64,
    pub target: f64,
    pub target_source: String,
    pub invalidation: f64,
    pub block_minutes: u64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub version: String,
    pub symbol: String,
    pub at: u64,
    pub state: String,
    pub structure: String,
    pub pivots: Vec<Pivot>,
    pub scenario: Option<Scenario>,
    pub quality: Quality,
    pub reason: String,
    pub calibrated_probability: Option<f64>,
}
pub(crate) fn detect(symbol: &str, bars: &[Block]) -> (String, Vec<Pivot>, Option<Scenario>) {
    let cutoff = bars.last().map(|b| b.start).unwrap_or(0);
    let ps: Vec<_> = pivots(bars)
        .into_iter()
        .filter(|p| p.confirmed_at <= cutoff)
        .collect();
    let highs: Vec<_> = ps.iter().filter(|p| p.kind == "high").collect();
    let lows: Vec<_> = ps.iter().filter(|p| p.kind == "low").collect();
    let structure = if highs.len() < 2 || lows.len() < 2 {
        "pending"
    } else {
        let h = highs[highs.len() - 1].price - highs[highs.len() - 2].price;
        let l = lows[lows.len() - 1].price - lows[lows.len() - 2].price;
        if h > 0. && l > 0. {
            "up"
        } else if h < 0. && l < 0. {
            "down"
        } else {
            "range"
        }
    }
    .to_string();
    if structure == "pending" {
        return (structure, ps, None);
    }
    let Some(last) = bars.last().filter(|b| b.complete) else {
        return (structure, ps, None);
    };
    let Some(prev) = bars.iter().rev().nth(1).filter(|b| b.complete) else {
        return (structure, ps, None);
    };
    let (Some(h), Some(l)) = (highs.last(), lows.last()) else {
        return (structure, ps, None);
    };
    // Only levels known BEFORE this block may trigger it.
    if h.confirmed_at > last.start || l.confirmed_at > last.start {
        return (structure, ps, None);
    }
    let tolerance = last.close * 0.0003;
    let signal = if prev.close <= h.price + tolerance && last.close > h.price + tolerance {
        Some((1, "breakout", h.price))
    } else if prev.close >= l.price - tolerance && last.close < l.price - tolerance {
        Some((-1, "breakdown", l.price))
    } else if last.low < l.price - tolerance && last.close > l.price + tolerance {
        Some((1, "recovery", l.price))
    } else if last.high > h.price + tolerance && last.close < h.price - tolerance {
        Some((-1, "rejection", h.price))
    } else {
        None
    };
    let Some((side, setup, level)) = signal else {
        return (structure, ps, None);
    };
    let stop = if side == 1 {
        last.low.min(level) - tolerance
    } else {
        last.high.max(level) + tolerance
    };
    let known = ps
        .iter()
        .filter(|p| {
            p.confirmed_at <= last.start && (p.price - last.close) * side as f64 > tolerance
        })
        .map(|p| p.price)
        .min_by(|a, b| ((a - last.close).abs()).total_cmp(&(b - last.close).abs()));
    let range = h.price - l.price;
    if !range.is_finite() || range <= tolerance {
        return (structure, ps, None);
    }
    let (target, source) = known
        .map(|p| (p, "known-pivot"))
        .unwrap_or((last.close + side as f64 * range, "range-projection"));
    if target <= 0.
        || !target.is_finite()
        || !stop.is_finite()
        || stop <= 0.
        || (target - last.close) * side as f64 <= 0.
        || (last.close - stop) * side as f64 <= 0.
    {
        return (structure, ps, None);
    }
    let scenario = Scenario {
        id: format!("{VERSION}:{symbol}:{}:{setup}", last.end),
        symbol: symbol.into(),
        at: last.end,
        side,
        setup: setup.into(),
        structure: structure.clone(),
        origin: last.close,
        target,
        target_source: source.into(),
        invalidation: stop,
        block_minutes: last.duration_minutes,
    };
    (structure, ps, Some(scenario))
}
pub fn read(symbol: &str, input: &[Minute], now: u64) -> Reading {
    let (bars, q) = aggregate(input, now);
    let tail = bars
        .iter()
        .rposition(|b| !b.complete)
        .map(|i| i + 1)
        .unwrap_or(0);
    let usable = &bars[tail..];
    let (structure, ps, mut scenario) = if usable.len() >= 12 {
        detect(symbol, usable)
    } else {
        ("pending".into(), vec![], None)
    };
    // Keep the last confirmed scenario through its two-session horizon; a subsequent
    // quiet 4H close must not erase its frozen target and monitoring outcome.
    if scenario.is_none() {
        for i in (12..usable.len()).rev() {
            if swing_deadline(usable[i - 1].end, 2).is_none_or(|end| end <= now) {
                break;
            }
            if let Some(s) = detect(symbol, &usable[..i]).2 {
                scenario = Some(s);
                break;
            }
        }
    }
    Reading{version:VERSION.into(),symbol:symbol.into(),at:now,state:if scenario.is_some(){"candidate"}else if structure=="pending"{"pending"}else{"waiting"}.into(),structure,pivots:ps.into_iter().rev().take(20).collect(),scenario,quality:q,reason:"Contexto 4H; no confirma por sí solo una entrada. Proyección de rango identificada cuando no hay objetivo estructural conocido. Sin probabilidad calibrada.".into(),calibrated_probability:None}
}
pub(crate) fn next_regular(mut t: u64) -> Option<u64> {
    for _ in 0..14 {
        let date = calendar::date_at(t)?;
        match calendar::schedule(date) {
            Day::Open(s) => {
                if t < s.open_ms {
                    return Some(s.open_ms);
                }
                if t < s.close_ms {
                    return Some(t);
                }
            }
            Day::Closed => {}
            Day::Unknown => return None,
        }
        t = calendar::day_bounds(&date.to_string())?.1;
    }
    None
}
pub(crate) fn swing_deadline(at: u64, count: usize) -> Option<u64> {
    let mut t = next_regular(at)?;
    for i in 0..count {
        let Day::Open(s) = calendar::schedule(calendar::date_at(t)?) else {
            return None;
        };
        if i + 1 == count {
            return Some(s.close_ms);
        }
        t = next_regular(s.close_ms)?;
    }
    None
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub scenario: Scenario,
    pub horizon: String,
    pub deadline: u64,
    pub status: String,
    pub resolved_at: Option<u64>,
    pub favorable: f64,
    pub adverse: f64,
}
/// Endpoint agrees with the existing route: target/stop by minute CLOSE. Wick overlap is not a close result.
pub fn monitor(s: &Scenario, input: &[Minute], now: u64) -> Outcome {
    let deadline = swing_deadline(s.at, 2).unwrap_or(s.at);
    let mut result = resolve(s, input, deadline, now);
    result.horizon = "swing2".into();
    result
}
pub fn resolve(s: &Scenario, minutes: &[Minute], deadline: u64, now: u64) -> Outcome {
    let mut out = Outcome {
        scenario: s.clone(),
        horizon: String::new(),
        deadline,
        status: "pending".into(),
        resolved_at: None,
        favorable: 0.,
        adverse: 0.,
    };
    let sign = s.side as f64;
    let mut expected = next_regular(s.at).unwrap_or(u64::MAX);
    for b in minutes
        .iter()
        .filter(|b| b.timestamp_ms >= s.at && b.timestamp_ms + 60000 <= deadline.min(now))
    {
        if next_regular(b.timestamp_ms) != Some(b.timestamp_ms) {
            continue;
        }
        if !valid(b) || b.timestamp_ms != expected {
            out.status = "data-gap".into();
            return out;
        }
        expected = next_regular(b.timestamp_ms + 60000).unwrap_or(u64::MAX);
        out.favorable = out.favorable.max(if s.side == 1 {
            b.high - s.origin
        } else {
            s.origin - b.low
        });
        out.adverse = out.adverse.max(if s.side == 1 {
            s.origin - b.low
        } else {
            b.high - s.origin
        });
        if (b.close - s.invalidation) * sign <= 0. {
            out.status = "invalidated".into();
            out.resolved_at = Some(b.timestamp_ms + 60000);
            return out;
        }
        if (b.close - s.target) * sign >= 0. {
            out.status = "reached".into();
            out.resolved_at = Some(b.timestamp_ms + 60000);
            return out;
        }
    }
    if now >= deadline {
        out.status = if expected < deadline {
            "data-gap"
        } else {
            "expired"
        }
        .into();
        out.resolved_at = Some(deadline);
    }
    out
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Study {
    pub version: String,
    pub symbol: String,
    pub quality: Quality,
    pub outcomes: Vec<Outcome>,
    pub summaries: Vec<Summary>,
    pub probability_claim: bool,
    pub notes: Vec<String>,
    pub projection_samples: Vec<crate::route_4h_projection::Sample>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub key: String,
    pub n: usize,
    pub reached: usize,
    pub invalidated: usize,
    pub expired: usize,
    pub frequency: Option<f64>,
    pub lower95: Option<f64>,
    pub upper95: Option<f64>,
}
pub fn summarize(key: &str, rows: &[&Outcome]) -> Summary {
    let n = rows
        .iter()
        .filter(|r| matches!(r.status.as_str(), "reached" | "invalidated" | "expired"))
        .count();
    let reached = rows.iter().filter(|r| r.status == "reached").count();
    let p = if n > 0 {
        Some(reached as f64 / n as f64)
    } else {
        None
    };
    let (lo, hi) = p
        .map(|p| {
            let nf = n as f64;
            let z = 1.96_f64;
            let d = 1. + z * z / nf;
            let c = (p + z * z / (2. * nf)) / d;
            let h = z * ((p * (1. - p) / nf + z * z / (4. * nf * nf)).sqrt()) / d;
            (
                Some((100. * (c - h)).max(0.)),
                Some((100. * (c + h)).min(100.)),
            )
        })
        .unwrap_or((None, None));
    Summary {
        key: key.into(),
        n,
        reached,
        invalidated: rows.iter().filter(|r| r.status == "invalidated").count(),
        expired: rows.iter().filter(|r| r.status == "expired").count(),
        frequency: p.map(|v| v * 100.),
        lower95: lo,
        upper95: hi,
    }
}
/// Replay never includes the next bar when generating a scenario. Independent lanes suppress overlapping outcomes.
pub fn study(symbol: &str, input: &[Minute], now: u64, split: u64) -> Study {
    let (bars, q) = aggregate(input, now);
    let mut minutes = input.to_vec();
    minutes.sort_by_key(|b| b.timestamp_ms);
    let mut outcomes = vec![];
    let mut busy = BTreeMap::<String, u64>::new();
    for i in 12..=bars.len() {
        let prefix = &bars[..i];
        if prefix.iter().rev().take(12).any(|b| !b.complete) {
            continue;
        }
        let tail = prefix
            .iter()
            .rposition(|b| !b.complete)
            .map(|i| i + 1)
            .unwrap_or(0);
        let (_, _, scenario) = detect(symbol, &prefix[tail..]);
        let Some(s) = scenario else { continue };
        let Some(date) = calendar::date_at(s.at - 1) else {
            continue;
        };
        let Day::Open(session) = calendar::schedule(date) else {
            continue;
        };
        // Intraday only: no backtest silently counts overnight minutes as a ten-minute interval.
        for (horizon, deadline) in [
            ("10m", s.at + 600000),
            ("session", session.close_ms),
            ("swing2", swing_deadline(s.at, 2).unwrap_or(s.at)),
            ("swing10", swing_deadline(s.at, 10).unwrap_or(s.at)),
        ] {
            if (matches!(horizon, "10m" | "session") && deadline > session.close_ms)
                || deadline <= s.at
            {
                continue;
            }
            let partition = if s.at < split { "train" } else { "test" };
            if partition == "train" && deadline > split {
                continue;
            }
            let lane = format!("{partition}:{horizon}");
            if busy.get(&lane).is_some_and(|end| *end > s.at) {
                continue;
            }
            let mut r = resolve(&s, &minutes, deadline, now);
            r.horizon = horizon.into();
            busy.insert(lane, r.resolved_at.unwrap_or(deadline));
            outcomes.push(r);
        }
    }
    let mut groups = BTreeMap::<String, Vec<&Outcome>>::new();
    for r in &outcomes {
        let partition = if r.scenario.at < split {
            "train"
        } else {
            "test"
        };
        let key = format!(
            "{partition}:{}:{}:{}:{}:{}",
            r.horizon,
            r.scenario.setup,
            r.scenario.side,
            r.scenario.block_minutes,
            r.scenario.target_source
        );
        groups.entry(key).or_default().push(r);
    }
    let summaries = groups.into_iter().map(|(k, v)| summarize(&k, &v)).collect();
    Study {
        version: VERSION.into(),
        symbol: symbol.into(),
        quality: q,
        outcomes,
        summaries,
        probability_claim: false,
        projection_samples: crate::route_4h_projection::samples(symbol, input, now),
        notes: vec![
            "Baseline price-only; no past GEX/CVD/BID-ASK reconstructed".into(),
            "Frequency is descriptive, not a calibrated probability".into(),
            "Close-based endpoint; intraday excludes overnight; swing counts regular sessions"
                .into(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(s: &str) -> u64 {
        calendar::rfc3339_ms(s).unwrap()
    }
    fn minute(t: u64, c: f64) -> Minute {
        Minute {
            timestamp_ms: t,
            open: c,
            high: c + 1.,
            low: c - 1.,
            close: c,
            volume: 10.,
        }
    }
    fn day(date: &str) -> Vec<Minute> {
        let Day::Open(s) =
            calendar::schedule(calendar::date_at(calendar::day_bounds(date).unwrap().0).unwrap())
        else {
            panic!()
        };
        (s.open_ms..s.close_ms)
            .step_by(60000)
            .map(|t| minute(t, 100.))
            .collect()
    }
    fn scenario() -> Scenario {
        Scenario {
            id: "test".into(),
            symbol: "QQQ".into(),
            at: ms("2026-10-05T13:30:00-04:00"),
            side: 1,
            setup: "breakout".into(),
            structure: "up".into(),
            origin: 100.,
            target: 102.,
            target_source: "known-pivot".into(),
            invalidation: 98.,
            block_minutes: 240,
        }
    }
    #[test]
    fn session_alignment_and_partial_bar() {
        let rows = day("2026-10-05");
        let (b, q) = aggregate(&rows, ms("2026-10-05T16:00:00-04:00"));
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].duration_minutes, b[1].duration_minutes), (240, 150));
        assert_eq!(q.closed_blocks, 2);
        assert_eq!(b[0].end, ms("2026-10-05T13:30:00-04:00"));
        assert_eq!(aggregate(&rows, b[0].end - 1).0.len(), 0);
    }
    #[test]
    fn shortened_session_has_no_fake_four_hours() {
        let rows = day("2026-11-27");
        let (b, _) = aggregate(&rows, ms("2026-11-27T16:00:00-05:00"));
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].duration_minutes, 210);
        assert!(b[0].complete);
    }
    #[test]
    fn duplicates_and_gaps_poison_windows() {
        let mut rows = day("2026-10-05");
        rows.push(rows[0].clone());
        let (_, q) = aggregate(&rows, ms("2026-10-05T16:00:00-04:00"));
        assert_eq!(q.duplicate_minutes, 1);
        assert_eq!(q.incomplete_blocks, 1);
        rows.retain(|r| r.timestamp_ms >= ms("2026-10-05T13:30:00-04:00"));
        let mut prev = day("2026-10-02");
        prev.extend(rows);
        let (b, _) = aggregate(&prev, ms("2026-10-05T16:00:00-04:00"));
        assert!(!b[2].complete);
    }
    #[test]
    fn invalid_ohlc_and_future_data_cannot_confirm() {
        let mut rows = day("2026-10-05");
        rows[1].low = 110.;
        let (_, q) = aggregate(&rows, ms("2026-10-05T13:30:00-04:00"));
        assert_eq!(q.invalid_minutes, 1);
        assert_eq!(q.closed_blocks, 0);
    }
    #[test]
    fn pivot_available_only_after_two_confirming_bars() {
        let mut blocks = vec![];
        for i in 0..5 {
            blocks.push(Block {
                start: i * 1000,
                end: (i + 1) * 1000,
                date: "test".into(),
                duration_minutes: 240,
                open: 100.,
                high: if i == 2 { 110. } else { 101. },
                low: 99.,
                close: 100.,
                volume: 1.,
                complete: true,
            });
        }
        assert!(pivots(&blocks[..4]).is_empty());
        let ps = pivots(&blocks);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].confirmed_at, 5000);
        blocks[1].complete = false;
        assert!(pivots(&blocks).is_empty());
    }
    #[test]
    fn endpoints_use_close_not_wicks_and_freeze_target() {
        let s = scenario();
        let mut b = minute(s.at, 100.);
        b.high = 103.;
        b.low = 97.;
        assert_eq!(
            resolve(&s, &[b], s.at + 60000, s.at + 60000).status,
            "expired"
        );
        assert_eq!(
            resolve(&s, &[minute(s.at, 102.)], s.at + 60000, s.at + 60000).status,
            "reached"
        );
        assert_eq!(
            resolve(&s, &[minute(s.at, 98.)], s.at + 60000, s.at + 60000).status,
            "invalidated"
        );
    }
    #[test]
    fn incomplete_minutes_and_duplicate_execution_are_unknown() {
        let s = scenario();
        assert_eq!(
            resolve(
                &s,
                &[minute(s.at + 60000, 102.)],
                s.at + 120000,
                s.at + 120000
            )
            .status,
            "data-gap"
        );
        assert_eq!(
            resolve(&s, &[], s.at + 600000, s.at + 300000).status,
            "pending"
        );
        let b = minute(s.at, 100.);
        assert_eq!(
            resolve(&s, &[b.clone(), b], s.at + 120000, s.at + 120000).status,
            "data-gap"
        );
    }
    #[test]
    fn swing_skips_weekends_and_respects_dst() {
        let mut s = scenario();
        s.at = ms("2026-10-30T16:00:00-04:00");
        let next = ms("2026-11-02T09:30:00-05:00");
        assert_eq!(next_regular(s.at), Some(next));
        assert_eq!(
            resolve(
                &s,
                &[minute(next, 102.)],
                swing_deadline(s.at, 2).unwrap(),
                next + 60000
            )
            .status,
            "reached"
        );
    }
    #[test]
    fn put_outcome_is_symmetric() {
        let mut s = scenario();
        s.side = -1;
        s.target = 98.;
        s.invalidation = 102.;
        assert_eq!(
            resolve(&s, &[minute(s.at, 98.)], s.at + 60000, s.at + 60000).status,
            "reached"
        );
    }
    #[test]
    fn summary_excludes_unknown_and_never_claims_probability() {
        let s = scenario();
        let a = resolve(&s, &[minute(s.at, 102.)], s.at + 60000, s.at + 60000);
        let b = resolve(&s, &[], s.at + 60000, s.at + 60000);
        let summary = summarize("test", &[&a, &b]);
        assert_eq!(summary.n, 1);
        assert_eq!(summary.frequency, Some(100.));
        assert!(summary.lower95.unwrap() < 30.);
        let r = study("QQQ", &[], s.at, s.at);
        assert!(!r.probability_claim);
        assert!(r.outcomes.is_empty());
    }
    #[test]
    fn history_append_does_not_change_past_aggregates() {
        let rows = day("2026-10-05");
        let cutoff = ms("2026-10-05T13:30:00-04:00");
        let a = aggregate(&rows, cutoff).0;
        let b = aggregate(&rows[..240], cutoff).0;
        assert_eq!(
            serde_json::to_value(a).unwrap(),
            serde_json::to_value(b).unwrap()
        );
    }
    #[test]
    fn scenario_uses_only_known_levels_and_future_bars_cannot_rewrite_it() {
        let mut bars = vec![];
        for i in 0..14 {
            let close = if i == 13 { 111. } else { 100. };
            bars.push(Block {
                start: i * 14400000,
                end: (i + 1) * 14400000,
                date: "test".into(),
                duration_minutes: 240,
                open: 100.,
                high: match i {
                    2 => 120.,
                    8 => 110.,
                    13 => 112.,
                    _ => 105.,
                },
                low: match i {
                    5 => 90.,
                    10 => 92.,
                    _ => 98.,
                },
                close,
                volume: 10.,
                complete: true,
            });
        }
        let (_, _, s) = detect("QQQ", &bars);
        let s = s.unwrap();
        assert_eq!(s.target, 120.);
        assert_eq!(s.target_source, "known-pivot");
        assert_eq!(s.side, 1);
        let origin = serde_json::to_value(&s).unwrap();
        let before = read("QQQ", &[], 0);
        assert_eq!(before.state, "pending");
        bars.push(Block {
            start: 14 * 14400000,
            end: 15 * 14400000,
            date: "future".into(),
            duration_minutes: 240,
            open: 111.,
            high: 1000.,
            low: 1.,
            close: 500.,
            volume: 10.,
            complete: true,
        });
        assert_eq!(
            serde_json::to_value(detect("QQQ", &bars[..14]).2.unwrap()).unwrap(),
            origin
        );
    }
}
