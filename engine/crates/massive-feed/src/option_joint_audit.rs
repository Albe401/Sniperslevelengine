//! Read-only prospective audit. Separate measurement bases, explicit denominators.
use crate::{option_activity_observer::FrozenSession, option_premium_audit, option_volume_audit};
use serde_json::{json, Value};
use snipers_core::{
    option_activity_baseline::Sample, option_activity_detector, option_premium_detector,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

type Key = (String, String, String, u64, u64);
fn key(sample: &Sample, date: &str) -> Key {
    (
        sample.underlying.clone(),
        date.into(),
        sample.ticker.clone(),
        sample.start_ms,
        sample.end_ms,
    )
}
fn percentage(n: usize, d: usize) -> Option<f64> {
    (d > 0).then(|| 100. * n as f64 / d as f64)
}
#[derive(Default)]
struct Engine {
    rows: BTreeMap<Key, Value>,
    sessions: Vec<Value>,
    errors: Vec<Value>,
}
fn inspect(dirs: &[PathBuf], now: u64, premium: bool) -> Result<Engine, String> {
    let audit = if premium {
        option_premium_audit::consolidate(dirs, now)?
    } else {
        option_volume_audit::consolidate(dirs, now)?
    };
    let invalid_files: BTreeSet<_> = audit
        .errors
        .iter()
        .filter_map(|e| e["file"].as_str().map(str::to_string))
        .collect();
    let mut engine = Engine {
        errors: audit.errors,
        ..Default::default()
    };
    for mut session in audit.sessions {
        let directory = PathBuf::from(session["directory"].as_str().ok_or("Carpeta inválida")?);
        if !directory.join("frozen-session.json").exists() {
            session["captureCoverageOfElapsedWindowsPct"] = Value::Null;
            session["readingCoverageOfCertifiedObservationsPct"] = Value::Null;
            engine.sessions.push(session);
            continue;
        }
        let frozen: FrozenSession = serde_json::from_value(option_premium_audit::read(
            &directory.join("frozen-session.json"),
        )?)
        .map_err(|_| "Sesión congelada inválida")?;
        let prefix = if premium {
            "premium-window-"
        } else {
            "window-"
        };
        let due = session["elapsedEligibleWindows"].as_u64().unwrap_or(0) as usize;
        let mut timely_due = 0;
        let mut certified = 0;
        let mut comparable = 0;
        for entry in fs::read_dir(&directory).map_err(|_| "Carpeta inaccesible")? {
            let path = entry.map_err(|_| "Archivo inaccesible")?.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with(prefix)
                || !name.ends_with(".json")
                || invalid_files.contains(&path.to_string_lossy().to_string())
            {
                continue;
            }
            let value = option_premium_audit::read(&path)?;
            let (samples, _, _) = if premium {
                option_premium_audit::verify_window(&frozen, &value, now)?
            } else {
                option_volume_audit::verify_window(&frozen, &value, now)?
            };
            let end = value["endMs"].as_u64().ok_or("Cierre ausente")?;
            let completed = value["captureCompletedMs"]
                .as_u64()
                .ok_or("Reloj ausente")?;
            // Capture coverage counts only the elapsed denominator, never today's not-yet-due windows.
            if end + 120_000 <= now && completed <= end + 120_000 {
                timely_due += 1;
            }
            for sample in samples {
                let status = if premium {
                    option_premium_detector::evaluate(&frozen.baseline, &sample, completed).status
                } else {
                    option_activity_detector::evaluate(&frozen.baseline, &sample, completed).status
                };
                let is_comparable = status == "WITHIN_REFERENCE_EXPERIMENTAL"
                    || status
                        == if premium {
                            "UNUSUAL_PREMIUM_EXPERIMENTAL"
                        } else {
                            "UNUSUAL_VOLUME_EXPERIMENTAL"
                        };
                certified += 1;
                comparable += usize::from(is_comparable);
                let k = key(&sample, &frozen.date);
                let row = json!({"status":status,"comparable":is_comparable,
                    "unusual":status.starts_with("UNUSUAL_"),"sample":sample,
                    "referenceBuiltMs":frozen.baseline.built_ms,"file":path,
                    "contextKnownMs":value["stockEvidence"]["knownMs"],"readingAvailableMs":completed});
                if engine.rows.insert(k, row).is_some() {
                    return Err("Observación duplicada entre sesiones".into());
                }
            }
        }
        session["timelyValidCapturedWindowsWithElapsedDeadline"] = json!(timely_due);
        session["captureCoverageOfElapsedWindowsPct"] = json!(percentage(timely_due, due));
        session["certifiedContractWindows"] = json!(certified);
        session["comparableCertifiedContractWindows"] = json!(comparable);
        session["readingCoverageOfCertifiedObservationsPct"] =
            json!(percentage(comparable, certified));
        engine.sessions.push(session);
    }
    Ok(engine)
}

fn reconcile(volume: Engine, premium: Engine, now: u64) -> Value {
    let keys: BTreeSet<_> = volume
        .rows
        .keys()
        .chain(premium.rows.keys())
        .cloned()
        .collect();
    let mut rows = vec![];
    let mut paired = 0;
    let mut compatible = 0;
    let mut both_comparable = 0;
    let mut both_unusual = 0;
    let mut volume_only = 0;
    let mut premium_only = 0;
    for k in keys {
        let v = volume.rows.get(&k);
        let p = premium.rows.get(&k);
        let (alignment, context_compatible) = match (v, p) {
            (Some(v), Some(p)) => {
                paired += 1;
                let same = v["sample"]["underlyingPrice"] == p["sample"]["underlyingPrice"]
                    && v["sample"]["priceAvailableMs"] == p["sample"]["priceAvailableMs"];
                if same {
                    compatible += 1;
                }
                if same && v["comparable"] == true && p["comparable"] == true {
                    both_comparable += 1;
                    if v["unusual"] == true && p["unusual"] == true {
                        both_unusual += 1;
                    }
                }
                ("PAIRED_CERTIFIED", Some(same))
            }
            (Some(_), None) => {
                volume_only += 1;
                ("VOLUME_ONLY_CERTIFIED", None)
            }
            (None, Some(_)) => {
                premium_only += 1;
                ("PREMIUM_ONLY_CERTIFIED", None)
            }
            _ => unreachable!(),
        };
        rows.push(json!({"underlying":k.0,"date":k.1,"ticker":k.2,"startMs":k.3,"endMs":k.4,
            "alignment":alignment,"priceContextCompatible":context_compatible,"volume":v,"premium":p}));
    }
    let errors = volume.errors.len() + premium.errors.len();
    json!({"version":"option-joint-prospective-audit-v1","generatedMs":now,
        "status":if errors>0 {"AUDIT_ERRORS"} else if rows.is_empty() {"WAITING_VALID_CAPTURE"} else {"AUDITED_EXPERIMENTAL"},
        "volume":{"measurement":"eligible-option-bars","sessions":volume.sessions,"errors":volume.errors},
        "premium":{"measurement":"reported-trades","sessions":premium.sessions,"errors":premium.errors},
        "summary":{"certifiedVolumeContractWindows":volume.rows.len(),"certifiedPremiumContractWindows":premium.rows.len(),
            "pairedCertifiedContractWindows":paired,"pairedPriceCompatibleContractWindows":compatible,
            "volumeOnlyCertifiedContractWindows":volume_only,"premiumOnlyCertifiedContractWindows":premium_only,
            "bothComparablePriceCompatibleContractWindows":both_comparable,"bothUnusualContractWindows":both_unusual,
            "bothUnusualAmongBothComparablePct":percentage(both_unusual,both_comparable),"auditErrors":errors},
        "reconciliation":rows,"limits":["Unpaired certification does not imply absent capture or zero activity.",
            "Eligible-bar contracts and reported-print contracts are different measurements; no equality check or inferred premium.",
            "Price-context compatibility compares prior price and its available clock; receipt clocks may differ.",
            "Coincident unusual readings are descriptive, not independent signals or a quantitative score.",
            "Capture coverage uses timely valid archived windows, including source-pending records; certification and reading coverage are separate.",
            "This is an offline snapshot, not a scheduler or predictive validation."],
        "calibrated":false,"formulaChanged":false,"executionEnabled":false,"ordersSent":0})
}
/// Explicit per-engine folders, each independently recertified; no network or reference replacement.
pub fn report(
    volume_dirs: &[PathBuf],
    premium_dirs: &[PathBuf],
    now: u64,
) -> Result<Value, String> {
    Ok(reconcile(
        inspect(volume_dirs, now, false)?,
        inspect(premium_dirs, now, true)?,
        now,
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    #[test]
    fn missing_capture_has_null_denominators() {
        let dir = std::env::temp_dir().join(format!("missing-joint-{}", crate::live::now_ms()));
        let r = report(&[dir.clone()], &[dir], crate::live::now_ms()).unwrap();
        assert_eq!(r["status"], "WAITING_VALID_CAPTURE");
        assert!(r["summary"]["bothUnusualAmongBothComparablePct"].is_null());
        assert!(r["volume"]["sessions"][0]["captureCoverageOfElapsedWindowsPct"].is_null());
    }
    pub(crate) fn write_fixture(root: &std::path::Path, premium: bool) -> PathBuf {
        let (f, v, _) = if premium {
            option_premium_audit::tests::fixture()
        } else {
            option_volume_audit::tests::fixture()
        };
        let dir = root.join(if premium { "premium" } else { "volume" });
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("frozen-session.json"),
            serde_json::to_vec(&f).unwrap(),
        )
        .unwrap();
        let prefix = if premium {
            "premium-window-"
        } else {
            "window-"
        };
        fs::write(
            dir.join(format!("{prefix}{}.json", v["startMs"].as_u64().unwrap())),
            serde_json::to_vec(&v).unwrap(),
        )
        .unwrap();
        dir
    }
    #[test]
    fn pairs_certified_data_without_equating_measurements() {
        let root = std::env::temp_dir().join(format!(
            "joint-pairs-{}-{}",
            std::process::id(),
            crate::live::now_ms()
        ));
        let v = write_fixture(&root, false);
        let p = write_fixture(&root, true);
        let (_, _, now) = option_premium_audit::tests::fixture();
        let r = report(&[v.clone()], &[p.clone()], now + 120_000).unwrap();
        assert_eq!(r["summary"]["pairedCertifiedContractWindows"], 6);
        assert_eq!(
            r["summary"]["bothComparablePriceCompatibleContractWindows"],
            0
        );
        assert_eq!(
            r["volume"]["sessions"][0]["captureCoverageOfElapsedWindowsPct"],
            100.
        );
        assert_eq!(
            r["volume"]["sessions"][0]["readingCoverageOfCertifiedObservationsPct"],
            0.
        );
        assert_eq!(r["reconciliation"][0]["volume"]["sample"]["contracts"], 50.);
        assert_eq!(r["reconciliation"][0]["premium"]["sample"]["contracts"], 1.);
        assert!(report(&[v.clone(), v], &[p], now + 120_000).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn not_yet_due_capture_never_inflates_coverage() {
        let root = std::env::temp_dir().join(format!("joint-not-due-{}", crate::live::now_ms()));
        let v = write_fixture(&root, false);
        let p = write_fixture(&root, true);
        let (_, _, now) = option_premium_audit::tests::fixture();
        let r = report(&[v], &[p], now).unwrap();
        assert!(r["volume"]["sessions"][0]["captureCoverageOfElapsedWindowsPct"].is_null());
        assert_eq!(r["summary"]["pairedCertifiedContractWindows"], 6);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn corrupted_window_excluded_from_pairing() {
        let root = std::env::temp_dir().join(format!("joint-corrupt-{}", crate::live::now_ms()));
        let v = write_fixture(&root, false);
        let p = write_fixture(&root, true);
        let (_, value, now) = option_volume_audit::tests::fixture();
        let path = v.join(format!(
            "window-{}.json",
            value["startMs"].as_u64().unwrap()
        ));
        let mut value = value;
        value["observations"][0]["alertEligible"] = json!(true);
        fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        let r = report(&[v], &[p], now + 120_000).unwrap();
        assert_eq!(r["status"], "AUDIT_ERRORS");
        assert_eq!(r["summary"]["pairedCertifiedContractWindows"], 0);
        assert_eq!(r["summary"]["premiumOnlyCertifiedContractWindows"], 6);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn price_mismatch_and_one_sided_keys_are_visible() {
        let k = ("QQQ".into(), "2026-10-12".into(), "contract".into(), 1, 2);
        let mut v = Engine::default();
        let mut p = Engine::default();
        v.rows.insert(k.clone(),json!({"sample":{"underlyingPrice":600.,"priceAvailableMs":0},"comparable":true,"unusual":true}));
        p.rows.insert(k,json!({"sample":{"underlyingPrice":601.,"priceAvailableMs":0},"comparable":true,"unusual":true}));
        p.rows.insert(
            ("QQQ".into(), "2026-10-12".into(), "other".into(), 1, 2),
            json!({}),
        );
        let r = reconcile(v, p, 0);
        assert_eq!(r["summary"]["pairedCertifiedContractWindows"], 1);
        assert_eq!(r["summary"]["bothUnusualContractWindows"], 0);
        assert_eq!(r["summary"]["premiumOnlyCertifiedContractWindows"], 1);
    }
}
