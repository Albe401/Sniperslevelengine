//! Explicit bounded samples, no full redownload, formula changes or certification override.
use massive_feed::{
    credentials::Credentials,
    live::now_ms,
    option_activity_dataset::{self, Dataset},
    option_activity_history::Report,
    option_trade_identity_probe,
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
fn read<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T, Box<dyn std::error::Error>> {
    if std::fs::metadata(p)?.len() > 64_000_000 {
        return Err("Archivo demasiado grande".into());
    }
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: diagnose_option_trade_identity COLLECTION_DIRECTORY COMMA_SEPARATED_DATES:1..5 NEW_OUTPUT_DIRECTORY".into());
    }
    let dates: BTreeSet<_> = args[1].split(',').map(str::to_owned).collect();
    if dates.is_empty() || dates.len() > 5 {
        return Err("Entre 1 y 5 fechas explícitas".into());
    }
    let mut files = std::fs::read_dir(&args[0])?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with("dataset-") && s.ends_with(".json"))
        })
        .collect::<Vec<_>>();
    if files.len() > 60 {
        return Err("Máximo 60 datasets".into());
    }
    files.sort();
    let mut selected = vec![];
    let mut found = BTreeSet::new();
    for p in files {
        let d: Dataset = read(&p)?;
        if !dates.contains(&d.date) {
            continue;
        }
        if d.underlying != "QQQ" || d.reports.len() != 6 || !found.insert(d.date.clone()) {
            return Err("Dataset ajeno, duplicado o sin seis contratos".into());
        }
        selected.push(d);
    }
    if found != dates {
        return Err("Faltan fechas solicitadas".into());
    }
    let key = Credentials::load()?;
    let output = PathBuf::from(&args[2]);
    let mut probes = vec![];
    for d in selected {
        let mut seen = BTreeSet::new();
        for file in d.reports {
            let archived: Report = read(Path::new(&file))?;
            if archived.request.date != d.date
                || archived.request.underlying != "QQQ"
                || !seen.insert(archived.request.ticker.clone())
            {
                return Err("Referencia de reporte ajena o duplicada".into());
            }
            let result = match option_trade_identity_probe::probe(&archived, &key, &output).await {
                Ok(v) => v,
                Err(e) => {
                    serde_json::json!({"date":d.date,"ticker":archived.request.ticker,"status":"PROBE_PENDING","reason":e})
                }
            };
            println!(
                "{}",
                serde_json::json!({"date":result["date"],"ticker":result["ticker"],"status":result["status"],"rows":result["rows"],"fieldCounts":result["fieldCounts"]})
            );
            probes.push(result);
        }
    }
    let errors = probes
        .iter()
        .filter(|p| p["status"] == "PROBE_PENDING")
        .count();
    let report = serde_json::json!({"version":"option-trade-identity-diagnosis-v1","generatedMs":now_ms(),"dates":dates,"probes":probes,
        "status":if errors>0{"PARTIAL_DIAGNOSTIC"}else{"ORIGINAL_RESPONSES_INSPECTED"},"probeErrors":errors,
        "documentation":"https://www.massive.com/docs/rest/options/trades-quotes/trades",
        "limits":["First page only; does not certify entire contracts or prove availability for unprobed dates.","Source field values are preserved; missing keys remain missing.","Parser uses the same identity extractor as this probe; no fallback based on price, size or timestamp."],
        "formulaChanged":false,"certificationPolicyChanged":false,"executionEnabled":false,"ordersSent":0});
    let file = option_activity_dataset::save_json(&output, "identity-diagnosis", &report)?;
    println!(
        "{}",
        serde_json::json!({"status":report["status"],"file":file,"probeErrors":errors})
    );
    Ok(())
}
