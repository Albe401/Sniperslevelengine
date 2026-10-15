//! Read-only session preparation checks. No credentials, network, scheduling or capture.
use crate::{
    option_activity_observer as observer, option_formula_baseline, option_premium_audit,
    option_volume_audit,
};
use serde::Deserialize;
use serde_json::{json, Value};
use snipers_core::{
    calendar::{self, Day, Session},
    option_activity_baseline::Baseline,
};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub target_date: String,
    pub underlying: String,
    pub volume_reference: PathBuf,
    pub premium_reference: PathBuf,
    pub volume_directory: PathBuf,
    pub premium_directory: PathBuf,
    pub baseline_journal: Option<PathBuf>,
}
fn read<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T, String> {
    if std::fs::metadata(p).map_err(|_| "Archivo ausente")?.len() > 64_000_000 {
        return Err("Archivo demasiado grande".into());
    }
    serde_json::from_slice(&std::fs::read(p).map_err(|_| "Archivo ilegible")?)
        .map_err(|_| "JSON incompatible".into())
}
fn reference(b: &Baseline, s: &Session, now: u64, premium: bool, symbol: &str) -> Value {
    let validation = observer::validate_reference(b, s, now);
    let measurement = if premium {
        "reported-trades"
    } else {
        "eligible-option-bars"
    };
    let cohorts: Vec<_> = b
        .cohorts
        .iter()
        .filter(|c| {
            c.key.underlying == symbol
                && c.key.measurement == measurement
                && (!premium || c.premium.is_some())
        })
        .collect();
    let ready = cohorts
        .iter()
        .filter(|c| {
            if premium {
                c.premium_reference_ready
            } else {
                c.volume_reference_ready
            }
        })
        .count();
    let dates = cohorts
        .iter()
        .flat_map(|c| c.dates.iter())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let valid = validation.is_ok() && !cohorts.is_empty();
    json!({"status":if !valid {"INVALID_REFERENCE"}else if ready==0 {"VALID_FOR_CAPTURE_PENDING_READING"}else{"VALID_REFERENCE_WITH_READY_COHORTS"},
        "validForPreparation":valid,"readingReadyCohorts":ready,"matchingCohorts":cohorts.len(),"distinctHistoricalDates":dates,
        "minimumSessionsPerCohort":b.minimum_sessions,"builtMs":b.built_ms,"cutoffMs":b.cutoff_ms,
        "measurement":measurement,"reason":validation.err(),"fullSelectedCoverageKnown":false})
}
fn reference_file(p: &Path, s: &Session, now: u64, premium: bool, symbol: &str) -> Value {
    match read::<Baseline>(p) {
        Ok(b) => reference(&b, s, now, premium, symbol),
        Err(e) => {
            json!({"status":"MISSING_OR_INVALID_REFERENCE","validForPreparation":false,"reason":e})
        }
    }
}
fn frozen(directory: &Path, s: &Session, now: u64, premium: bool, symbol: &str) -> Value {
    if !directory.join("frozen-session.json").exists() {
        return json!({"status":"WAITING_SESSION_FREEZE","valid":false});
    }
    let check = (|| -> Result<Value, String> {
        let f: observer::FrozenSession = read(&directory.join("frozen-session.json"))?;
        if f.date != s.date
            || f.underlying != symbol
            || f.premium_mode != premium
            || f.execution_enabled
            || f.frozen_ms > now
            || f.version != "option-activity-frozen-session-v1"
            || f.frozen_ms < s.open_ms + 60_000
            || f.frozen_ms >= s.close_ms
            || f.eligible_from_ms != s.open_ms + ((f.frozen_ms - s.open_ms) / 300_000 + 1) * 300_000
            || f.eligible_from_ms + 300_000 > s.close_ms
        {
            return Err("Sesión congelada incompatible con objetivo".into());
        }
        observer::validate_reference(&f.baseline, s, f.frozen_ms)?;
        let selected = observer::selected_tickers(&f)?;
        let errors = if premium {
            option_premium_audit::consolidate(&[directory.to_owned()], now)?.errors
        } else {
            option_volume_audit::consolidate(&[directory.to_owned()], now)?.errors
        };
        if !errors.is_empty() {
            return Err("Capturas archivadas con errores de auditoría".into());
        }
        Ok(
            json!({"status":"FROZEN_SESSION_CHECKED","valid":true,"eligibleFromMs":f.eligible_from_ms,"frozenMs":f.frozen_ms,"selectedTickers":selected}),
        )
    })();
    check.unwrap_or_else(|e| json!({"status":"INVALID_FROZEN_SESSION","valid":false,"reason":e}))
}
pub fn build(config: &Config, now: u64) -> Result<Value, String> {
    if config.underlying != "QQQ" {
        return Err("Ensayo registrado solo para QQQ".into());
    }
    let s = match calendar::day_bounds(&config.target_date)
        .and_then(|(t, _)| calendar::date_at(t))
        .map(calendar::schedule)
    {
        Some(Day::Open(s)) => s,
        _ => return Err("Fecha sin sesión conocida abierta".into()),
    };
    if config.volume_directory == config.premium_directory {
        return Err("Destinos de volumen/primas deben ser distintos".into());
    }
    let volume = reference_file(&config.volume_reference, &s, now, false, &config.underlying);
    let premium = reference_file(&config.premium_reference, &s, now, true, &config.underlying);
    let vf = frozen(&config.volume_directory, &s, now, false, &config.underlying);
    let pf = frozen(&config.premium_directory, &s, now, true, &config.underlying);
    let journal=match &config.baseline_journal {
        None=>json!({"status":"NO_EXPORTED_JOURNAL","targetSnapshots":0,"valid":false}),
        Some(p)=>match read::<Value>(p).and_then(|v| {
            option_formula_baseline::attach(&mut json!({"cases":[]}),&v,now)?;
            let rows=v["formulaBaseline"]["rows"].as_array().unwrap();
            let count=rows.iter().filter(|r|r["symbol"]==config.underlying && r["session"]==s.date).count();
            Ok(json!({"status":if count>0{"ORIGINAL_TARGET_SNAPSHOTS_PRESENT"}else{"WAITING_TARGET_SNAPSHOTS"},"valid":true,"targetSnapshots":count,
                "dropped":v["formulaBaseline"]["dropped"],"liveDeploymentVerified":false,"perUnitFreshMatchingStillRequired":true}))
        }) {Ok(v)=>v,Err(e)=>json!({"status":"INVALID_JOURNAL","valid":false,"reason":e})}
    };
    let phase = if now < s.open_ms {
        "BEFORE_SESSION"
    } else if now < s.close_ms {
        "SESSION_OPEN"
    } else {
        "SESSION_CLOSED"
    };
    let mut blockers = vec![];
    if volume["validForPreparation"] != true {
        blockers.push("VOLUME_REFERENCE_REQUIRED");
    }
    if premium["validForPreparation"] != true {
        blockers.push("PREMIUM_REFERENCE_REQUIRED");
    }
    if premium["readingReadyCohorts"] == 0 {
        blockers.push("PREMIUM_COMPARABLE_HISTORY_PENDING");
    }
    if vf["valid"] != true {
        blockers.push("VOLUME_SESSION_NOT_FROZEN");
    }
    if pf["valid"] != true {
        blockers.push("PREMIUM_SESSION_NOT_FROZEN");
    }
    if journal["valid"] != true || journal["targetSnapshots"] == 0 {
        blockers.push("ORIGINAL_TARGET_CONFLUENCE_PENDING");
    }
    let same_selection = vf["valid"] == true
        && pf["valid"] == true
        && vf["selectedTickers"] == pf["selectedTickers"];
    if vf["valid"] == true && pf["valid"] == true && !same_selection {
        blockers.push("SELECTIONS_DIFFER");
    }
    let command = |mode: &str, reference: &Path, directory: &Path| {
        json!([
            "cargo",
            "run",
            "--offline",
            "-p",
            "massive-feed",
            "--example",
            "observe_option_activity",
            "--",
            mode,
            reference,
            config.underlying,
            directory
        ])
    };
    let operation = |mode: &str, directory: &Path| {
        json!([
            "cargo",
            "run",
            "--offline",
            "-p",
            "massive-feed",
            "--example",
            "observe_option_activity",
            "--",
            mode,
            directory
        ])
    };
    Ok(
        json!({"version":"option-capture-preflight-v1","generatedMs":now,"targetDate":s.date,"session":s,"phase":phase,
        "status":if phase=="SESSION_CLOSED"{"SESSION_EXPIRED"}else if volume["validForPreparation"]==true && premium["validForPreparation"]==true{"REFERENCES_READY_FOR_PREPARATION"}else{"PENDING_REFERENCES"},
        "volumeReference":volume,"premiumReference":premium,"volumeObserver":vf,"premiumObserver":pf,"baselineJournal":journal,
        "sameFrozenSelection":same_selection,"sharedUnitReadinessGuaranteed":false,"blockers":blockers,
        "commandsNotExecuted":{"volumePrepare":command("prepare",&config.volume_reference,&config.volume_directory),"premiumPrepare":command("prepare-premium",&config.premium_reference,&config.premium_directory),
            "volumeArm":operation("arm",&config.volume_directory),"premiumArm":operation("arm",&config.premium_directory),
            "volumeCapture":operation("capture",&config.volume_directory),"premiumCapture":operation("capture-premium",&config.premium_directory)},
        "captureProtocol":{"mode":"one-shot","scheduledCaptureVerified":false,"prepareEarliestMs":s.open_ms+60_000,"windowMinutes":5,"armDuringLastMinuteBeforeStart":true,"priorContextRequiredForBothObservers":true,"maxCaptureLagMs":120000,"baselineMaxAgeMs":3000},
        "limits":["Preparation requires the first closed stock minute; absent freezes before open are expected.","Ready historical cohorts do not guarantee coverage of the six contracts selected at opening.","Valid premium history can capture certified samples while anomaly readings remain pending.","Original confluence requires the updated panel open and actual exported snapshots; source code alone does not prove deployment.","Credential access and provider connectivity are untested; no network request or capture was made."],
        "credentialsLoaded":false,"networkRequests":0,"captureStarted":false,"formulaChanged":false,"executionEnabled":false,"ordersSent":0}),
    )
}
pub fn from_file(path: &Path, now: u64) -> Result<Value, String> {
    build(&read::<Config>(path)?, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> (Baseline, Session, u64) {
        let (mut f, _, n) = crate::option_volume_audit::tests::fixture();
        for c in &mut f.baseline.cohorts {
            c.key.measurement = "eligible-option-bars".into();
            c.premium = None;
        }
        (f.baseline, f.session, n)
    }
    #[test]
    fn missing_files_are_pending_without_creating_capture_directories() {
        let missing = PathBuf::from("target/preflight-test-absent-input-do-not-create");
        let c = Config {
            target_date: "2026-10-12".into(),
            underlying: "QQQ".into(),
            volume_reference: missing.join("volume.json"),
            premium_reference: missing.join("premium.json"),
            volume_directory: missing.join("volume"),
            premium_directory: missing.join("premium"),
            baseline_journal: None,
        };
        let now = calendar::legacy_eastern_ms("2026-10-10 12:00:00").unwrap();
        let r = build(&c, now).unwrap();
        assert_eq!(r["phase"], "BEFORE_SESSION");
        assert_eq!(r["status"], "PENDING_REFERENCES");
        assert_eq!(r["networkRequests"], 0);
        assert!(!missing.exists());
        let mut bad = c;
        bad.target_date = "2026-10-10".into();
        assert!(build(&bad, now).is_err());
    }
    #[test]
    fn references_require_exact_session_cutoff_and_prior_clock() {
        let (mut b, s, n) = sample();
        assert_eq!(
            reference(&b, &s, n, false, "QQQ")["validForPreparation"],
            true
        );
        b.cutoff_ms += 1;
        assert_eq!(
            reference(&b, &s, n, false, "QQQ")["validForPreparation"],
            false
        );
        b.cutoff_ms = s.open_ms;
        b.built_ms = n + 1;
        assert_eq!(
            reference(&b, &s, n, false, "QQQ")["validForPreparation"],
            false
        );
    }
    #[test]
    fn volume_history_cannot_substitute_for_premium_history() {
        let (b, s, n) = sample();
        assert_eq!(
            reference(&b, &s, n, true, "QQQ")["validForPreparation"],
            false
        );
    }
    #[test]
    fn valid_pending_premium_reference_can_prepare_but_has_no_ready_readings() {
        let (f, _, n) = crate::option_premium_audit::tests::fixture();
        let r = reference(&f.baseline, &f.session, n, true, "QQQ");
        assert_eq!(r["validForPreparation"], true);
        assert_eq!(r["readingReadyCohorts"], 0);
    }
}
