//! Offline re-certification, strict provenance and distinct-session consolidation.
use crate::{
    option_activity_history::WindowReport,
    option_activity_observer::{self as observer, FrozenSession},
};
use serde_json::Value;
use snipers_core::option_activity_baseline::Sample;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Default)]
pub struct Consolidation {
    pub samples: Vec<Sample>,
    pub sessions: Vec<Value>,
    pub errors: Vec<Value>,
}
pub(crate) fn read(path: &Path) -> Result<Value, String> {
    let size = fs::metadata(path).map_err(|_| "Archivo ausente")?.len();
    if size > 64_000_000 {
        return Err("Archivo supera límite de 64 MB".into());
    }
    serde_json::from_slice(&fs::read(path).map_err(|_| "Lectura fallida")?)
        .map_err(|_| "JSON inválido".into())
}
pub(crate) fn equivalent(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if let (Some(x), Some(y)) = (x.as_u64(), y.as_u64()) {
                return x == y;
            }
            match (x.as_f64(), y.as_f64()) {
                (Some(x), Some(y)) => {
                    x.is_finite()
                        && y.is_finite()
                        && (x - y).abs() <= 1e-8 + 1e-10 * x.abs().max(y.abs())
                }
                _ => false,
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| equivalent(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|b| equivalent(v, b)))
        }
        _ => a == b,
    }
}
/// Reject an entire window if any record differs from its independently re-evaluated evidence.
pub fn verify_window(
    frozen: &FrozenSession,
    value: &Value,
    now: u64,
) -> Result<(Vec<Sample>, BTreeMap<String, usize>, usize), String> {
    let start = value["startMs"].as_u64().ok_or("Ventana sin inicio")?;
    let end = value["endMs"].as_u64().ok_or("Ventana sin cierre")?;
    let began = value["captureStartedMs"]
        .as_u64()
        .ok_or("Captura sin reloj inicial")?;
    let completed = value["captureCompletedMs"]
        .as_u64()
        .ok_or("Captura sin reloj final")?;
    if value["version"] != "option-premium-prospective-window-v1"
        || value["date"] != frozen.date
        || value["underlying"] != frozen.underlying
        || value["frozenMs"] != frozen.frozen_ms
        || value["referenceBuiltMs"] != frozen.baseline.built_ms
        || value["executionEnabled"] != false
        || value["calibrated"] != false
        || value["ordersSent"] != 0
        || began > completed
        || completed > now
        || observer::closed_window(frozen, began) != (Ok((start, end)))
    {
        return Err("Metadatos o relojes de captura incompatibles".into());
    }
    observer::validate_reference(&frozen.baseline, &frozen.session, frozen.frozen_ms)?;
    let selected: BTreeSet<_> = observer::selected_tickers(frozen)?.into_iter().collect();
    let records = value["observations"]
        .as_array()
        .ok_or("Captura sin observaciones")?;
    if records.len() != selected.len() {
        return Err("Faltan observaciones de contratos".into());
    }
    let mut seen = BTreeSet::new();
    let mut samples = vec![];
    let mut statuses = BTreeMap::new();
    let mut alerts = 0;
    for record in records {
        let ticker = record["ticker"].as_str().ok_or("Registro sin contrato")?;
        if !selected.contains(ticker) || !seen.insert(ticker.to_owned()) {
            return Err("Contrato ajeno o duplicado en ventana".into());
        }
        let status = record["status"].as_str().ok_or("Registro sin estado")?;
        *statuses.entry(status.to_owned()).or_default() += 1;
        if record["evidence"].is_null() {
            if !["PENDING_CONTEXT", "PENDING_CAPTURE_LATE", "PENDING_SOURCE"].contains(&status)
                || record["alertEligible"] != false
                || !record["sample"].is_null()
                || !record["reading"].is_null()
                || !record["quality"].is_null()
            {
                return Err("Registro pendiente contiene datos no certificados".into());
            }
            if let Some(clock) = record["completedMs"].as_u64() {
                if clock < began || clock > completed {
                    return Err("Reloj pendiente fuera de captura".into());
                }
            }
            continue;
        }
        let evidence: WindowReport =
            serde_json::from_value(record["evidence"].clone()).map_err(|_| "Evidencia inválida")?;
        let clock = record["completedMs"]
            .as_u64()
            .ok_or("Observación sin reloj")?;
        if evidence.start_ms != start
            || evidence.end_ms != end
            || evidence.report.request.ticker != ticker
            || evidence.capture_started_ms < began
            || clock < evidence.report.collected_ms
            || clock > completed
        {
            return Err("Evidencia de otra ventana o reloj posterior".into());
        }
        let reconstructed =
            observer::premium_observation(frozen, &value["stockEvidence"], &evidence, clock)?;
        // JSON floating point comparisons use the same parser/serializer as the observer.
        for field in [
            "status",
            "quality",
            "sample",
            "reading",
            "timelyCapture",
            "alertEligible",
        ] {
            if !equivalent(&reconstructed[field], &record[field]) {
                return Err(format!("Registro alterado: {field}"));
            }
        }
        if record["alertEligible"] == true {
            alerts += 1;
        }
        if record["timelyCapture"] == true && !reconstructed["sample"].is_null() {
            samples.push(
                serde_json::from_value(reconstructed["sample"].clone())
                    .map_err(|_| "Muestra inválida")?,
            );
        }
    }
    Ok((samples, statuses, alerts))
}
pub fn consolidate(directories: &[std::path::PathBuf], now: u64) -> Result<Consolidation, String> {
    if directories.is_empty() || directories.len() > 60 {
        return Err("Entre 1 y 60 sesiones explícitas".into());
    }
    let mut result = Consolidation::default();
    let mut sessions_seen = BTreeSet::new();
    for directory in directories {
        if !directory.join("frozen-session.json").exists() {
            result.sessions.push(
                serde_json::json!({"directory":directory,"status":"WAITING_CAPTURE",
                "certifiedSamples":0,"observedContractWindows":0}),
            );
            continue;
        }
        let frozen: FrozenSession =
            serde_json::from_value(read(&directory.join("frozen-session.json"))?)
                .map_err(|_| "Sesión congelada inválida")?;
        if !frozen.premium_mode
            || frozen.frozen_ms > now
            || !sessions_seen.insert((frozen.underlying.clone(), frozen.date.clone()))
        {
            return Err("Sesión futura, de volumen o duplicada".into());
        }
        observer::validate_reference(&frozen.baseline, &frozen.session, frozen.frozen_ms)?;
        let selected = observer::selected_tickers(&frozen)?;
        let mut files = fs::read_dir(directory)
            .map_err(|_| "Carpeta inaccesible")?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.starts_with("premium-window-") && s.ends_with(".json"))
            })
            .collect::<Vec<_>>();
        if files.len() > 78 {
            return Err("Demasiados archivos de ventana".into());
        }
        files.sort();
        let mut window_starts = BTreeSet::new();
        let mut counts = BTreeMap::<String, usize>::new();
        let mut certified = 0;
        let mut invalid = 0;
        let mut alerts = 0;
        let mut valid = 0;
        let mut lags = vec![];
        for file in files {
            let checked = (|| {
                let value = read(&file)?;
                let start = value["startMs"].as_u64().ok_or("Inicio ausente")?;
                let expected = format!("premium-window-{start}.json");
                if file.file_name().and_then(|s| s.to_str()) != Some(expected.as_str())
                    || !window_starts.insert(start)
                {
                    return Err(String::from("Nombre o ventana duplicada"));
                }
                let verified = verify_window(&frozen, &value, now)?;
                let lag = value["captureCompletedMs"].as_u64().unwrap()
                    - value["endMs"].as_u64().unwrap();
                Ok((verified, lag))
            })();
            match checked {
                Ok(((samples, statuses, n), lag)) => {
                    lags.push(lag);
                    certified += samples.len();
                    alerts += n;
                    valid += 1;
                    result.samples.extend(samples);
                    for (k, v) in statuses {
                        *counts.entry(k).or_default() += v;
                    }
                }
                Err(reason) => {
                    invalid += 1;
                    result
                        .errors
                        .push(serde_json::json!({"file":file,"reason":reason}));
                }
            }
        }
        // Count only windows whose freshness deadline has elapsed by the audit clock.
        let due = (frozen.eligible_from_ms..frozen.session.close_ms)
            .step_by(300_000)
            .filter(|start| start + 300_000 + 120_000 <= now)
            .count();
        let missing = (frozen.eligible_from_ms..frozen.session.close_ms)
            .step_by(300_000)
            .filter(|start| start + 300_000 + 120_000 <= now && !window_starts.contains(start))
            .count();
        let observed = valid * selected.len();
        let eligible = due * selected.len();
        lags.sort_unstable();
        let comparable = counts
            .get("UNUSUAL_PREMIUM_EXPERIMENTAL")
            .copied()
            .unwrap_or(0)
            + counts
                .get("WITHIN_REFERENCE_EXPERIMENTAL")
                .copied()
                .unwrap_or(0);
        result.sessions.push(serde_json::json!({"directory":directory,"date":frozen.date,
            "underlying":frozen.underlying,"status":if valid==0 {"WAITING_VALID_CAPTURE"} else {"AUDITED"},
            "elapsedEligibleWindows":due,"missingElapsedWindows":missing,"validCapturedWindows":valid,"invalidCapturedWindows":invalid,
            "observedContractWindows":observed,"certifiedSamples":certified,"statuses":counts,
            "eligibleContractWindowsWithElapsedDeadline":eligible,"unusualPremiumAlerts":alerts,
            "comparablePremiumReadings":comparable,
            "alertFrequencyAmongComparablePct":if comparable>0 {Some(100.*alerts as f64/comparable as f64)} else {None},
            "captureLagMs":{"minimum":lags.first(),"maximum":lags.last(),
                "median":if lags.is_empty() {None} else {Some((lags[(lags.len()-1)/2] as f64+lags[lags.len()/2] as f64)/2.)}},
            "certificationCoverageOfObservedPct":if observed>0 {Some(100.*certified as f64/observed as f64)} else {None},
            "pointInTimeEvidenceAudited":valid>0,"executionEnabled":false}));
    }
    Ok(result)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture() -> (FrozenSession, Value, u64) {
        let (mut f, c, e) = observer::tests::premium_fixture();
        f.baseline.cohorts.push(
            serde_json::from_value(serde_json::json!({
                "key":{"underlying":"QQQ","kind":"call","daysToExpiration":0,"minuteOffset":5,
                    "windowMinutes":5,"sessionMinutes":390,"moneynessBand":"[-0.25,0.25)",
                    "measurement":"reported-trades"},
                "dates":["2026-10-09"],"sessions":1,"contractWindows":1,"status":"PENDING_SAMPLES",
                "volumeReferenceReady":false,"premiumReferenceReady":false,
                "volume":{"p25":1,"median":1,"p75":1,"p90":1,"p95":1,"mad":0},
                "premium":{"p25":5000,"median":5000,"p75":5000,"p90":5000,"p95":5000,"mad":0}
            }))
            .unwrap(),
        );
        let mut rows = vec![];
        for kind in ["call", "put"] {
            for strike in [596, 600, 604] {
                let side = if kind == "call" { "C" } else { "P" };
                rows.push(
                    serde_json::json!({"ticker":format!("O:QQQ261012{side}{:08}",strike*1000),
                "underlying_ticker":"QQQ","expiration_date":f.date,"contract_type":kind,
                "shares_per_contract":100,"strike_price":strike}),
                );
            }
        }
        let tickers = crate::option_activity_dataset::select_contracts_breadth(
            &rows, "QQQ", &f.date, 600., 3,
        )
        .unwrap();
        f.selection_evidence =
            serde_json::json!({"complete":true,"catalogRows":rows,"selected":tickers});
        let completed = e.end_ms + 3000;
        let records = tickers
            .iter()
            .map(|ticker| {
                let mut x = e.clone();
                x.report.request.ticker = ticker.clone();
                x.report.contract.ticker = ticker.clone();
                let tail = &ticker[ticker.len() - 15..];
                x.report.contract.kind = if &tail[6..7] == "C" { "call" } else { "put" }.into();
                x.report.contract.strike = tail[7..].parse::<f64>().unwrap() / 1000.;
                observer::premium_observation(&f, &c, &x, completed).unwrap()
            })
            .collect::<Vec<_>>();
        let v = serde_json::json!({"version":"option-premium-prospective-window-v1",
            "date":f.date,"underlying":f.underlying,"startMs":e.start_ms,"endMs":e.end_ms,
            "captureStartedMs":e.end_ms+500,"captureCompletedMs":completed,"frozenMs":f.frozen_ms,
            "referenceBuiltMs":f.baseline.built_ms,"stockEvidence":c,"observations":records,
            "calibrated":false,"executionEnabled":false,"ordersSent":0});
        (f, v, completed)
    }
    #[test]
    fn accepts_certified_data_even_when_detector_is_pending() {
        let (f, v, now) = fixture();
        let (samples, _, alerts) = verify_window(&f, &v, now).unwrap();
        assert_eq!(samples.len(), 6);
        assert_eq!(alerts, 0);
        assert!(samples
            .iter()
            .all(|s| s.premium == Some(5000.) && s.quality_complete));
    }
    #[test]
    fn rejects_altered_premium_or_anomaly_flag() {
        let (f, mut v, now) = fixture();
        v["observations"][0]["sample"]["premium"] = serde_json::json!(99999.);
        assert!(verify_window(&f, &v, now).is_err());
        let (f, mut v, now) = fixture();
        v["observations"][0]["alertEligible"] = serde_json::json!(true);
        assert!(verify_window(&f, &v, now).is_err());
    }
    #[test]
    fn rejects_duplicates_future_clock_and_unselected_contract() {
        let (f, mut v, now) = fixture();
        v["observations"][1] = v["observations"][0].clone();
        assert!(verify_window(&f, &v, now).is_err());
        let (f, mut v, now) = fixture();
        v["captureCompletedMs"] = serde_json::json!(now + 1);
        assert!(verify_window(&f, &v, now).is_err());
        let (f, mut v, now) = fixture();
        v["observations"][0]["ticker"] = serde_json::json!("O:QQQ261012C00900000");
        assert!(verify_window(&f, &v, now).is_err());
    }
    #[test]
    fn waiting_capture_never_invents_a_zero_sample() {
        let dir = std::env::temp_dir().join(format!(
            "missing-premium-capture-{}-{}",
            std::process::id(),
            crate::live::now_ms()
        ));
        let r = consolidate(&[dir], crate::live::now_ms()).unwrap();
        assert!(r.samples.is_empty());
        assert_eq!(r.sessions[0]["status"], "WAITING_CAPTURE");
    }
    #[test]
    fn persisted_json_replays_consistently_with_float_tolerance() {
        let (f, v, now) = fixture();
        let f: FrozenSession = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
        let v: Value = serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(verify_window(&f, &v, now).unwrap().0.len(), 6);
        assert!(!equivalent(
            &serde_json::json!(1234567890123456789u64),
            &serde_json::json!(1234567890123456790u64)
        ));
    }
    #[test]
    fn consolidates_files_without_counting_same_session_twice() {
        let (f, v, now) = fixture();
        let dir = std::env::temp_dir().join(format!(
            "premium-audit-files-{}-{}",
            std::process::id(),
            crate::live::now_ms()
        ));
        fs::create_dir_all(&dir).unwrap();
        let snapshot = dir.join("frozen-session.json");
        let window = dir.join(format!(
            "premium-window-{}.json",
            v["startMs"].as_u64().unwrap()
        ));
        fs::write(&snapshot, serde_json::to_vec(&f).unwrap()).unwrap();
        fs::write(&window, serde_json::to_vec(&v).unwrap()).unwrap();
        let r = consolidate(&[dir.clone()], now + 120_000).unwrap();
        assert_eq!(r.samples.len(), 6);
        assert_eq!(r.sessions[0]["validCapturedWindows"], 1);
        assert_eq!(r.sessions[0]["comparablePremiumReadings"], 0);
        assert!(r.sessions[0]["alertFrequencyAmongComparablePct"].is_null());
        assert_eq!(r.sessions[0]["captureLagMs"]["minimum"], 3000);
        assert!(consolidate(&[dir.clone(), dir.clone()], now + 120_000).is_err());
        fs::remove_file(&window).unwrap();
        fs::remove_file(&snapshot).unwrap();
        fs::remove_dir(&dir).unwrap();
    }
}
