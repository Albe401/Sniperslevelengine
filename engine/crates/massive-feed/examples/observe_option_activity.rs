use massive_feed::option_activity_observer;
use std::path::Path;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result=match args.first().map(String::as_str) {
        Some("prepare") if args.len()==4 => option_activity_observer::prepare(
            Path::new(&args[1]),&args[2],Path::new(&args[3])).await,
        Some("capture") if args.len()==2 => option_activity_observer::capture(Path::new(&args[1])).await,
        Some("arm") if args.len()==2 => option_activity_observer::arm(Path::new(&args[1])).await,
        Some("prepare-premium") if args.len()==4 => option_activity_observer::prepare_premium(
            Path::new(&args[1]),&args[2],Path::new(&args[3])).await,
        Some("capture-premium") if args.len()==2 => option_activity_observer::capture_premium(Path::new(&args[1])).await,
        _ => return Err("Usage: observe_option_activity prepare|prepare-premium BASELINE_JSON SYMBOL NEW_SESSION_DIR | arm SESSION_DIR | capture|capture-premium SESSION_DIR".into()),
    };
    match result {
        Ok(summary) => println!("{summary}"),
        Err(reason) => {
            println!(
                "{}",
                serde_json::json!({"status":"PENDING","reason":reason,"executionEnabled":false,"ordersSent":0})
            );
            std::process::exit(2);
        }
    }
    Ok(())
}
