//! Offline one-shot labels from original captures and explicit archived price evidence.
use massive_feed::{
    live::now_ms,
    option_activity_dataset,
    option_future_outcomes::{self, Policy, PriceEvidence},
};
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
fn read<T: serde::de::DeserializeOwned>(
    p: &std::path::Path,
    max: u64,
) -> Result<T, Box<dyn std::error::Error>> {
    if std::fs::metadata(p)?.len() > max {
        return Err("Archivo excede límite".into());
    }
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=3).contains(&args.len()) {
        return Err("Usage: audit_option_future_outcomes MANIFEST_JSON OUTPUT_DIRECTORY [BASELINE_ROUTE_JOURNAL_JSON]; paths relative to working directory".into());
    }
    let manifest: Manifest = read(&PathBuf::from(&args[0]), 64_000)?;
    if manifest.price_evidence_files.len() > 60 {
        return Err("Máximo 60 archivos de precios".into());
    }
    let policy: Policy = read(&manifest.policy_file, 64_000)?;
    let prices: Vec<PriceEvidence> = manifest
        .price_evidence_files
        .iter()
        .map(|p| read(p, 64_000_000))
        .collect::<Result<_, _>>()?;
    let mut report = option_future_outcomes::build(
        &manifest.volume_directories,
        &manifest.premium_directories,
        &prices,
        &policy,
        now_ms(),
    )?;
    report["inputFiles"] = serde_json::json!({"manifest":args[0],"policy":manifest.policy_file,"prices":manifest.price_evidence_files});
    if args.len() == 3 {
        let journal: serde_json::Value = read(&PathBuf::from(&args[2]), 64_000_000)?;
        massive_feed::option_formula_baseline::attach(&mut report, &journal, now_ms())?;
        report["inputFiles"]["baselineJournal"] = serde_json::json!(args[2]);
    }
    let file = option_activity_dataset::save_json(
        &PathBuf::from(&args[1]),
        "option-future-outcomes",
        &report,
    )?;
    println!(
        "{}",
        serde_json::json!({"status":report["status"],"cases":report["cases"].as_array().map(Vec::len),"labelStatusCounts":report["labelStatusCounts"],"file":file,"executionEnabled":false,"ordersSent":0})
    );
    Ok(())
}
