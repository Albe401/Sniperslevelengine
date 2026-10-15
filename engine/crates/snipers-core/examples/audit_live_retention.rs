//! Synthetic multi-session LIVE/REST retention; no network, storage or orders.
use snipers_core::{
    calendar,
    off_exchange::Quote,
    off_exchange_tape::Engine,
    off_exchange_trade::{Source, Trade},
};
use std::time::{Duration, Instant};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let count: usize = args
        .get(1)
        .map_or("250000", String::as_str)
        .parse()
        .map_err(|_| "Invalid count")?;
    let sessions: usize = args
        .get(2)
        .map_or("20", String::as_str)
        .parse()
        .map_err(|_| "Invalid sessions")?;
    if !(1..=500_000).contains(&count) || !(7..=30).contains(&sessions) {
        return Err("Audit bounded to 500k prints/day and 7–30 sessions".into());
    }
    let mut engine = Engine::new("QQQ")?;
    let mut date = chrono::NaiveDate::from_ymd_opt(2026, 9, 1).ok_or("Invalid date")?;
    let mut completed = 0;
    while completed < sessions {
        let calendar::Day::Open(day) = calendar::schedule(date) else {
            date = date.succ_opt().ok_or("Date overflow")?;
            continue;
        };
        let started = Instant::now();
        let mut history = Vec::with_capacity(count);
        for id in 0..count {
            let timestamp = day.open_ms + id as u64 * (day.close_ms - day.open_ms) / count as u64;
            let mut row = Trade {
                symbol: "QQQ".into(),
                price: 500.0,
                shares: 300.0,
                sip_ms: timestamp,
                participant_ms: Some(timestamp),
                trf_ms: None,
                exchange: 4,
                trf_id: 12,
                trade_id: Some(format!("{id:024}")),
                sequence: Some(id as u64),
                conditions: vec![0, 12],
                tape: Some(3),
                correction: None,
                source: Source::Live,
            };
            engine.quote(Quote {
                bid: 499.0,
                ask: 500.0,
                timestamp_ms: timestamp,
            });
            engine.trade(row.clone(), timestamp);
            row.source = Source::History;
            history.push(row);
        }
        engine.history(&day.date, history, day.close_ms, true)?;
        let result = engine.snapshot(day.close_ms);
        let expected_history = completed.min(5);
        let valid = |s: &snipers_core::off_exchange_tape::Summary| {
            s.complete
                && s.print_count == count
                && s.total_shares == count as f64 * 300.0
                && s.pressure_pct == Some(100.0)
        };
        if !valid(&result.session)
            || result.history.len() != expected_history
            || !result.history.iter().all(valid)
            || result.diagnostics.capacity_drops != 0
        {
            return Err("Session/history lost volume, classification or bounded retention".into());
        }
        completed += 1;
        println!(
            "{}",
            serde_json::json!({"session":completed,"date":day.date,"prints":count,
            "retainedHistoryDays":result.history.len(),"complete":true,"pressurePct":result.session.pressure_pct,
            "elapsedMs":started.elapsed().as_millis()})
        );
        // Let the external monitor sample a stable checkpoint between sessions.
        std::thread::sleep(Duration::from_millis(300));
        date = date.succ_opt().ok_or("Date overflow")?;
    }
    Ok(())
}
