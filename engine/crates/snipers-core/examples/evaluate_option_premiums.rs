//! Offline strict-clock premium evaluation. Does not rewrite historical availability.
use snipers_core::{
    option_activity_baseline::{Baseline, Sample},
    option_premium_detector,
};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: evaluate_option_premiums BASELINE_JSON CERTIFIED_PREMIUM_DATASET_JSON NEW_OUTPUT_JSON".into());
    }
    let baseline: Baseline = serde_json::from_slice(&fs::read(&args[0])?)?;
    let data: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    if data["version"] != "reported-premium-dataset-v1" {
        return Err("Dataset no certificado de primas".into());
    }
    let samples: Vec<Sample> = serde_json::from_value(data["samples"].clone())?;
    let now = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let mut reasons = BTreeMap::<String, usize>::new();
    let readings: Vec<_> = samples
        .iter()
        .map(|s| {
            let r = option_premium_detector::evaluate(&baseline, s, now);
            *reasons.entry(r.reason.clone()).or_default() += 1;
            serde_json::json!({"ticker":s.ticker,"startMs":s.start_ms,"reading":r})
        })
        .collect();
    let summary = serde_json::json!({"version":"option-premium-detector-offline-v1","windows":samples.len(),
        "reasons":reasons,"calibrated":false,"executionEnabled":false,"ordersSent":0});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?;
    serde_json::to_writer_pretty(
        &mut file,
        &serde_json::json!({"summary":summary,"readings":readings}),
    )?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!("{summary}");
    Ok(())
}
