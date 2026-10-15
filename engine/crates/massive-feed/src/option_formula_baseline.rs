//! Attach original live outputs to outcome cases without hindsight recalculation.
use serde_json::{json, Value};
use snipers_core::calendar;
use std::collections::BTreeSet;
fn valid(row: &Value, now: u64) -> bool {
    let Some(at) = row["observedMs"].as_u64() else {
        return false;
    };
    let Some(published) = row["publishedMs"].as_u64() else {
        return false;
    };
    let numbers = [&row["confluence"]["call"], &row["confluence"]["put"]];
    let scores = numbers.iter().all(|v| {
        v.is_null()
            || v.as_f64()
                .is_some_and(|n| n.is_finite() && (0. ..=100.).contains(&n))
    });
    let probabilities = &row["quant"]["probabilities"];
    let probs = probabilities.is_null()
        || probabilities.as_array().is_some_and(|a| {
            a.len() == 3
                && a.iter()
                    .all(|v| v.as_f64().is_some_and(|x| (0. ..=1.).contains(&x)))
                && (a.iter().filter_map(Value::as_f64).sum::<f64>() - 1.).abs() < 1e-6
        });
    let quant_at = row["quant"]["at"].as_u64();
    row["version"] == "option-formula-baseline-v1"
        && row["source"] == "desktop-live"
        && row["confluence"]["version"] == "direction-confluence-40-40-20-v1"
        && row["quant"]["version"] == "quant-route-close-first-10m-v1"
        && row["quant"]["horizonMinutes"] == 10
        && quant_at.is_some_and(|t| t <= at && at - t <= 3000)
        && published <= at
        && at - published <= 3000
        && at <= now
        && scores
        && probs
        && row["symbol"].as_str().is_some_and(|s| !s.is_empty())
        && calendar::date_at(at).is_some_and(|d| row["session"] == d.to_string())
}
/// Archive is exported by routeJournal; timestamps are consistency checks, not signatures.
pub fn attach(dataset: &mut Value, journal: &Value, now: u64) -> Result<(), String> {
    let archive = &journal["formulaBaseline"];
    if journal["schema"] != "route-observation-v1"
        || archive["version"] != "option-formula-baseline-archive-v1"
    {
        return Err("Exportación de fórmula incompatible".into());
    }
    let rows = archive["rows"].as_array().ok_or("Archivo sin snapshots")?;
    if rows.len() > 10000 {
        return Err("Demasiados snapshots".into());
    }
    let mut seen = BTreeSet::new();
    for row in rows {
        if !valid(row, now) {
            return Err("Snapshot de fórmula inválido, futuro o incompatible".into());
        }
        let key = (
            row["symbol"].as_str().unwrap(),
            row["session"].as_str().unwrap(),
            row["observedMs"].as_u64().unwrap(),
        );
        if !seen.insert(key) {
            return Err("Snapshot duplicado".into());
        }
    }
    let mut attached = 0;
    let mut missing = 0;
    for case in dataset["cases"].as_array_mut().ok_or("Dataset sin casos")? {
        let available = case["readingAvailableMs"]
            .as_u64()
            .ok_or("Caso sin disponibilidad")?;
        let selected = rows
            .iter()
            .filter(|r| r["symbol"] == case["underlying"] && r["session"] == case["date"])
            .filter(|r| {
                r["observedMs"]
                    .as_u64()
                    .is_some_and(|at| at <= available && available - at <= 3000)
            })
            .max_by_key(|r| r["observedMs"].as_u64().unwrap());
        if let Some(row) = selected {
            case["baselineForecast"] = json!({"status":"RECORDED_ORIGINAL_OUTPUT","snapshot":row,
                "quantHorizonMatchesPrimaryOutcome":false,"confluenceIsCalibratedProbability":false});
            attached += 1;
        } else {
            case["baselineForecast"] = Value::Null;
            missing += 1;
        }
    }
    dataset["baselineAttachment"] = json!({"status":if attached>0{"ORIGINAL_OUTPUTS_ATTACHED"}else{"PENDING_BASELINE_SNAPSHOTS"},
        "attachedCases":attached,"missingCases":missing,"maximumAgeMs":3000,"archiveDropped":archive["dropped"],
        "archiveStorageError":archive["storageError"],"predictionRecomputed":false,
        "note":"Confluence is a score; 10-minute upper/lower/neither probabilities are not 15-minute return predictions."});
    if let Some(limits)=dataset["limits"].as_array_mut(){
        limits.retain(|v|v.as_str()!=Some("Current-formula forecasts are absent; incremental predictive benefit cannot yet be measured."));
        limits.push(json!("Baseline availability is reported per case. No incremental experiment or 15-minute probability calibration has been performed."));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Value, Value, u64) {
        let t = calendar::legacy_eastern_ms("2026-10-12 09:40:03").unwrap();
        let d = json!({"cases":[{"underlying":"QQQ","date":"2026-10-12","readingAvailableMs":t,"baselineForecast":null,"labels":[{"returnPct":1}]}]});
        let j = json!({"schema":"route-observation-v1","formulaBaseline":{"version":"option-formula-baseline-archive-v1","dropped":0,"rows":[{
   "version":"option-formula-baseline-v1","source":"desktop-live","symbol":"QQQ","session":"2026-10-12","observedMs":t-1,"publishedMs":t-2,
   "confluence":{"version":"direction-confluence-40-40-20-v1","call":60,"put":40},
   "quant":{"version":"quant-route-close-first-10m-v1","horizonMinutes":10,"at":t-1,"probabilities":null}}]}});
        (d, j, t)
    }
    #[test]
    fn attaches_prior_original_output_without_changing_labels() {
        let (mut d, j, t) = fixture();
        let labels = d["cases"][0]["labels"].clone();
        attach(&mut d, &j, t).unwrap();
        assert_eq!(d["baselineAttachment"]["attachedCases"], 1);
        assert_eq!(d["cases"][0]["labels"], labels);
        assert_eq!(
            d["cases"][0]["baselineForecast"]["quantHorizonMatchesPrimaryOutcome"],
            false
        );
    }
    #[test]
    fn later_and_stale_snapshots_are_not_matched() {
        for delta in [1i64, -3001] {
            let (mut d, mut j, t) = fixture();
            let at = (t as i64 + delta) as u64;
            j["formulaBaseline"]["rows"][0]["observedMs"] = json!(at);
            j["formulaBaseline"]["rows"][0]["publishedMs"] = json!(at);
            j["formulaBaseline"]["rows"][0]["quant"]["at"] = json!(at);
            attach(&mut d, &j, t + 10).unwrap();
            assert!(d["cases"][0]["baselineForecast"].is_null());
        }
    }
    #[test]
    fn malformed_probabilities_and_duplicate_packets_rejected() {
        let (mut d, mut j, t) = fixture();
        j["formulaBaseline"]["rows"][0]["quant"]["probabilities"] = json!([0.9, 0.9, 0.9]);
        assert!(attach(&mut d, &j, t).is_err());
        let (mut d, mut j, t) = fixture();
        let r = j["formulaBaseline"]["rows"][0].clone();
        j["formulaBaseline"]["rows"].as_array_mut().unwrap().push(r);
        assert!(attach(&mut d, &j, t).is_err());
    }
}
