//! Read-only capture replay. Writes a new report to stdout; never changes historical data.
use crate::level_research::{self, Case, ObservedBar};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader},
    path::Path,
};
pub fn audit(
    directory: &Path,
    now: u64,
    full_levels: bool,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let mut files: Vec<_> = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "jsonl")
                && p.file_name()
                    .is_some_and(|s| s.to_string_lossy().starts_with("research-"))
        })
        .collect();
    files.sort();
    if !full_levels
        && files.iter().try_fold(0_u64, |sum, p| {
            Ok::<_, std::io::Error>(sum.saturating_add(fs::metadata(p)?.len()))
        })? > 512 * 1024 * 1024
    {
        return Err(std::io::Error::other("El historial supera 512 MiB: evaluación de escritorio pendiente de indexación; usa la auditoría offline.").into());
    }
    let mut errors = vec![];
    let mut captures = BTreeMap::new();
    let mut conflicts = std::collections::BTreeSet::new();
    for file in &files {
        for (n, line) in BufReader::new(fs::File::open(file)?).lines().enumerate() {
            let line = line?;
            let row: Value = match serde_json::from_str(&line) {
                Ok(row) => row,
                Err(e) => {
                    errors.push(json!({"file":file.file_name(),"line":n+1,"error":e.to_string()}));
                    continue;
                }
            };
            if row["version"] != "research-capture-v1"
                || row["id"].as_str().is_none()
                || row["capturedMs"].as_u64().is_none()
            {
                errors.push(
                    json!({"file":file.file_name(),"line":n+1,"error":"incompatible-capture"}),
                );
                continue;
            }
            if row["capturedMs"].as_u64().unwrap() > now {
                continue;
            }
            let id = row["id"].as_str().unwrap().to_owned();
            if let Some(old) = captures.get(&id) {
                if old != &row {
                    errors.push(json!({"id":id,"error":"conflicting-capture-id"}));
                    conflicts.insert(id);
                }
            } else {
                captures.insert(id, row);
            }
        }
    }
    captures.retain(|id, _| !conflicts.contains(id));
    let mut cases: Vec<Case> = vec![];
    let mut bars: BTreeMap<String, Vec<ObservedBar>> = BTreeMap::new();
    let mut blockers: BTreeMap<String, u64> = BTreeMap::new();
    let mut supported_levels = 0;
    let mut captured_catalogs = 0;
    let mut forecasts: Vec<crate::arrival_baseline::Forecast> = vec![];
    for row in captures.values() {
        // Original frozen catalog is authoritative. Legacy P0 rows are explicitly reconstructed.
        let cat = if row["levelCatalog"].is_object() {
            captured_catalogs += 1;
            match serde_json::from_value(row["levelCatalog"].clone()) {
                Ok(catalog) => catalog,
                Err(e) => {
                    errors.push(json!({"id":row["id"],"error":format!("invalid-catalog:{e}")}));
                    continue;
                }
            }
        } else {
            level_research::catalog(row)
        };
        if cat.version != "level-contract-v1"
            || cat.observation_id != row["id"].as_str().unwrap()
            || cat.cases.iter().any(|c| {
                c.level.symbol != row["symbol"].as_str().unwrap_or("")
                    || c.captured_ms != row["capturedMs"].as_u64().unwrap()
            })
        {
            errors.push(json!({"id":row["id"],"error":"incompatible-frozen-catalog"}));
            continue;
        }
        supported_levels += cat.levels.len();
        for raw in row["arrivalBaseline"]["forecasts"]
            .as_array()
            .into_iter()
            .flatten()
        {
            match serde_json::from_value::<crate::arrival_baseline::Forecast>(raw.clone()) {
                Ok(f)
                    if f.version == crate::arrival_baseline::VERSION
                        && !f.calibrated
                        && f.issued_ms == row["capturedMs"].as_u64().unwrap()
                        && f.probability.is_finite()
                        && f.probability >= 0.
                        && f.probability <= 1.
                        && f.returns >= 60
                        && f.sigma_per_minute.is_finite()
                        && f.sigma_per_minute > 0.
                        && f.volatility_through_ms <= f.issued_ms
                        && f.horizon_trading_minutes > 0
                        && cat.cases.iter().any(|c| {
                            serde_json::to_value(c).ok() == serde_json::to_value(&f.case).ok()
                        }) =>
                {
                    forecasts.push(f)
                }
                _ => errors.push(json!({"id":row["id"],"error":"invalid-saved-forecast"})),
            }
        }
        if full_levels {
            cases.extend(cat.cases);
        }
        for reason in cat.blockers {
            *blockers.entry(reason).or_default() += 1;
        }
        let Some(symbol) = row["symbol"].as_str() else {
            continue;
        };
        let available = row["availableMs"].as_u64().unwrap_or(0);
        if available > now || available < row["capturedMs"].as_u64().unwrap() {
            continue;
        }
        for b in row["evidence"]["chartHistory"]["minutes"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let read = || {
                Some(ObservedBar {
                    symbol: symbol.into(),
                    timestamp_ms: b["timestampMs"].as_u64()?,
                    available_ms: available,
                    open: b["open"].as_f64()?,
                    high: b["high"].as_f64()?,
                    low: b["low"].as_f64()?,
                    close: b["close"].as_f64()?,
                })
            };
            if let Some(b) = read() {
                bars.entry(symbol.into()).or_default().push(b);
            } else {
                errors.push(json!({"id":row["id"],"error":"incompatible-bar"}));
            }
        }
    }
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut labels = vec![];
    for case in &cases {
        let label = level_research::evaluate(
            case,
            bars.get(&case.level.symbol)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            now,
        );
        *counts.entry(label.status.clone()).or_default() += 1;
        labels.push(json!({"case":case,"label":label}));
    }
    let mut baseline_labels = vec![];
    let mut metrics: BTreeMap<String, (u64, f64, f64, f64, u64)> = BTreeMap::new();
    let mut baseline_status: BTreeMap<String, u64> = BTreeMap::new();
    for f in &forecasts {
        let label = level_research::evaluate(
            &f.case,
            bars.get(&f.case.level.symbol)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            now,
        );
        *baseline_status.entry(label.status.clone()).or_default() += 1;
        if let Some(hit) = label.binary_arrival {
            let y = if hit { 1. } else { 0. };
            let p = f.probability;
            let clamped = p.clamp(1e-12, 1. - 1e-12);
            let m = metrics
                .entry(format!("{}:{}", f.case.level.symbol, f.case.level.horizon))
                .or_default();
            m.0 += 1;
            m.1 += (p - y).powi(2);
            m.2 += -y * clamped.ln() - (1. - y) * (1. - clamped).ln();
            m.3 += p;
            m.4 += u64::from(hit);
        }
        baseline_labels.push(json!({"forecast":f,"label":label}));
    }
    let metrics:Vec<_>=metrics.into_iter().map(|(key,(n,brier,loss,p,hits))|json!({"group":key,"n":n,"brier":brier/n as f64,"logLoss":loss/n as f64,"meanPredicted":p/n as f64,"observedArrival":hits as f64/n as f64,"validated":false})).collect();
    Ok(
        json!({"version":"level-dataset-audit-v1","evaluatedMs":now,"files":files.len(),"observations":captures.len(),"capturedCatalogs":captured_catalogs,"reconstructedCatalogs":captures.len()-captured_catalogs,"supportedLevels":supported_levels,"cases":cases.len(),"statusCounts":counts,"blockers":blockers,"errors":errors,"labels":labels,"baselineForecasts":forecasts.len(),"baselineStatusCounts":baseline_status,"baselineMetrics":metrics,"baselineLabels":baseline_labels,"trainingValidated":false,"predictionEnabled":false,"note":"Minute OHLC labels; correlated cases are not independent samples. Frozen forecasts only; no hindsight recomputation."}),
    )
}
