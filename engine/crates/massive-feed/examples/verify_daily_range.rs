//! Bounded REST-only audit while legacy sockets remain connected. No persistence.
use massive_feed::{credentials::Credentials, history, option_snapshot};

async fn audit() -> Result<(), &'static str> {
    let key = Credentials::load()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "Reloj anterior al epoch")?
        .as_millis() as u64;
    let bars = history::fetch(&key, "QQQ", now - 5 * 86_400_000, now).await?;
    let reference = bars.last().ok_or("Sin vela REST de referencia")?;
    let observation = option_snapshot::fetch(&key, "QQQ", reference.close, 4).await?;
    let range = snipers_core::daily_range::from_observation(&observation, reference.close)?;
    println!(
        "{}",
        serde_json::json!({
            "symbol":observation.symbol,"observedMs":observation.observed_ms,
            "referenceSource":"Cierre de última vela REST; no tick en vivo",
            "referenceBarMs":reference.timestamp_ms,"contracts":observation.contracts.len(),
            "websocketOpened":false,"range":range
        })
    );
    Ok(())
}
#[tokio::main]
async fn main() {
    match tokio::time::timeout(std::time::Duration::from_secs(120), audit()).await {
        Ok(Ok(())) => {}
        result => {
            eprintln!(
                "{}",
                match result {
                    Ok(Err(e)) => e,
                    _ => "Auditoría REST excedió 120 segundos",
                }
            );
            std::process::exit(1);
        }
    }
}
