//! Explicit capture-to-reference bridge; no scheduler or live formula changes.
use massive_feed::{
    live::now_ms, option_activity_dataset, option_activity_observer,
    option_premium_reference_accumulator,
};
use std::path::Path;

fn captured_window_certified(
    report: &serde_json::Value,
    session: &Path,
    capture: &serde_json::Value,
) -> bool {
    let Some(start) = capture["startMs"].as_u64() else {
        return false;
    };
    report["sourceRecords"].as_array().is_some_and(|records| {
        records.iter().any(|r| {
            r["kind"] == "prospective"
                && r["path"] == serde_json::json!(session)
                && r["certifiedSamples"].as_array().is_some_and(|samples| {
                    samples.iter().any(|s| s["startMs"].as_u64() == Some(start))
                })
        })
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 || !matches!(args[0].as_str(), "ingest" | "capture-and-ingest") {
        return Err("Usage: ingest_premium_capture ingest|capture-and-ingest PREVIOUS_SNAPSHOT_ABSOLUTE SESSION_DIRECTORY_ABSOLUTE FUTURE_CUTOFF_DATE OUTPUT_DIRECTORY".into());
    }
    let previous = Path::new(&args[1]);
    let session = Path::new(&args[2]);
    if !previous.is_absolute() || !session.is_absolute() {
        return Err("Snapshot y sesión requieren rutas absolutas".into());
    }
    // Validate the registry and future cutoff before requesting a live capture.
    let preflight = option_premium_reference_accumulator::build(
        &option_premium_reference_accumulator::Config {
            cutoff_date: args[3].clone(),
            dataset_files: vec![],
            prospective_directories: vec![],
            previous_snapshot: Some(previous.into()),
        },
        now_ms(),
    )?;
    if preflight["baseline"].is_null() {
        return Err(format!(
            "Referencia anterior bloqueada: {}",
            preflight["sourceErrors"]
        )
        .into());
    }
    let captured = if args[0] == "capture-and-ingest" {
        Some(option_activity_observer::capture_premium(session).await?)
    } else {
        None
    };
    let mut report = match option_premium_reference_accumulator::ingest_capture(
        previous,
        session,
        &args[3],
        now_ms(),
    ) {
        Ok(report) => report,
        Err(reason) => {
            println!(
                "{}",
                serde_json::json!({"status":"PENDING_INGESTION","reason":reason,"capture":captured,"previousSnapshot":previous,"executionEnabled":false,"ordersSent":0})
            );
            std::process::exit(2);
        }
    };
    if captured
        .as_ref()
        .is_some_and(|capture| !captured_window_certified(&report, session, capture))
    {
        println!(
            "{}",
            serde_json::json!({"status":"PENDING_INGESTION","reason":"La ventana recién capturada no tiene muestras certificadas","capture":captured,"previousSnapshot":previous,"executionEnabled":false,"ordersSent":0})
        );
        std::process::exit(2);
    }
    report["baselineFile"] = serde_json::json!(option_activity_dataset::save_json(
        Path::new(&args[4]),
        "premium-baseline",
        &report["baseline"]
    )?);
    let file =
        option_activity_dataset::save_json(Path::new(&args[4]), "premium-accumulator", &report)?;
    println!(
        "{}",
        serde_json::json!({"status":"CERTIFIED_CAPTURE_INGESTED","capture":captured,"snapshot":file,"baselineFile":report["baselineFile"],"summary":report["summary"],"executionEnabled":false,"ordersSent":0})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_windows_cannot_certify_the_new_capture() {
        let session = Path::new("session");
        let report = serde_json::json!({"sourceRecords":[{"kind":"prospective","path":"session","certifiedSamples":[{"startMs":100}]}]});
        assert!(!captured_window_certified(
            &report,
            session,
            &serde_json::json!({"startMs":200})
        ));
        assert!(captured_window_certified(
            &report,
            session,
            &serde_json::json!({"startMs":100})
        ));
        assert!(!captured_window_certified(
            &report,
            Path::new("other"),
            &serde_json::json!({"startMs":100})
        ));
    }
}
