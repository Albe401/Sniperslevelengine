//! Offline certification of prospective eligible-bar volume, never estimated premium.
use crate::{
    option_activity_observer::{self as observer, FrozenSession},
    option_premium_audit::{equivalent, read, Consolidation},
};
use serde_json::Value;
use snipers_core::{option_activity_baseline::Sample, option_activity_detector};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

fn reconstruct(
    f: &FrozenSession,
    context: &Value,
    evidence: &Value,
    start: u64,
    end: u64,
    began: u64,
    clock: u64,
) -> Result<Value, String> {
    let ticker = evidence["request"]["ticker"]
        .as_str()
        .ok_or("Ticker ausente")?;
    let collected = evidence["collectedMs"].as_u64().ok_or("Reloj ausente")?;
    if evidence["version"] != "option-aggregate-volume-evidence-v1"
        || evidence["measurement"] != "eligible-option-bars"
        || evidence["adjusted"] != false
        || evidence["retrievalComplete"] != true
        || evidence["premiumReconstructed"] != false
        || evidence["request"]["underlying"] != f.underlying
        || evidence["request"]["date"] != f.date
        || evidence["request"]["windowMinutes"] != 5
        || evidence["request"]["maxPages"] != 4
        || evidence["session"] != serde_json::to_value(&f.session).unwrap()
        || evidence["contract"]["ticker"] != ticker
        || collected < began
        || collected < end
        || collected > clock
        || !matches!(evidence["pages"].as_u64(), Some(1..=4))
    {
        return Err("Metadatos, paginación o relojes de barras inválidos".into());
    }
    let row = f.selection_evidence["catalogRows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["ticker"] == ticker)
        .ok_or("Contrato fuera de selección")?;
    let metadata = &evidence["contract"];
    if metadata["kind"] != row["contract_type"]
        || metadata["expiration"] != row["expiration_date"]
        || metadata["strike"].as_f64() != row["strike_price"].as_f64()
        || metadata["premiumMultiplier"].as_f64() != Some(100.)
    {
        return Err("Metadatos de contrato no coinciden con catálogo congelado".into());
    }
    let (price, available) = observer::context_price(context, start, &f.date, &f.underlying)?;
    let mut minutes = BTreeMap::new();
    for bar in evidence["bars"].as_array().ok_or("Barras ausentes")? {
        let t = bar["timestampMs"].as_u64().ok_or("Minuto inválido")?;
        let values = ["open", "high", "low", "close", "volume"].map(|k| bar[k].as_f64());
        let [Some(o), Some(h), Some(l), Some(c), Some(v)] = values else {
            return Err("Barra incompleta".into());
        };
        if ![o, h, l, c, v].iter().all(|x| x.is_finite())
            || o <= 0.
            || h <= 0.
            || l <= 0.
            || c <= 0.
            || v < 0.
            || h < o.max(c)
            || l > o.min(c)
            || h < l
            || !t.is_multiple_of(60_000)
            || t < f.session.open_ms
            || t.checked_add(60_000).is_none_or(|close| close > end)
            || minutes.insert(t, v).is_some()
        {
            return Err("Barra duplicada, futura o inválida".into());
        }
    }
    let volumes: Option<Vec<f64>> = (start..end)
        .step_by(60_000)
        .map(|t| minutes.get(&t).copied())
        .collect();
    let sample = volumes.map(|v| Sample {
        underlying: f.underlying.clone(),
        ticker: ticker.into(),
        kind: metadata["kind"].as_str().unwrap_or("").into(),
        expiration: metadata["expiration"].as_str().unwrap_or("").into(),
        strike: metadata["strike"].as_f64().unwrap_or(f64::NAN),
        start_ms: start,
        end_ms: end,
        underlying_price: price,
        price_available_ms: available,
        contracts: v.iter().sum(),
        premium: None,
        measurement: "eligible-option-bars".into(),
        source_collected_ms: collected,
        quality_complete: true,
    });
    if sample.as_ref().is_some_and(|s| !s.contracts.is_finite()) {
        return Err("Suma no finita".into());
    }
    let reading = sample
        .as_ref()
        .map(|s| option_activity_detector::evaluate(&f.baseline, s, clock));
    let timely = clock <= end + 120_000;
    Ok(serde_json::json!({"sample":sample,"reading":reading,
        "status":if !timely {"PENDING_CAPTURE_LATE"} else {reading.as_ref().map(|r|r.status.as_str()).unwrap_or("PENDING_MISSING_MINUTE")},
        "timelyCapture":timely,"alertEligible":timely && reading.as_ref().is_some_and(|r|r.status=="UNUSUAL_VOLUME_EXPERIMENTAL")}))
}

pub fn verify_window(
    f: &FrozenSession,
    value: &Value,
    now: u64,
) -> Result<(Vec<Sample>, BTreeMap<String, usize>, usize), String> {
    let start = value["startMs"].as_u64().ok_or("Inicio ausente")?;
    let end = value["endMs"].as_u64().ok_or("Cierre ausente")?;
    let began = value["captureStartedMs"]
        .as_u64()
        .ok_or("Inicio captura ausente")?;
    let completed = value["captureCompletedMs"]
        .as_u64()
        .ok_or("Fin captura ausente")?;
    if f.premium_mode
        || value["version"] != "option-activity-prospective-window-v1"
        || value["date"] != f.date
        || value["underlying"] != f.underlying
        || value["frozenMs"] != f.frozen_ms
        || value["referenceBuiltMs"] != f.baseline.built_ms
        || value["executionEnabled"] != false
        || value["calibrated"] != false
        || value["ordersSent"] != 0
        || began > completed
        || completed > now
        || observer::closed_window(f, began) != Ok((start, end))
    {
        return Err("Sesión, ventana o relojes incompatibles".into());
    }
    observer::validate_reference(&f.baseline, &f.session, f.frozen_ms)?;
    let selected: BTreeSet<_> = observer::selected_tickers(f)?.into_iter().collect();
    let records = value["observations"]
        .as_array()
        .ok_or("Observaciones ausentes")?;
    if records.len() != selected.len() {
        return Err("Faltan contratos".into());
    }
    let mut seen = BTreeSet::new();
    let mut samples = vec![];
    let mut counts = BTreeMap::new();
    let mut alerts = 0;
    for record in records {
        let ticker = record["ticker"].as_str().ok_or("Ticker ausente")?;
        let clock = record["completedMs"]
            .as_u64()
            .ok_or("Reloj contrato ausente")?;
        if !selected.contains(ticker) || !seen.insert(ticker) || clock < began || clock > completed
        {
            return Err("Contrato duplicado, ajeno o reloj inválido".into());
        }
        let status = record["status"].as_str().ok_or("Estado ausente")?;
        if record["evidence"].is_null() {
            if !["PENDING_CONTEXT", "PENDING_SOURCE"].contains(&status)
                || record["alertEligible"] != false
                || !record["sample"].is_null()
                || !record["reading"].is_null()
            {
                return Err("Pendiente con muestra o alerta".into());
            }
            if status == "PENDING_CONTEXT"
                && observer::context_price(&value["stockEvidence"], start, &f.date, &f.underlying)
                    .is_ok()
            {
                return Err("Contexto pendiente contradice evidencia".into());
            }
        } else {
            if record["evidence"]["request"]["ticker"] != ticker {
                return Err("Evidencia de otro contrato".into());
            }
            let reconstructed = reconstruct(
                f,
                &value["stockEvidence"],
                &record["evidence"],
                start,
                end,
                began,
                clock,
            )?;
            for field in [
                "sample",
                "reading",
                "status",
                "timelyCapture",
                "alertEligible",
            ] {
                if !equivalent(&reconstructed[field], &record[field]) {
                    return Err(format!("Registro alterado: {field}"));
                }
            }
            if reconstructed["timelyCapture"] == true && !reconstructed["sample"].is_null() {
                samples.push(
                    serde_json::from_value(reconstructed["sample"].clone())
                        .map_err(|_| "Muestra inválida")?,
                );
            }
            if record["alertEligible"] == true {
                alerts += 1;
            }
        }
        *counts.entry(status.to_string()).or_default() += 1;
    }
    Ok((samples, counts, alerts))
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
        if frozen.premium_mode
            || frozen.frozen_ms > now
            || !sessions_seen.insert((frozen.underlying.clone(), frozen.date.clone()))
        {
            return Err("Sesión futura, de primas o duplicada".into());
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
                    .is_some_and(|s| s.starts_with("window-") && s.ends_with(".json"))
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
                let expected = format!("window-{start}.json");
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
            .get("UNUSUAL_VOLUME_EXPERIMENTAL")
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
            "eligibleContractWindowsWithElapsedDeadline":eligible,"unusualVolumeAlerts":alerts,
            "comparableVolumeReadings":comparable,
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
        let (mut f, mut value, now) = crate::option_premium_audit::tests::fixture();
        f.premium_mode = false;
        value["version"] = serde_json::json!("option-activity-prospective-window-v1");
        let start = value["startMs"].as_u64().unwrap();
        let end = value["endMs"].as_u64().unwrap();
        let began = value["captureStartedMs"].as_u64().unwrap();
        let records=value["observations"].as_array().unwrap().iter().map(|old| {
            let ticker=old["ticker"].as_str().unwrap();
            let mut evidence=serde_json::json!({"version":"option-aggregate-volume-evidence-v1",
                "request":{"underlying":f.underlying,"ticker":ticker,"date":f.date,"windowMinutes":5,"maxPages":4},
                "contract":old["evidence"]["report"]["contract"],"session":f.session,"collectedMs":end+2000,
                "measurement":"eligible-option-bars","adjusted":false,"pages":1,"retrievalComplete":true,
                "premiumReconstructed":false});
            evidence["bars"]=serde_json::json!((start..end).step_by(60_000).map(|t|serde_json::json!({
                "timestampMs":t,"open":1.,"high":2.,"low":0.5,"close":1.,"volume":10.,"vwap":null})).collect::<Vec<_>>());
            let mut record=reconstruct(&f,&value["stockEvidence"],&evidence,start,end,began,now).unwrap();
            record["ticker"]=serde_json::json!(ticker);record["completedMs"]=serde_json::json!(now);record["evidence"]=evidence;
            record
        }).collect::<Vec<_>>();
        value["observations"] = serde_json::json!(records);
        (f, value, now)
    }
    #[test]
    fn certified_volume_is_accepted_with_pending_reference() {
        let (f, v, now) = fixture();
        let (s, _, a) = verify_window(&f, &v, now).unwrap();
        assert_eq!(s.len(), 6);
        assert_eq!(a, 0);
        assert!(s.iter().all(|s| s.contracts == 50. && s.premium.is_none()));
        let replay: Value = serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(verify_window(&f, &replay, now).unwrap().0.len(), 6);
    }
    #[test]
    fn tampering_and_duplicate_minutes_are_rejected() {
        for field in [
            "amount",
            "flag",
            "duplicate",
            "future",
            "metadata",
            "clock",
            "context",
        ] {
            let (f, mut v, now) = fixture();
            match field {
                "amount" => v["observations"][0]["sample"]["contracts"] = serde_json::json!(99.),
                "flag" => v["observations"][0]["alertEligible"] = serde_json::json!(true),
                "duplicate" => {
                    v["observations"][0]["evidence"]["bars"][1] =
                        v["observations"][0]["evidence"]["bars"][0].clone()
                }
                "future" => {
                    v["observations"][0]["evidence"]["bars"][0]["timestampMs"] = v["endMs"].clone()
                }
                "metadata" => {
                    v["observations"][0]["evidence"]["contract"]["strike"] = serde_json::json!(999.)
                }
                "clock" => {
                    v["observations"][0]["evidence"]["collectedMs"] = serde_json::json!(now + 1)
                }
                _ => v["stockEvidence"]["knownMs"] = v["startMs"].clone(),
            }
            assert!(verify_window(&f, &v, now).is_err(), "{field}");
        }
    }
    #[test]
    fn missing_minute_remains_pending_not_zero() {
        let (f, mut v, now) = fixture();
        let start = v["startMs"].as_u64().unwrap();
        let end = v["endMs"].as_u64().unwrap();
        let began = v["captureStartedMs"].as_u64().unwrap();
        v["observations"][0]["evidence"]["bars"]
            .as_array_mut()
            .unwrap()
            .pop();
        let r = reconstruct(
            &f,
            &v["stockEvidence"],
            &v["observations"][0]["evidence"],
            start,
            end,
            began,
            now,
        )
        .unwrap();
        for field in [
            "sample",
            "reading",
            "status",
            "timelyCapture",
            "alertEligible",
        ] {
            v["observations"][0][field] = r[field].clone();
        }
        assert_eq!(verify_window(&f, &v, now).unwrap().0.len(), 5);
        assert_eq!(r["status"], "PENDING_MISSING_MINUTE");
        assert!(r["sample"].is_null());
    }
    #[test]
    fn late_capture_not_consolidated_and_foreign_mode_rejected() {
        let (mut f, mut v, now) = fixture();
        f.premium_mode = true;
        assert!(verify_window(&f, &v, now).is_err());
        f.premium_mode = false;
        let end = v["endMs"].as_u64().unwrap();
        let late = end + 120_001;
        v["captureCompletedMs"] = serde_json::json!(late);
        for r in v["observations"].as_array_mut().unwrap() {
            r["completedMs"] = serde_json::json!(late);
            r["status"] = serde_json::json!("PENDING_CAPTURE_LATE");
            r["timelyCapture"] = serde_json::json!(false);
        }
        assert!(verify_window(&f, &v, late).unwrap().0.is_empty());
        assert!(verify_window(&f, &v, now).is_err());
    }
    #[test]
    fn consolidates_real_files_and_rejects_repeated_session() {
        let (f, v, now) = fixture();
        let dir = std::env::temp_dir().join(format!(
            "volume-audit-{}-{}",
            std::process::id(),
            crate::live::now_ms()
        ));
        fs::create_dir(&dir).unwrap();
        fs::write(
            dir.join("frozen-session.json"),
            serde_json::to_vec(&f).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.join(format!("window-{}.json", v["startMs"].as_u64().unwrap())),
            serde_json::to_vec(&v).unwrap(),
        )
        .unwrap();
        assert_eq!(consolidate(&[dir.clone()], now).unwrap().samples.len(), 6);
        assert!(consolidate(&[dir.clone(), dir.clone()], now).is_err());
        fs::remove_dir_all(&dir).unwrap();
        assert!(consolidate(&[dir], now).unwrap().samples.is_empty());
    }
}
