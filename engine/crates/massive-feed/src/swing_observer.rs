//! Passive selected-asset observer. Reuses existing feeds; never owns subscriptions or orders.
use crate::{
    live,
    swing_context::{self, ClosedBar, Engine},
};
use serde_json::{json, Value};
use snipers_core::calendar::{self, Day};
use std::{collections::BTreeMap, path::PathBuf};

fn capture(state: &live::SharedState, now: u64) -> Value {
    let mut s = state.lock().unwrap();
    let symbol = s.symbol.clone();
    let flow_expiry = s.scanner_options.bot_data(&symbol, now)["flowExpiry"].clone();
    let minutes = s
        .chart_history
        .minutes
        .iter()
        .rev()
        .take(240)
        .cloned()
        .collect::<Vec<_>>();
    let snap = &s.option_snapshot_status;
    let horizons=snap.horizons.iter().take(8).map(|h|json!({"mode":h.mode,"observedMs":h.observed_ms,"expirations":h.expirations,"exposure":h.exposure,"structure":{"complete":h.structure.complete,"provisional":h.structure.provisional.is_some(),"callWall":h.structure.call_wall,"putWall":h.structure.put_wall,"selected":h.structure.selected,"routes":h.structure.routes}})).collect::<Vec<_>>();
    json!({"symbol":symbol,"publishedMs":now,"startedMs":s.started_ms,"stocks":s.stocks,"options":s.options,"ticks":{symbol.clone():s.ticks.get(&symbol)},"metrics":s.metrics,"dailyAtr":s.daily_atr,"qqq_report":s.qqq_report,"cvd":s.cvd_engine.snapshot(&symbol,now,s.stocks.phase=="streaming"),"chartHistory":{"minutes":minutes},"flow":s.flow,"flowExpiry":flow_expiry,"optionSnapshotStatus":{"phase":snap.phase,"observedMs":snap.observed_ms,"expirations":snap.expirations,"horizons":horizons}})
}
/// Aggregate only 30 exact, unique, closed minute bars. No OHLC or volume fabrication.
pub fn closed_half_hours(frame: &Value, symbol: &str, now: u64) -> Vec<ClosedBar> {
    let Some(date) = calendar::date_at(now) else {
        return vec![];
    };
    let Day::Open(session) = calendar::schedule(date) else {
        return vec![];
    };
    let Some(minutes) = frame["chartHistory"]["minutes"]
        .as_array()
        .filter(|v| v.len() <= 240)
    else {
        return vec![];
    };
    let mut by_time = BTreeMap::new();
    for m in minutes {
        let Some(t) = m["timestampMs"].as_u64() else {
            return vec![];
        };
        if by_time.insert(t, m).is_some() {
            return vec![];
        }
    }
    let mut out = Vec::new();
    for start in (session.open_ms..session.close_ms).step_by(1_800_000) {
        let end = start + 1_800_000;
        if end > now || end > session.close_ms || now - end > 5_400_000 {
            continue;
        }
        let rows = (0..30)
            .map(|i| by_time.get(&(start + i * 60_000)).copied())
            .collect::<Option<Vec<_>>>();
        let Some(rows) = rows else {
            continue;
        };
        if rows.iter().any(|b| {
            let fields = ["open", "high", "low", "close", "volume"]
                .map(|k| b[k].as_f64().filter(|n| n.is_finite()));
            match fields {
                [Some(o), Some(h), Some(l), Some(c), Some(v)] => {
                    l <= 0. || h < o.max(c) || l > o.min(c) || v < 0.
                }
                _ => true,
            }
        }) {
            continue;
        }
        out.push(ClosedBar {
            symbol: symbol.into(),
            start_ms: start,
            open: rows[0]["open"].as_f64().unwrap(),
            high: rows
                .iter()
                .map(|b| b["high"].as_f64().unwrap())
                .fold(f64::NEG_INFINITY, f64::max),
            low: rows
                .iter()
                .map(|b| b["low"].as_f64().unwrap())
                .fold(f64::INFINITY, f64::min),
            close: rows[29]["close"].as_f64().unwrap(),
        });
    }
    out
}
fn append(path: &std::path::Path, row: &Value) -> std::io::Result<()> {
    use std::io::Write;
    if std::fs::metadata(path).is_ok_and(|m| m.len() > 10 * 1024 * 1024) {
        let previous = path.with_extension("previous.jsonl");
        if previous.exists() {
            std::fs::remove_file(&previous)?;
        }
        std::fs::rename(path, previous)?;
    }
    writeln!(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?,
        "{row}"
    )
}
pub async fn run(state: live::SharedState, directory: PathBuf) {
    let cache_path = directory.join("swing-observation-state-v1.json");
    let mut engines: BTreeMap<String, Engine> = std::fs::metadata(&cache_path)
        .ok()
        .filter(|m| m.len() <= 1_048_576)
        .and_then(|_| std::fs::read(&cache_path).ok())
        .and_then(|b| serde_json::from_slice::<BTreeMap<String, Engine>>(&b).ok())
        .filter(|m| m.len() <= 8)
        .unwrap_or_default();
    let mut last_key = String::new();
    let mut last_minute = 0;
    let mut journal_error = None;
    loop {
        let now = live::now_ms();
        let frame = capture(&state, now);
        let symbol = frame["symbol"].as_str().unwrap_or("").to_owned();
        if !engines.contains_key(&symbol) && engines.len() >= 8 {
            engines.remove(engines.keys().next().cloned().as_deref().unwrap_or(""));
        }
        let mut reading = crate::intraday_strategy::read(&frame, now);
        crate::quick_bot_context::apply(&frame, &mut reading, now);
        let input = swing_context::intraday_evidence(&frame, &reading, now);
        let bars = closed_half_hours(&frame, &symbol, now);
        let report = engines.entry(symbol.clone()).or_default().update(
            &frame,
            &symbol,
            frame["stocks"]["phase"] == "streaming",
            now,
            &input,
            &bars,
        );
        let mut published = json!(report);

        let key = format!(
            "{}:{}:{}:{:?}:{}",
            symbol,
            report.state,
            report.context.side,
            report.context.snapshot_at,
            report.intraday.state
        );
        if key != last_key || now / 60_000 != last_minute {
            let row = json!({"version":swing_context::VERSION,"at":now,"symbol":symbol,"state":report.state,"side":report.context.side,"spot":report.context.spot,"plan":report.plan,"intraday":report.intraday,"remainingDollars":report.remaining_dollars,"remainingDailyAtr":report.remaining_daily_atr,"reason":report.reason,"blockers":report.context.blockers});
            let path = directory.join("swing-observation-v1.jsonl");
            let saved = serde_json::to_vec(&engines).unwrap_or_default();
            let saved_path = cache_path.clone();
            match tokio::task::spawn_blocking(move || {
                append(&path, &row)?;
                std::fs::write(saved_path, saved)
            })
            .await
            {
                Ok(Ok(())) => journal_error = None,
                _ => journal_error = Some("No se pudo guardar el registro de observación"),
            }
            last_key = key;
            last_minute = now / 60_000;
        }
        published["journalError"] = json!(journal_error);
        published["mode"] = json!("observation-only");
        {
            let mut s = state.lock().unwrap();
            if s.symbol == symbol {
                s.swing_report = published;
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Value, u64) {
        let now = calendar::rfc3339_ms("2026-10-05T10:00:00-04:00").unwrap();
        let rows=(0..30).map(|i|json!({"timestampMs":now-1_800_000+i*60_000,"open":100.,"high":102.,"low":99.,"close":101.,"volume":10.})).collect::<Vec<_>>();
        (json!({"chartHistory":{"minutes":rows}}), now)
    }
    #[test]
    fn only_complete_closed_buckets() {
        let (mut f, n) = fixture();
        assert_eq!(closed_half_hours(&f, "QQQ", n).len(), 1);
        assert!(closed_half_hours(&f, "QQQ", n - 1).is_empty());
        f["chartHistory"]["minutes"].as_array_mut().unwrap().pop();
        assert!(closed_half_hours(&f, "QQQ", n).is_empty());
    }
    #[test]
    fn rejects_duplicates_and_invalid_ohlc() {
        let (mut f, n) = fixture();
        let row = f["chartHistory"]["minutes"][0].clone();
        f["chartHistory"]["minutes"]
            .as_array_mut()
            .unwrap()
            .push(row);
        assert!(closed_half_hours(&f, "QQQ", n).is_empty());
        let (mut f, n) = fixture();
        f["chartHistory"]["minutes"][0]["high"] = json!(98.);
        assert!(closed_half_hours(&f, "QQQ", n).is_empty());
    }
    #[test]
    fn aggregation_is_bounded_and_fast() {
        let (f, n) = fixture();
        let t = std::time::Instant::now();
        for _ in 0..10000 {
            assert_eq!(closed_half_hours(&f, "QQQ", n).len(), 1);
        }
        println!(
            "30-minute aggregation: {:.4} ms",
            t.elapsed().as_secs_f64() * 1000. / 10000.
        );
    }
}
