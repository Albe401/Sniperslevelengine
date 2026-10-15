//! Read-only REST or saved-observation audit. No WebSockets, orders or persistence.
use massive_feed::{credentials::Credentials, option_snapshot};
use snipers_core::{horizons, options::Observation};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Pass saved observation path; append --refresh for REST")?;
    let saved: Observation = serde_json::from_slice(&std::fs::read(path)?)?;
    let spot = saved
        .contracts
        .values()
        .find_map(|c| c.underlying_price)
        .ok_or("No reference price")?;
    let observation = if std::env::args().any(|a| a == "--refresh") {
        let key = Credentials::load()?;
        tokio::time::timeout(
            std::time::Duration::from_secs(120),
            option_snapshot::fetch(&key, &saved.symbol, spot, 4),
        )
        .await??
    } else {
        saved
    };
    let horizons = horizons::calculate(&observation, spot, "saved reference (not live price)")?;
    println!(
        "{}",
        serde_json::json!({
            "symbol":observation.symbol, "date":observation.observed_date,
            "contracts":observation.contracts.len(),
            "missingOi":observation.contracts.values().filter(|c| c.open_interest.is_none()).count(),
            "missingVolume":observation.contracts.values().filter(|c| c.volume.is_none()).count(),
            "missingGamma":observation.contracts.values().filter(|c| c.gamma.is_none()).count(),
            "horizons":horizons.iter().map(|h| serde_json::json!({"mode":h.mode,"expirations":h.expirations,
                "certified":h.structure.complete,"readingAvailable":h.structure.reading.available,
                "provisional":h.structure.provisional})).collect::<Vec<_>>()
        })
    );
    Ok(())
}
