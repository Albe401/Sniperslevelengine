//! Common-cutoff units, built only from recertified captures. No fitting or execution.
use crate::{
    option_formula_baseline,
    option_future_outcomes::{self, Policy, PriceEvidence},
};
use serde_json::{json, Value};
use snipers_core::calendar::{self, Day};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
type Key = (String, String, u64, u64);
fn validate_policy(p: &Value, now: u64) -> Result<u64, String> {
    let registered = p["registeredMs"].as_u64().ok_or("Política sin reloj")?;
    if p["version"] != "option-incremental-trial-policy-v1"
        || registered == 0
        || registered > now
        || p["primaryTarget"]["field"] != "returnPct"
        || p["primaryTarget"]["horizonMinutes"] != 15
        || p["primaryTarget"]["unit"] != "percentage-points"
        || p["secondaryHorizonsMinutes"] != json!([5, 30])
        || p["unit"] != "symbol-session-activity-window"
        || p["sharedCutoff"] != "max-reading-available-ms"
        || p["sharedAnchor"] != "first-full-minute-open-after-shared-cutoff"
        || p["baselineMaxAgeMs"] != 3000
        || p["requiredPairsPerUnit"] != 6
        || p["requiredComparableReadingsPerUnit"] != 12
        || p["executionEnabled"] != false
        || p["formulaChanged"] != false
    {
        return Err("Política de unidades incompatible".into());
    }
    Ok(registered)
}
fn assemble(
    joint: Value,
    prices: &[PriceEvidence],
    trial: &Value,
    journal: Option<&Value>,
    now: u64,
) -> Result<Value, String> {
    let registered = validate_policy(trial, now)?;
    let mut price_map = BTreeMap::new();
    for e in prices {
        option_future_outcomes::validate_prices(e, now)?;
        if price_map
            .insert((e.underlying.clone(), e.date.clone()), e)
            .is_some()
        {
            return Err("Precio repetido por símbolo y fecha".into());
        }
    }
    let mut groups = BTreeMap::<Key, Vec<&Value>>::new();
    for row in joint["reconciliation"]
        .as_array()
        .ok_or("Conciliación ausente")?
    {
        let k = (
            row["underlying"].as_str().ok_or("Símbolo ausente")?.into(),
            row["date"].as_str().ok_or("Fecha ausente")?.into(),
            row["startMs"].as_u64().ok_or("Inicio ausente")?,
            row["endMs"].as_u64().ok_or("Cierre ausente")?,
        );
        groups.entry(k).or_default().push(row);
    }
    let mut baseline = json!({"cases":groups.iter().map(|((symbol,date,_,_),rows)|{
        let cutoff=rows.iter().flat_map(|r|[&r["volume"],&r["premium"]]).filter_map(|r|r["readingAvailableMs"].as_u64()).max();
        json!({"underlying":symbol,"date":date,"readingAvailableMs":cutoff,"baselineForecast":null})
    }).collect::<Vec<_>>()});
    if let Some(journal) = journal {
        option_formula_baseline::attach(&mut baseline, journal, now)?;
    }
    let mut eligible = vec![];
    let mut excluded = vec![];
    let mut counts = BTreeMap::<String, usize>::new();
    let mut dates = BTreeSet::new();
    for (index, ((symbol, date, start, end), rows)) in groups.iter().enumerate() {
        let mut reasons = BTreeSet::<String>::new();
        let mut tickers = BTreeSet::new();
        let mut calls = 0;
        let mut puts = 0;
        let mut alerts = [0usize; 4];
        let mut contexts = BTreeSet::new();
        if rows.len() != 6 {
            reasons.insert("incomplete-six-contract-selection".into());
        }
        for row in rows {
            if !tickers.insert(row["ticker"].as_str().unwrap_or("")) {
                reasons.insert("duplicated-contract".into());
            }
            if row["alignment"] != "PAIRED_CERTIFIED" {
                reasons.insert("unpaired-certification".into());
            }
            if row["priceContextCompatible"] != true {
                reasons.insert("incompatible-prior-price-context".into());
            }
            let kind = row["volume"]["sample"]["kind"].as_str();
            match kind {
                Some("call") => calls += 1,
                Some("put") => puts += 1,
                _ => {
                    reasons.insert("invalid-contract-side".into());
                }
            }
            for (motor, base) in [("volume", 0), ("premium", 2)] {
                let r = &row[motor];
                if r["comparable"] != true {
                    reasons.insert(format!("{motor}-reference-pending"));
                }
                if r["unusual"] == true {
                    alerts[base + usize::from(kind == Some("put"))] += 1;
                }
                if let (Some(price), Some(at)) = (
                    r["sample"]["underlyingPrice"].as_f64(),
                    r["sample"]["priceAvailableMs"].as_u64(),
                ) {
                    contexts.insert((price.to_bits(), at));
                }
            }
        }
        if calls != 3 || puts != 3 {
            reasons.insert("selection-not-three-call-three-put".into());
        }
        if contexts.len() != 1 {
            reasons.insert("mixed-prior-price-contexts".into());
        }
        let b = &baseline["cases"][index];
        let cutoff = b["readingAvailableMs"]
            .as_u64()
            .ok_or("Unidad sin disponibilidad")?;
        if registered >= cutoff {
            reasons.insert("trial-policy-not-prior-to-reading".into());
        }
        let original = &b["baselineForecast"];
        if original.is_null() {
            reasons.insert("missing-fresh-original-formula".into());
        }
        let confluence = &original["snapshot"]["confluence"];
        let signed = confluence["call"]
            .as_f64()
            .zip(confluence["put"].as_f64())
            .filter(|(c, p)| (c + p - 100.).abs() < 1e-6)
            .map(|(c, p)| (c - p) / 100.);
        if signed.is_none() {
            reasons.insert("original-confluence-pending".into());
        }
        let session = calendar::day_bounds(date)
            .and_then(|(t, _)| calendar::date_at(t))
            .map(calendar::schedule);
        let fraction = match session {
            Some(Day::Open(s)) if cutoff >= s.open_ms && cutoff < s.close_ms => {
                Some((cutoff - s.open_ms) as f64 / (s.close_ms - s.open_ms) as f64)
            }
            _ => None,
        };
        if fraction.is_none() {
            reasons.insert("cutoff-outside-session".into());
        }
        let evidence = price_map.get(&(symbol.clone(), date.clone())).copied();
        let labels: Vec<_> = [5, 15, 30]
            .iter()
            .map(|h| option_future_outcomes::label(symbol, date, cutoff, *h, evidence, now))
            .collect();
        let primary = &labels[1];
        if primary["status"] != "LABELED" {
            reasons.insert(format!(
                "primary-outcome-{}",
                primary["reason"]
                    .as_str()
                    .unwrap_or(primary["status"].as_str().unwrap_or("pending"))
            ));
        }
        let features = if reasons.is_empty() {
            json!({"signedConfluence":signed,"sessionFraction":fraction,
            "volCall":alerts[0]as f64/3.,"volPut":alerts[1]as f64/3.,"premCall":alerts[2]as f64/3.,"premPut":alerts[3]as f64/3.})
        } else {
            Value::Null
        };
        let unit = json!({"id":format!("{symbol}:{date}:{start}"),"underlying":symbol,"date":date,"windowStartMs":start,"windowEndMs":end,
            "sharedCutoffMs":cutoff,"sharedAnchorMs":primary["startMs"],"originalBaseline":original,
            "features":features,
            "labels":labels,"primaryReturnPct":primary["returnPct"],"contractEvidence":rows,
            "status":if reasons.is_empty(){"ELIGIBLE_SHARED_UNIT"}else{"EXCLUDED"},"exclusionReasons":reasons});
        if reasons.is_empty() {
            dates.insert(date.clone());
            eligible.push(unit);
        } else {
            for r in reasons {
                *counts.entry(r).or_default() += 1;
            }
            excluded.push(unit);
        }
    }
    let errors = joint["summary"]["auditErrors"].as_u64().unwrap_or(0);
    Ok(
        json!({"version":"option-trial-shared-units-v1","generatedMs":now,"trialPolicy":trial,
        "status":if errors>0{"AUDIT_ERRORS"}else if eligible.is_empty(){"WAITING_ELIGIBLE_SHARED_UNITS"}else{"ASSEMBLED_RESEARCH_ONLY"},
        "summary":{"candidateWindows":eligible.len()+excluded.len(),"eligibleUnits":eligible.len(),"excludedUnits":excluded.len(),
            "eligibleSessions":dates.len(),"auditErrors":errors,"exclusionCounts":counts},
        "units":eligible,"excluded":excluded,"jointAudit":joint,"baselineAttachment":baseline["baselineAttachment"],
        "assemblerImplemented":true,"trainingImplemented":false,"evaluationImplemented":false,
        "limits":["Excluded units have null features; zero fractions are only meaningful for fully comparable selections.",
            "Primary eligibility requires the 15-minute label; 5/30-minute secondary labels may still be pending.",
            "Dates with no certified candidate windows remain in the source-session audit; they are not fabricated as zero-return units.",
            "The registered policy snapshot keeps its original implementation flags; runtime implementation status is reported separately."],
        "formulaChanged":false,"executionEnabled":false,"ordersSent":0}),
    )
}
/// The individual dataset is rebuilt for audit; its per-motor labels are never reused as shared labels.
pub fn build(
    volume: &[PathBuf],
    premium: &[PathBuf],
    prices: &[PriceEvidence],
    outcome_policy: &Policy,
    trial: &Value,
    journal: Option<&Value>,
    now: u64,
) -> Result<Value, String> {
    let audited = option_future_outcomes::build(volume, premium, prices, outcome_policy, now)?;
    assemble(audited["jointAudit"].clone(), prices, trial, journal, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_pipeline_reaudits_original_captures_and_preserves_pending_references() {
        let (_, prices, trial, journal, now) = fixture();
        let root = std::env::temp_dir().join(format!(
            "trial-reaudit-{}-{}",
            std::process::id(),
            crate::live::now_ms()
        ));
        let volume = crate::option_joint_audit::tests::write_fixture(&root, false);
        let premium = crate::option_joint_audit::tests::write_fixture(&root, true);
        let policy = Policy {
            version: "option-future-outcome-policy-v1".into(),
            registered_ms: trial["registeredMs"].as_u64().unwrap(),
            horizons_minutes: vec![5, 15, 30],
            primary_horizon_minutes: 15,
            anchor: "first-full-minute-open-after-reading".into(),
        };
        let r = build(
            &[volume],
            &[premium],
            &prices,
            &policy,
            &trial,
            Some(&journal),
            now,
        )
        .unwrap();
        assert_eq!(r["summary"]["candidateWindows"], 1);
        assert_eq!(r["summary"]["eligibleUnits"], 0);
        assert_eq!(
            r["summary"]["exclusionCounts"]["volume-reference-pending"],
            1
        );
        assert!(r["excluded"][0]["features"].is_null());
        std::fs::remove_dir_all(root).unwrap();
    }
    fn fixture() -> (Value, Vec<PriceEvidence>, Value, Value, u64) {
        let start = calendar::legacy_eastern_ms("2026-10-12 09:35:00").unwrap();
        let end = start + 300_000;
        let cutoff = end + 61_000;
        let anchor = end + 120_000;
        let now = anchor + 30 * 60_000;
        let rows:Vec<_>=(0..6).map(|i|{let side=if i<3{"call"}else{"put"};let r=|available|json!({"comparable":true,"unusual":i==0,
            "readingAvailableMs":available,"sample":{"kind":side,"underlyingPrice":600.,"priceAvailableMs":start-60_000}});
            json!({"underlying":"QQQ","date":"2026-10-12","ticker":format!("contract-{i}"),"startMs":start,"endMs":end,
                "alignment":"PAIRED_CERTIFIED","priceContextCompatible":true,"volume":r(end+3000),"premium":r(cutoff)})}).collect();
        let joint = json!({"reconciliation":rows,"summary":{"auditErrors":0}});
        let prices = vec![PriceEvidence {
            version: "stock-context-evidence-v1".into(),
            underlying: "QQQ".into(),
            date: "2026-10-12".into(),
            adjusted: false,
            collected_ms: now,
            bars: (0..30)
                .map(|i| option_future_outcomes::PriceBar {
                    timestamp_ms: anchor + i * 60_000,
                    open: 100.,
                    high: 102.,
                    low: 99.,
                    close: 101.,
                    volume: 10.,
                })
                .collect(),
        }];
        let trial = json!({"version":"option-incremental-trial-policy-v1","registeredMs":start-86_400_000,"primaryTarget":{"field":"returnPct","horizonMinutes":15,"unit":"percentage-points"},
            "secondaryHorizonsMinutes":[5,30],"unit":"symbol-session-activity-window","sharedCutoff":"max-reading-available-ms","sharedAnchor":"first-full-minute-open-after-shared-cutoff", 
            "baselineMaxAgeMs":3000,"requiredPairsPerUnit":6,"requiredComparableReadingsPerUnit":12,"executionEnabled":false,"formulaChanged":false});
        let journal = json!({"schema":"route-observation-v1","formulaBaseline":{"version":"option-formula-baseline-archive-v1","rows":[{
            "version":"option-formula-baseline-v1","source":"desktop-live","symbol":"QQQ","session":"2026-10-12","observedMs":cutoff-1,"publishedMs":cutoff-2,
            "confluence":{"version":"direction-confluence-40-40-20-v1","call":60,"put":40},"quant":{"version":"quant-route-close-first-10m-v1","horizonMinutes":10,"at":cutoff-1,"probabilities":null}}]}});
        (joint, prices, trial, journal, now)
    }
    #[test]
    fn six_contracts_become_one_unit_with_later_shared_anchor() {
        let (j, p, t, b, n) = fixture();
        let r = assemble(j, &p, &t, Some(&b), n).unwrap();
        assert_eq!(r["summary"]["eligibleUnits"], 1);
        assert_eq!(r["units"][0]["features"]["volCall"], 1. / 3.);
        assert_eq!(r["units"][0]["sharedAnchorMs"], p[0].bars[0].timestamp_ms);
        assert_eq!(
            r["units"][0]["originalBaseline"]["snapshot"]["observedMs"],
            r["units"][0]["sharedCutoffMs"].as_u64().unwrap() - 1
        );
    }
    #[test]
    fn pending_or_incomplete_selection_never_becomes_zero_features() {
        for mode in 0..3 {
            let (mut j, p, t, b, n) = fixture();
            match mode {
                0 => {
                    j["reconciliation"].as_array_mut().unwrap().pop();
                }
                1 => j["reconciliation"][0]["premium"]["comparable"] = json!(false),
                _ => j["reconciliation"][0]["priceContextCompatible"] = json!(false),
            }
            let r = assemble(j, &p, &t, Some(&b), n).unwrap();
            assert!(r["units"].as_array().unwrap().is_empty());
            assert_eq!(r["summary"]["excludedUnits"], 1);
        }
    }
    #[test]
    fn absent_baseline_missing_prices_and_late_policy_are_excluded() {
        let (j, p, t, b, n) = fixture();
        assert_eq!(
            assemble(j.clone(), &p, &t, None, n).unwrap()["summary"]["eligibleUnits"],
            0
        );
        assert_eq!(
            assemble(j.clone(), &[], &t, Some(&b), n).unwrap()["summary"]["eligibleUnits"],
            0
        );
        let mut late = t;
        late["registeredMs"] = json!(n - 1);
        assert_eq!(
            assemble(j, &p, &late, Some(&b), n).unwrap()["summary"]["eligibleUnits"],
            0
        );
    }
    #[test]
    fn zero_alerts_remain_eligible_and_secondary_pending_is_visible() {
        let (mut j, mut p, t, b, n) = fixture();
        for row in j["reconciliation"].as_array_mut().unwrap() {
            row["volume"]["unusual"] = json!(false);
            row["premium"]["unusual"] = json!(false);
        }
        p[0].bars.truncate(15);
        p[0].collected_ms = p[0].bars.last().unwrap().timestamp_ms + 60_000;
        let r = assemble(j, &p, &t, Some(&b), p[0].collected_ms).unwrap();
        assert_eq!(r["summary"]["eligibleUnits"], 1);
        assert_eq!(r["units"][0]["features"]["premCall"], 0.);
        assert_eq!(r["units"][0]["labels"][2]["status"], "PENDING");
        assert!(n > p[0].collected_ms);
    }
    #[test]
    fn duplicate_prices_and_future_policy_rejected() {
        let (j, p, t, b, n) = fixture();
        assert!(assemble(j.clone(), &[p[0].clone(), p[0].clone()], &t, Some(&b), n).is_err());
        let mut t = t;
        t["registeredMs"] = json!(n + 1);
        assert!(assemble(j, &p, &t, Some(&b), n).is_err());
    }
}
