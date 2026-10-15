//! Offline consolidation; explicit directories only, no schedules or broker operations.
use massive_feed::{live::now_ms, option_activity_dataset, option_premium_audit};
use snipers_core::{calendar, option_activity_baseline};
use std::path::PathBuf;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        return Err(
            "Usage: audit_premium_captures CUTOFF_DATE NEW_OUTPUT_DIRECTORY SESSION_DIRECTORY..."
                .into(),
        );
    }
    let cutoff =
        calendar::legacy_eastern_ms(&format!("{} 09:30:00", args[0])).ok_or("Corte inválido")?;
    let now = now_ms();
    let dirs = args[2..].iter().map(PathBuf::from).collect::<Vec<_>>();
    let audit = option_premium_audit::consolidate(&dirs, now)?;
    let baseline = option_activity_baseline::build(&audit.samples, cutoff, 20, now)?;
    let output = PathBuf::from(&args[1]);
    let samples = option_activity_dataset::save_json(
        &output,
        "prospective-premium-samples",
        &serde_json::json!({"version":"reported-premium-dataset-v1","provenance":"audited-prospective-captures",
            "samples":audit.samples,"sessions":audit.sessions,"auditErrors":audit.errors}),
    )?;
    let reference =
        option_activity_dataset::save_json(&output, "prospective-premium-baseline", &baseline)?;
    let summary = serde_json::json!({"version":"prospective-premium-consolidation-v1",
        "status":if audit.samples.is_empty() {"WAITING_VALID_CAPTURE"} else if baseline.accepted_samples==0 {"AUDITED_REFERENCE_PENDING"} else {"AUDITED_EXPERIMENTAL"},
        "certifiedSamples":audit.samples.len(),"referenceAcceptedSamples":baseline.accepted_samples,
        "readyPremiumCohorts":baseline.cohorts.iter().filter(|c|c.premium_reference_ready).count(),
        "auditErrors":audit.errors.len(),"minimumSessions":20,"sampleFile":samples,"baselineFile":reference,
        "availableBeforeCutoff":baseline.available_before_cutoff,"executionEnabled":false,"ordersSent":0});
    option_activity_dataset::save_json(
        &output,
        "prospective-premium-audit",
        &serde_json::json!({"summary":summary,"sessions":audit.sessions,"errors":audit.errors}),
    )?;
    println!("{summary}");
    Ok(())
}
