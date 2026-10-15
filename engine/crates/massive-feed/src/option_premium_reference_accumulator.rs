//! Incremental source registry with full offline recertification. No REST or live formula writes.
use crate::{
    live::Bar,
    option_activity_dataset::{self, Dataset},
    option_activity_history::Report,
    option_premium_audit, option_premium_dataset,
};
use serde::Deserialize;
use serde_json::{json, Value};
use snipers_core::{
    calendar::{self, Day, Session},
    option_activity_baseline::{self, Sample},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub cutoff_date: String,
    pub dataset_files: Vec<PathBuf>,
    pub prospective_directories: Vec<PathBuf>,
    pub previous_snapshot: Option<PathBuf>,
}
fn read<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T, String> {
    if std::fs::metadata(p).map_err(|_| "Archivo ausente")?.len() > 64_000_000 {
        return Err("Archivo supera 64 MB".into());
    }
    serde_json::from_slice(&std::fs::read(p).map_err(|_| "Archivo ilegible")?)
        .map_err(|_| "JSON incompatible".into())
}
fn session(date: &str) -> Result<Session, String> {
    match calendar::day_bounds(date)
        .and_then(|(t, _)| calendar::date_at(t))
        .map(calendar::schedule)
    {
        Some(Day::Open(s)) => Ok(s),
        _ => Err("Fecha sin sesión abierta conocida".into()),
    }
}
fn historical(path: &Path, now: u64) -> Result<(Vec<Sample>, Value), String> {
    let d: Dataset = read(path)?;
    let s = session(&d.date)?;
    if d.version != "option-activity-dataset-v1"
        || d.underlying != "QQQ"
        || d.collected_ms < s.close_ms
        || d.collected_ms > now
        || d.reports.len() != 6
        || d.context_reports.len() != 2
    {
        return Err("Dataset histórico incompatible".into());
    }
    let stock: Value = read(Path::new(&d.context_reports[0]))?;
    let collected = stock["collectedMs"]
        .as_u64()
        .filter(|t| *t >= s.close_ms && *t <= d.collected_ms)
        .ok_or("Reloj de contexto inválido")?;
    if stock["version"] != "stock-context-evidence-v1"
        || stock["underlying"] != "QQQ"
        || stock["date"] != d.date
        || stock["adjusted"] != false
    {
        return Err("Contexto de otra sesión o ajustado".into());
    }
    let mut bars = vec![];
    let mut times = BTreeSet::new();
    for b in stock["bars"]
        .as_array()
        .filter(|a| a.len() <= 390)
        .ok_or("Barras de contexto inválidas")?
    {
        let ts = b["timestampMs"]
            .as_u64()
            .filter(|t| {
                *t >= s.open_ms && *t < s.close_ms && (*t - s.open_ms).is_multiple_of(60000)
            })
            .ok_or("Minuto fuera de sesión")?;
        let number = |key: &str| {
            b[key]
                .as_f64()
                .filter(|v| v.is_finite())
                .ok_or("Número de contexto inválido")
        };
        let (o, h, l, c, v) = (
            number("open")?,
            number("high")?,
            number("low")?,
            number("close")?,
            number("volume")?,
        );
        if !times.insert(ts)
            || ts + 60000 > collected
            || l <= 0.
            || o < l
            || c < l
            || h < o
            || h < c
            || h < l
            || v < 0.
        {
            return Err("OHLC duplicado o inválido".into());
        }
        bars.push(Bar {
            timestamp_ms: ts,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: v,
            vwap: None,
        });
    }
    let opening = bars
        .iter()
        .find(|b| b.timestamp_ms == s.open_ms)
        .ok_or("Primer minuto ausente")?;
    if d.opening_reference_price != Some(opening.close)
        || d.opening_reference_available_ms != Some(s.open_ms + 60000)
        || d.stock_minutes != bars.len()
    {
        return Err("Selección no coincide con primer minuto cerrado".into());
    }
    let selection: Value = read(Path::new(&d.context_reports[1]))?;
    if selection["version"] != "option-selection-evidence-v1"
        || selection["underlying"] != "QQQ"
        || selection["date"] != d.date
        || selection["complete"] != true
        || selection["perSide"] != 3
        || selection["openingReferencePrice"].as_f64() != Some(opening.close)
    {
        return Err("Selección histórica incompatible".into());
    }
    let expected = option_activity_dataset::select_contracts_breadth(
        selection["catalogRows"]
            .as_array()
            .ok_or("Catálogo ausente")?,
        "QQQ",
        &d.date,
        opening.close,
        3,
    )?;
    if selection["selected"] != json!(expected) {
        return Err("Selección de contratos alterada".into());
    }
    let mut seen = BTreeSet::new();
    let mut samples = vec![];
    let mut reports = vec![];
    for p in &d.reports {
        let r: Report = read(Path::new(p))?;
        if r.request.underlying != "QQQ"
            || r.request.date != d.date
            || !expected.contains(&r.request.ticker)
            || !seen.insert(r.request.ticker.clone())
            || r.collected_ms > d.collected_ms
        {
            return Err("Reporte ajeno, duplicado o futuro".into());
        }
        let (quality, accepted, skipped) =
            option_premium_dataset::samples(&r, &bars, s.open_ms + 60000);
        reports.push(json!({"ticker":r.request.ticker,"sourceFile":p,"status":quality.status,"reasons":quality.reasons,"acceptedSamples":accepted.len(),"skipped":skipped}));
        samples.extend(accepted);
    }
    Ok((
        samples,
        json!({"date":d.date,"sourceType":"historical-reconstruction","reports":reports,"pointInTimeValidated":false}),
    ))
}
fn registry(config: &Config, previous: &Value) -> Result<Vec<(String, PathBuf)>, String> {
    let mut paths = BTreeMap::new();
    let mut add = |kind: &str, p: &Path| -> Result<(), String> {
        // Require explicit absolute paths in the manifest; never resolve inherited
        // relative paths against a different invocation directory.
        if !p.is_absolute() {
            return Err("Las fuentes requieren rutas absolutas".into());
        }
        let key = p.to_string_lossy().into_owned();
        if paths.get(&key).is_some_and(|old| old != kind) {
            return Err("Fuente usada en dos modalidades".into());
        }
        paths.insert(key, kind.to_owned());
        Ok(())
    };
    if !previous.is_null() {
        for r in previous["sourceRecords"]
            .as_array()
            .ok_or("Snapshot sin registro de fuentes")?
        {
            let kind = r["kind"]
                .as_str()
                .filter(|k| matches!(*k, "historical" | "prospective"))
                .ok_or("Modalidad incompatible")?;
            add(
                kind,
                Path::new(r["path"].as_str().ok_or("Fuente sin ruta")?),
            )?;
        }
    }
    for p in &config.dataset_files {
        add("historical", p)?;
    }
    for p in &config.prospective_directories {
        add("prospective", p)?;
    }
    if paths.len() > 252 {
        return Err("Máximo 252 fuentes registradas".into());
    }
    Ok(paths
        .into_iter()
        .map(|(p, k)| (k, PathBuf::from(p)))
        .collect())
}
fn preserved(old: &Value, new: &Value) -> Result<(), String> {
    let current = new.as_array().ok_or("Fuente sin muestras recertificadas")?;
    for sample in old.as_array().ok_or("Snapshot sin muestras originales")? {
        if !current.contains(sample) {
            return Err("Una observación previamente certificada cambió o desapareció".into());
        }
    }
    Ok(())
}
pub fn build(config: &Config, now: u64) -> Result<Value, String> {
    let target = session(&config.cutoff_date)?;
    if now == 0 || now >= target.open_ms {
        return Err("El corte debe ser una apertura futura".into());
    }
    let previous: Value = match &config.previous_snapshot {
        Some(p) => read(p)?,
        None => Value::Null,
    };
    if !previous.is_null()
        && (previous["version"] != "option-premium-reference-accumulator-v1"
            || previous["executionEnabled"] != false
            || previous["formulaChanged"] != false
            || previous["generatedMs"].as_u64().is_none_or(|t| t > now)
            || previous["sourceErrors"]
                .as_array()
                .is_none_or(|a| !a.is_empty())
            || previous["baseline"].is_null())
    {
        return Err("Snapshot anterior incompatible o pendiente".into());
    }
    let sources = registry(config, &previous)?;
    let mut all = vec![];
    let mut records = vec![];
    let mut errors = vec![];
    for (kind, path) in sources {
        let checked = if kind == "historical" {
            historical(&path, now)
        } else {
            option_premium_audit::consolidate(&[path.clone()], now).and_then(|r| {
                if !r.errors.is_empty() {
                    return Err("Capturas prospectivas con errores".into());
                }
                Ok((
                    r.samples,
                    json!({"sourceType":"prospective-capture","sessions":r.sessions}),
                ))
            })
        };
        match checked {
            Ok((samples, detail)) => {
                let value = json!(samples);
                if let Some(old) = previous["sourceRecords"]
                    .as_array()
                    .and_then(|a| a.iter().find(|r| r["path"] == json!(path)))
                {
                    if let Err(e) = preserved(&old["certifiedSamples"], &value) {
                        errors.push(json!({"path":path,"reason":e}));
                    }
                }
                records.push(
                    json!({"kind":kind,"path":path,"detail":detail,"certifiedSamples":value}),
                );
                all.extend(samples);
            }
            Err(e) => errors.push(json!({"path":path,"reason":e})),
        }
    }
    let baseline =
        option_activity_baseline::build(&all, target.open_ms, 20, now).map_err(str::to_owned)?;
    if baseline.conflicting_samples > 0 {
        errors.push(json!({"reason":"Conflicting certified observations across sources","count":baseline.conflicting_samples}));
    }
    if !baseline.excluded.is_empty() {
        errors.push(
            json!({"reason":"Samples excluded by reference guards","excluded":baseline.excluded}),
        );
    }
    let progress:Vec<_>=baseline.cohorts.iter().map(|c|json!({"key":c.key,"certifiedDates":c.dates,"distinctSessions":c.sessions,
        "remainingSessions":20usize.saturating_sub(c.sessions),"ready":c.premium_reference_ready,"status":c.status,"medianGrossPremium":c.premium.as_ref().map(|p|p.median)})).collect();
    let ready = baseline
        .cohorts
        .iter()
        .filter(|c| c.premium_reference_ready)
        .count();
    let dates: BTreeSet<_> = all
        .iter()
        .filter_map(|s| calendar::date_at(s.start_ms))
        .map(|d| d.to_string())
        .collect();
    let valid = errors.is_empty();
    Ok(
        json!({"version":"option-premium-reference-accumulator-v1","generatedMs":now,"cutoffDate":config.cutoff_date,
        "previousSnapshot":config.previous_snapshot,"status":if !valid{"PENDING_SOURCE_ERRORS"}else if ready>0{"EXPERIMENTAL_REFERENCE_WITH_READY_COHORTS"}else{"ACCUMULATING_CERTIFIED_DATES"},
        "sourceRecords":records,"sourceErrors":errors,"baseline":if valid{json!(baseline)}else{Value::Null},"cohortProgress":progress,
        "summary":{"registeredSources":records.len(),"certifiedDates":dates,"distinctCertifiedSessions":dates.len(),"acceptedWindowSamples":baseline.accepted_samples,
            "duplicateWindowsIgnored":baseline.duplicates,"readyPremiumCohorts":if valid{Some(ready)}else{None},"cohorts":progress.len(),"minimumDistinctDatesPerCohort":20},
        "limits":["Original local files are re-audited on every update; no provider redownload or trusted sample-only cache.",
            "Previously certified observations must remain present and identical; new windows may be appended.",
            "Referenced source files must be retained; snapshots are not a self-contained backup or cryptographic signature.",
            "Legacy relative context/report paths require the original collection working directory.",
            "Historical reference dates are not prospective trial training/evaluation sessions.","Ready cohorts do not imply full selected-contract coverage or predictive calibration."],
        "networkRequests":0,"credentialsLoaded":false,"scheduled":false,"calibrated":false,"formulaChanged":false,"executionEnabled":false,"ordersSent":0}),
    )
}
/// Incorporate a real capture directory, retaining the previous source registry.
/// Pending/invalid captures never publish a new usable reference.
pub fn ingest_capture(
    previous: &Path,
    directory: &Path,
    cutoff_date: &str,
    now: u64,
) -> Result<Value, String> {
    if !previous.is_absolute() || !directory.is_absolute() {
        return Err("Snapshot y sesión requieren rutas absolutas".into());
    }
    let audited = option_premium_audit::consolidate(&[directory.to_path_buf()], now)?;
    if !audited.errors.is_empty() || audited.samples.is_empty() {
        return Err("Sesión pendiente: no hay ventanas certificadas incorporables".into());
    }
    let report = build(
        &Config {
            cutoff_date: cutoff_date.into(),
            dataset_files: vec![],
            prospective_directories: vec![directory.to_path_buf()],
            previous_snapshot: Some(previous.to_path_buf()),
        },
        now,
    )?;
    if report["baseline"].is_null() {
        return Err(format!("Referencia bloqueada: {}", report["sourceErrors"]));
    }
    Ok(report)
}

pub fn from_file(path: &Path, now: u64) -> Result<Value, String> {
    build(&read::<Config>(path)?, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn previous_observations_must_survive_unchanged() {
        let old = json!([{"ticker":"A","premium":100}]);
        assert!(preserved(
            &old,
            &json!([{"ticker":"A","premium":100},{"ticker":"B","premium":200}])
        )
        .is_ok());
        assert!(preserved(&old, &json!([{"ticker":"A","premium":101}])).is_err());
        assert!(preserved(&old, &json!([])).is_err());
    }
    #[test]
    fn empty_input_does_not_invent_dates_or_ready_cohorts() {
        let now = calendar::legacy_eastern_ms("2026-10-10 12:00:00").unwrap();
        let c = Config {
            cutoff_date: "2026-10-12".into(),
            dataset_files: vec![],
            prospective_directories: vec![],
            previous_snapshot: None,
        };
        let r = build(&c, now).unwrap();
        assert_eq!(r["summary"]["distinctCertifiedSessions"], 0);
        assert_eq!(r["summary"]["readyPremiumCohorts"], 0);
        assert!(build(&c, session("2026-10-12").unwrap().open_ms).is_err());
    }
    #[test]
    fn repeated_source_paths_do_not_add_sessions_and_relative_paths_are_rejected() {
        let root = std::env::current_dir()
            .unwrap()
            .join("target/accumulator-source-fixture.json");
        let c = Config {
            cutoff_date: "2026-10-12".into(),
            dataset_files: vec![root.clone(), root.clone()],
            prospective_directories: vec![],
            previous_snapshot: None,
        };
        assert_eq!(registry(&c, &Value::Null).unwrap().len(), 1);
        let mut bad = c;
        bad.dataset_files = vec![PathBuf::from("relative.json")];
        assert!(registry(&bad, &Value::Null).is_err());
    }
    #[test]
    fn prospective_resume_recertifies_and_blocks_disappearing_windows() {
        let root = std::env::current_dir().unwrap().join(format!(
            "target/accumulator-prospective-{}-{}",
            std::process::id(),
            crate::live::now_ms()
        ));
        let directory = crate::option_joint_audit::tests::write_fixture(&root, true);
        let (_, window, now) = crate::option_premium_audit::tests::fixture();
        let mut c = Config {
            cutoff_date: "2026-10-13".into(),
            dataset_files: vec![],
            prospective_directories: vec![directory.clone()],
            previous_snapshot: None,
        };
        let first = build(&c, now).unwrap();
        assert_eq!(first["summary"]["acceptedWindowSamples"], 6);
        assert_eq!(first["summary"]["distinctCertifiedSessions"], 1);
        assert_eq!(first["summary"]["readyPremiumCohorts"], 0);
        let previous = root.join("previous.json");
        std::fs::write(&previous, first.to_string()).unwrap();
        c.previous_snapshot = Some(previous.clone());
        let second = ingest_capture(&previous, &directory, "2026-10-13", now + 1).unwrap();
        assert_eq!(first["summary"], second["summary"]);
        let source = directory.join(format!(
            "premium-window-{}.json",
            window["startMs"].as_u64().unwrap()
        ));
        std::fs::remove_file(&source).unwrap();
        let blocked = build(&c, now + 2).unwrap();
        assert_eq!(blocked["status"], "PENDING_SOURCE_ERRORS");
        assert!(blocked["baseline"].is_null());
        assert!(ingest_capture(&previous, &directory, "2026-10-13", now + 2).is_err());
        std::fs::remove_file(previous).unwrap();
        std::fs::remove_file(directory.join("frozen-session.json")).unwrap();
        std::fs::remove_dir(directory).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn capture_bridge_rejects_absent_sources_and_relative_paths() {
        let root = std::env::current_dir()
            .unwrap()
            .join("target/absent-premium-bridge-source");
        assert!(ingest_capture(
            &root.join("previous.json"),
            &root,
            "2026-10-13",
            1791816000000
        )
        .is_err());
        assert!(ingest_capture(
            Path::new("previous.json"),
            &root,
            "2026-10-13",
            1791816000000
        )
        .is_err());
    }
}
