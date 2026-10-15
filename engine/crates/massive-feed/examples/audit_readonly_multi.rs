//! Bounded REST-only audit. No WebSockets, orders, observations or credential output.
use massive_feed::{credentials::Credentials, live::now_ms, option_snapshot};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

async fn asset(key: &Credentials, symbol: &str) -> Result<Value, String> {
    let start = Instant::now();
    let now = now_ms();
    let bars = massive_feed::history::fetch(key, symbol, now - 5 * 86_400_000, now).await?;
    let latest = bars.last().ok_or("No stock history returned")?;
    let spot = latest.close;
    let snapshot = option_snapshot::fetch(key, symbol, spot, 4).await?;
    let horizons =
        snipers_core::horizons::calculate(&snapshot, spot, "REST audit; not proof of freshness")?;
    Ok(json!({
        "symbol":symbol,"elapsedMs":start.elapsed().as_millis(),"spot":spot,
        "referenceBarMs":latest.timestamp_ms,"historyBars":bars.len(),"observedDate":snapshot.observed_date,
        "contracts":snapshot.contracts.len(),
        "missingOi":snapshot.contracts.values().filter(|c|c.open_interest.is_none()).count(),
        "missingVolume":snapshot.contracts.values().filter(|c|c.volume.is_none()).count(),
        "missingGamma":snapshot.contracts.values().filter(|c|c.gamma.is_none()).count(),
        "horizons":horizons.iter().map(|h|json!({"mode":h.mode,"expirations":h.expirations,
            "certified":h.structure.complete,"readingAvailable":h.structure.reading.available,
            "provisional":h.structure.provisional})).collect::<Vec<_>>()
    }))
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let key = Credentials::load()?;
    let mut rows = vec![];
    for symbol in ["QQQ", "SPY", "NVDA", "GOOGL", "MSFT"] {
        let row = match tokio::time::timeout(Duration::from_secs(120), asset(&key, symbol)).await {
            Ok(Ok(value)) => value,
            Ok(Err(error)) => json!({"symbol":symbol,"error":error}),
            Err(_) => json!({"symbol":symbol,"error":"Timeout after 120 seconds"}),
        };
        println!("{row}");
        rows.push(row);
    }
    println!(
        "{}",
        json!({"finishedMs":now_ms(),"assets":rows.len(),
        "failures":rows.iter().filter(|r|r.get("error").is_some()).count(),"ordersSent":0})
    );
    if rows.iter().any(|r| r.get("error").is_some()) {
        return Err("One or more REST audits failed".into());
    }
    Ok(())
}
