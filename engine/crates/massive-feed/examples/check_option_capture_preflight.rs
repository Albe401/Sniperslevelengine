use massive_feed::{live::now_ms, option_activity_dataset, option_capture_preflight};
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: check_option_capture_preflight CONFIG_JSON NEW_REPORT_DIRECTORY; paths relative to cwd".into());
    }
    let report = option_capture_preflight::from_file(Path::new(&args[0]), now_ms())?;
    let file = option_activity_dataset::save_json(
        Path::new(&args[1]),
        "option-capture-preflight",
        &report,
    )?;
    println!(
        "{}",
        serde_json::json!({"status":report["status"],"phase":report["phase"],"blockers":report["blockers"],"file":file,"networkRequests":0,"captureStarted":false})
    );
    Ok(())
}
