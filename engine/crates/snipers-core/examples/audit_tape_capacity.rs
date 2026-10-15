//! Synthetic offline load; no network, files, orders or private account data.
use snipers_core::{
    calendar,
    off_exchange_tape::Engine,
    off_exchange_trade::{Source, Trade},
};
use std::time::Instant;

fn main() -> Result<(), String> {
    let count: usize = std::env::args()
        .nth(1)
        .as_deref()
        .unwrap_or("2568520")
        .parse()
        .map_err(|_| "Invalid count")?;
    if count == 0 || count > snipers_core::off_exchange_tape::SESSION_LIMIT {
        return Err("Count outside the engine capacity".into());
    }
    let date = "2026-09-25";
    let (start, end) = calendar::day_bounds(date).ok_or("Invalid date")?;
    let now = end + 3_600_000;
    let mut engine = Engine::new("QQQ")?;
    for round in 1..=3 {
        let started = Instant::now();
        let rows = (0..count)
            .map(|id| Trade {
                symbol: "QQQ".into(),
                price: 500.0 + (id % 100) as f64 / 100.0,
                shares: 300.0,
                sip_ms: start + 36_000_000 + id as u64,
                participant_ms: None,
                trf_ms: None,
                exchange: 4,
                trf_id: 12,
                trade_id: Some(format!("{id:024}")),
                sequence: Some(id as u64),
                conditions: vec![0, 12],
                tape: Some(3),
                correction: None,
                source: Source::History,
            })
            .collect();
        engine.history(date, rows, now, true)?;
        let result = engine.snapshot(now);
        let day = result
            .history
            .iter()
            .find(|d| d.date == date)
            .ok_or("Missing archived day")?;
        if !day.complete || day.print_count != count || day.total_shares != count as f64 * 300.0 {
            return Err("Load result lost completeness or duplicated rows".into());
        }
        println!(
            "{}",
            serde_json::json!({"round":round,"prints":day.print_count,"complete":day.complete,"elapsedMs":started.elapsed().as_millis(),"summaryBytes":serde_json::to_vec(day).unwrap().len()})
        );
    }
    Ok(())
}
