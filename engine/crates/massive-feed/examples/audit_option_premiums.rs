//! Offline certification of archived printed premium, no REST or live mutation.
use massive_feed::{
    live::{now_ms, Bar},
    option_activity_dataset::{self, Dataset},
    option_activity_history::Report,
    option_premium_dataset,
};
use snipers_core::{calendar, option_activity_baseline};
use std::{collections::BTreeMap, fs, path::PathBuf};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "Usage: audit_option_premiums PILOT_DIRECTORY CUTOFF_DATE NEW_OUTPUT_DIRECTORY".into(),
        );
    }
    let cutoff =
        calendar::legacy_eastern_ms(&format!("{} 09:30:00", args[1])).ok_or("Corte inválido")?;
    let output = PathBuf::from(&args[2]);
    let mut files = fs::read_dir(&args[0])?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with("dataset-") && s.ends_with(".json"))
        })
        .collect::<Vec<_>>();
    files.sort();
    if files.is_empty() || files.len() > 60 {
        return Err("Cantidad de datasets fuera de límite".into());
    }
    let mut evidence = vec![];
    let mut samples = vec![];
    let mut skips = BTreeMap::<String, usize>::new();
    let mut dates = std::collections::BTreeSet::new();
    for file in files {
        let data: Dataset = serde_json::from_slice(&fs::read(&file)?)?;
        dates.insert(data.date.clone());
        let context = data
            .context_reports
            .first()
            .ok_or("Dataset sin evidencia de subyacente")?;
        let stock: serde_json::Value = serde_json::from_slice(&fs::read(context)?)?;
        if stock["adjusted"] != false
            || stock["date"] != data.date
            || stock["underlying"] != data.underlying
        {
            return Err("Contexto de otro dataset o ajustado".into());
        }
        let bars = stock["bars"]
            .as_array()
            .ok_or("Contexto sin barras")?
            .iter()
            .map(|b| {
                Ok(Bar {
                    timestamp_ms: b["timestampMs"].as_u64().ok_or("Minuto sin reloj")?,
                    open: b["open"].as_f64().ok_or("OHLC ausente")?,
                    high: b["high"].as_f64().ok_or("OHLC ausente")?,
                    low: b["low"].as_f64().ok_or("OHLC ausente")?,
                    close: b["close"].as_f64().ok_or("OHLC ausente")?,
                    volume: b["volume"].as_f64().ok_or("Volumen ausente")?,
                    vwap: b["vwap"].as_f64(),
                })
            })
            .collect::<Result<Vec<_>, &str>>()?;
        for path in &data.reports {
            let report: Report = serde_json::from_slice(&fs::read(path)?)?;
            if report.request.date != data.date || report.request.underlying != data.underlying {
                return Err("Reporte de otra sesión o símbolo".into());
            }
            let (quality, accepted, skipped) = option_premium_dataset::samples(
                &report,
                &bars,
                data.opening_reference_available_ms
                    .ok_or("Selección sin reloj")?,
            );
            for (reason, n) in skipped {
                *skips.entry(reason).or_default() += n;
            }
            evidence.push(serde_json::json!({"file":path,"date":data.date,
                "ticker":report.request.ticker,"kind":report.contract.kind,"pages":report.pages,
                "rowsReceived":report.rows_received,"retrievalComplete":report.retrieval_complete,
                "acceptedSamples":accepted.len(),"quality":quality}));
            samples.extend(accepted);
        }
    }
    let baseline = option_activity_baseline::build(&samples, cutoff, 20, now_ms())?;
    let dataset_file = option_activity_dataset::save_json(
        &output,
        "premium-samples",
        &serde_json::json!({"version":"reported-premium-dataset-v1","dates":dates,"samples":samples,"skipped":skips}),
    )?;
    let baseline_file = option_activity_dataset::save_json(&output, "premium-baseline", &baseline)?;
    let certified = evidence
        .iter()
        .filter(|e| e["quality"]["status"] == "CERTIFIED_REPORTED_PREMIUM")
        .count();
    let total: f64 = evidence
        .iter()
        .filter_map(|e| e["quality"]["certifiedPremium"].as_f64())
        .sum();
    let summary = serde_json::json!({"version":"option-premium-pilot-certification-v1","sessions":dates.len(),
        "reports":evidence.len(),"certifiedReports":certified,"pendingReports":evidence.len()-certified,
        "certifiedGrossPremium":total,"certifiedWindowSamples":samples.len(),
        "referenceAcceptedSamples":baseline.accepted_samples,"premiumReadyCohorts":baseline.cohorts.iter().filter(|c|c.premium_reference_ready).count(),
        "status":baseline.status,"sampleFile":dataset_file,"baselineFile":baseline_file,
        "minimumSessions":20,"calibrated":false,"executionEnabled":false,"ordersSent":0});
    option_activity_dataset::save_json(
        &output,
        "premium-quality",
        &serde_json::json!({"summary":summary,"reports":evidence}),
    )?;
    println!("{summary}");
    Ok(())
}
