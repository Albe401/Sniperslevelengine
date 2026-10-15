use massive_feed::option_outcome_prices;
use serde::Deserialize;
use std::path::PathBuf;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    volume_directories: Vec<PathBuf>,
    premium_directories: Vec<PathBuf>,
    price_evidence_files: Vec<PathBuf>,
    policy_file: PathBuf,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: collect_option_outcome_prices CAPTURE_MANIFEST_JSON OUTPUT_DIRECTORY (paths relative to working directory)".into());
    }
    if std::fs::metadata(&args[0])?.len() > 64_000 {
        return Err("Manifiesto demasiado grande".into());
    }
    let input: Manifest = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    if !input.price_evidence_files.is_empty() {
        return Err(
            "Usar manifiesto de captura sin precios: no se reemplazan archivos previos".into(),
        );
    }
    let result = option_outcome_prices::collect(
        &input.volume_directories,
        &input.premium_directories,
        &input.policy_file,
        &PathBuf::from(&args[1]),
    )
    .await?;
    println!("{result}");
    Ok(())
}
