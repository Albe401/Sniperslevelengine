//! Recertify original evidence before research fitting. No deployment or orders.
use massive_feed::{
    live::now_ms,
    option_activity_dataset,
    option_future_outcomes::{Policy, PriceEvidence},
    option_trial_units,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    volume_directories: Vec<PathBuf>,
    premium_directories: Vec<PathBuf>,
    price_evidence_files: Vec<PathBuf>,
    policy_file: PathBuf,
}
fn read<T: serde::de::DeserializeOwned>(
    p: &Path,
    max: u64,
) -> Result<T, Box<dyn std::error::Error>> {
    if std::fs::metadata(p)?.len() > max {
        return Err("Archivo demasiado grande".into());
    }
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(3..=4).contains(&args.len()) {
        return Err("Usage: train_option_trial OUTCOME_MANIFEST TRIAL_POLICY NEW_OUTPUT_DIRECTORY [ORIGINAL_ROUTE_JOURNAL]; paths relative to cwd".into());
    }
    let m: Manifest = read(Path::new(&args[0]), 64_000)?;
    if m.price_evidence_files.len() > 60 {
        return Err("Máximo 60 precios".into());
    }
    let policy: Policy = read(&m.policy_file, 64_000)?;
    let trial: serde_json::Value = read(Path::new(&args[1]), 64_000)?;
    let prices: Vec<PriceEvidence> = m
        .price_evidence_files
        .iter()
        .map(|p| read(p, 64_000_000))
        .collect::<Result<_, _>>()?;
    let journal: Option<serde_json::Value> = if args.len() == 4 {
        Some(read(Path::new(&args[3]), 64_000_000)?)
    } else {
        None
    };
    let now = now_ms();
    let units = option_trial_units::build(
        &m.volume_directories,
        &m.premium_directories,
        &prices,
        &policy,
        &trial,
        journal.as_ref(),
        now,
    )?;
    let source =
        option_activity_dataset::save_json(Path::new(&args[2]), "training-source-units", &units)?;
    let mut report = snipers_core::option_trial_training::train(&units, now_ms())?;
    report["sourceUnitsFile"] = serde_json::json!(source);
    report["inputFiles"] =
        serde_json::json!({"manifest":args[0],"trialPolicy":args[1],"baselineJournal":args.get(3)});
    let file =
        option_activity_dataset::save_json(Path::new(&args[2]), "option-trial-training", &report)?;
    println!(
        "{}",
        serde_json::json!({"status":report["status"],"file":file,"trainingSessions":report["trainingSessions"],"eligibleSessions":report["eligibleSessions"],"executionEnabled":false})
    );
    Ok(())
}
