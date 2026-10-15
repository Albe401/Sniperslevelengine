//! Offline report from an explicit directory manifest; never schedules collection.
use massive_feed::{live::now_ms, option_activity_dataset, option_joint_audit};
use serde::Deserialize;
use std::path::PathBuf;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    volume_directories: Vec<PathBuf>,
    premium_directories: Vec<PathBuf>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: audit_option_captures_joint MANIFEST_JSON OUTPUT_DIRECTORY (paths relative to working directory)".into());
    }
    if std::fs::metadata(&args[0])?.len() > 64_000 {
        return Err("Manifiesto demasiado grande".into());
    }
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let report = option_joint_audit::report(
        &manifest.volume_directories,
        &manifest.premium_directories,
        now_ms(),
    )?;
    let file = option_activity_dataset::save_json(
        &PathBuf::from(&args[1]),
        "joint-prospective-audit",
        &report,
    )?;
    println!(
        "{}",
        serde_json::json!({"status":report["status"],"summary":report["summary"],"file":file,"executionEnabled":false,"ordersSent":0})
    );
    Ok(())
}
