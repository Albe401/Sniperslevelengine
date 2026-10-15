//! Offline observation: immutable output, no orders, network or live state.
use snipers_core::{
    option_activity_baseline::{Baseline, Sample},
    option_activity_detector,
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
        return Err(
            "Usage: evaluate_option_activity BASELINE_JSON DATASET_JSON NEW_OUTPUT_JSON".into(),
        );
    }
    let baseline: Baseline = serde_json::from_slice(&fs::read(&args[0])?)?;
    let dataset: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let samples: Vec<Sample> =
        serde_json::from_value(dataset.get("samples").ok_or("Missing samples")?.clone())?;
    let now = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let readings: Vec<_> = samples
        .iter()
        .map(|s| {
            serde_json::json!({"ticker":s.ticker,"startMs":s.start_ms,
            "reading":option_activity_detector::evaluate(&baseline,s,now)})
        })
        .collect();
    let mut reasons = BTreeMap::<String, usize>::new();
    for r in &readings {
        *reasons
            .entry(r["reading"]["reason"].as_str().unwrap().into())
            .or_default() += 1;
    }
    let summary = serde_json::json!({"version":"option-activity-detector-offline-v1",
        "datasetDate":dataset["date"],"windows":readings.len(),"reasons":reasons,
        "calibrated":false,"executionEnabled":false,"ordersSent":0});
    let output = serde_json::json!({"summary":summary,"readings":readings});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?;
    file.write_all(serde_json::to_string_pretty(&output)?.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!("{summary}");
    Ok(())
}
