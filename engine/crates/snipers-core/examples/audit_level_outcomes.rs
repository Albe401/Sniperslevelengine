//! Read-only replay of frozen levels and saved experimental forecasts.
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let directory = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("capture-directory required")?,
    );
    let now: u64 = std::env::args()
        .nth(2)
        .ok_or("evaluation-ms required")?
        .parse()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&snipers_core::level_dataset::audit(&directory, now, true)?)?
    );
    Ok(())
}
