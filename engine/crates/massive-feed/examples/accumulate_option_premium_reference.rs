use massive_feed::{live::now_ms, option_activity_dataset, option_premium_reference_accumulator};
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: accumulate_option_premium_reference CONFIG_JSON NEW_OUTPUT_DIRECTORY; sources use absolute paths".into());
    }
    let mut report =
        option_premium_reference_accumulator::from_file(Path::new(&args[0]), now_ms())?;
    if !report["baseline"].is_null() {
        let file = option_activity_dataset::save_json(
            Path::new(&args[1]),
            "premium-baseline",
            &report["baseline"],
        )?;
        report["baselineFile"] = serde_json::json!(file);
    }
    let file =
        option_activity_dataset::save_json(Path::new(&args[1]), "premium-accumulator", &report)?;
    println!(
        "{}",
        serde_json::json!({"status":report["status"],"file":file,"summary":report["summary"],"sourceErrors":report["sourceErrors"],"baselineFile":report["baselineFile"]})
    );
    Ok(())
}
