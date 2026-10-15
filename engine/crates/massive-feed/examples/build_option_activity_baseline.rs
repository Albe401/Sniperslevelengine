//! Explicit bounded sampling; immutable artifacts, no WebSocket or live-engine writes.
use massive_feed::{credentials::Credentials, live::now_ms, option_activity_dataset as dataset};
use snipers_core::calendar;
use std::path::PathBuf;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 4 || args.len() > 7 {
        return Err("Usage: build_option_activity_baseline SYMBOL CUTOFF_DATE SESSIONS OUTPUT_DIR [MAX_PAGES] [trades|volume|both] [PER_SIDE:1|3]".into());
    }
    let count = args[2].parse::<usize>()?;
    let max_pages = args.get(4).map(|s| s.parse()).transpose()?.unwrap_or(20);
    let mode = args.get(5).map(String::as_str).unwrap_or("both");
    let per_side = args
        .get(6)
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    if ![1, 3].contains(&per_side) {
        return Err("PER_SIDE must be 1 or 3".into());
    }
    if !["trades", "volume", "both"].contains(&mode) {
        return Err("Mode must be trades, volume or both".into());
    }
    let cutoff =
        calendar::legacy_eastern_ms(&format!("{} 09:30:00", args[1])).ok_or("Corte no válido")?;
    let dates = dataset::previous_sessions(&args[1], count)?;
    let key = Credentials::load()?;
    let directory = PathBuf::from(&args[3]);
    let mut datasets = vec![];
    let mut failures = vec![];
    for date in dates {
        eprintln!("Recopilando {} {} CALL/PUT", args[0], date);
        match dataset::collect_session_breadth(
            &key,
            &args[0],
            &date,
            &directory,
            max_pages,
            mode != "volume",
            mode != "trades",
            per_side,
        )
        .await
        {
            Ok(data) => {
                dataset::save_json(&directory, "dataset", &data)?;
                datasets.push(data);
            }
            Err(error) => {
                eprintln!("Sesión pendiente: {error}");
                failures.push(serde_json::json!({"date":date,"error":error}));
            }
        }
    }
    let baseline = dataset::baseline(&datasets, cutoff, now_ms())?;
    let file = dataset::save_json(&directory, "baseline", &baseline)?;
    let summary = serde_json::json!({"version":"option-activity-baseline-run-v1","baseline":file,"status":baseline.status,
        "sessionsRequested":count,"datasets":datasets.len(),"samples":baseline.accepted_samples,"excluded":baseline.excluded,"mode":mode,"perSide":per_side,
        "cohorts":baseline.cohorts.len(),"readyCohorts":baseline.cohorts.iter().filter(|c|c.volume_reference_ready||c.premium_reference_ready).count(),
        "availableBeforeCutoff":baseline.available_before_cutoff,"failures":failures,"calibrated":false,"ordersSent":0,"websocketOpened":false});
    dataset::save_json(&directory, "manifest", &summary)?;
    println!("{summary}");
    Ok(())
}
