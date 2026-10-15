//! Synthetic integration fixture, exclusively in a new caller-supplied file.
use snipers_store::learning::{LearningStore, Observation};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Provide a NEW fixture database path")?,
    );
    // Never overwrite or open an existing production database.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let mut store = LearningStore::open(&path)?;
    let start = snipers_core::calendar::rfc3339_ms("2026-09-24T15:00:00Z").unwrap();
    for i in 0..=70 {
        let now = start + i * 30_000;
        store.record(&Observation {
            symbol: "QQQ".into(),
            observed_ms: now,
            price: Some(100. + i as f64 / 100.),
            price_ms: Some(now),
            eligible: true,
            quality: "SYNTHETIC_FIXTURE".into(),
            context: serde_json::json!({"synthetic":true,"engine_build":"fixture","analyst":null}),
        })?;
    }
    println!(
        "Synthetic fixture: 71 observations; expected observed outcomes: 5m=61, 15m=41, 30m=11."
    );
    Ok(())
}
