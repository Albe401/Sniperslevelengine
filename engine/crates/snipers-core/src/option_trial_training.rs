//! Fixed research ridge models. No live formula changes or trading permissions.
use crate::calendar::{self, Day};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
const COLUMNS: [&str; 6] = [
    "signedConfluence",
    "sessionFraction",
    "volCall",
    "volPut",
    "premCall",
    "premPut",
];
const VARIANTS: [(&str, usize); 4] = [("M0", 2), ("M1", 4), ("M2", 4), ("M3", 6)];
fn columns(name: &str) -> Vec<usize> {
    match name {
        "M0" => vec![0, 1],
        "M1" => vec![0, 1, 2, 3],
        "M2" => vec![0, 1, 4, 5],
        _ => vec![0, 1, 2, 3, 4, 5],
    }
}
#[derive(Clone, PartialEq)]
struct Row {
    id: String,
    date: String,
    x: [f64; 6],
    y: f64,
}
fn session(date: &str) -> Result<calendar::Session, String> {
    match calendar::day_bounds(date)
        .and_then(|(t, _)| calendar::date_at(t))
        .map(calendar::schedule)
    {
        Some(Day::Open(s)) => Ok(s),
        _ => Err("Fecha de unidad sin sesión válida".into()),
    }
}
fn next_open(date: &str) -> Result<u64, String> {
    let s = session(date)?;
    let mut day = calendar::date_at(s.open_ms).ok_or("Calendario inválido")?;
    for _ in 0..14 {
        day = day.succ_opt().ok_or("Calendario fuera de rango")?;
        match calendar::schedule(day) {
            Day::Open(s) => return Ok(s.open_ms),
            Day::Unknown => return Err("Calendario siguiente desconocido".into()),
            Day::Closed => {}
        }
    }
    Err("No se encontró próxima sesión".into())
}
fn policy(p: &Value, now: u64) -> Result<(), String> {
    if p["version"] != "option-incremental-trial-policy-v1"
        || p["registeredMs"].as_u64().is_none_or(|t| t == 0 || t > now)
        || p["primaryTarget"]["field"] != "returnPct"
        || p["primaryTarget"]["horizonMinutes"] != 15
        || p["primaryTarget"]["unit"] != "percentage-points"
        || p["model"]["family"] != "ridge"
        || p["model"]["lambda"].as_f64() != Some(1.)
        || p["model"]["interceptPenalized"] != false
        || p["model"]["standardization"] != "training-only"
        || p["model"]["lossWeighting"] != "equal-session-then-equal-unit"
        || p["model"]["tuningOnEvaluation"] != false
        || p["split"]["trainingEligibleSessions"] != 20
        || p["split"]["evaluationEligibleSessions"] != 20
        || p["split"]["order"] != "chronological-ET-sessions"
        || p["split"]["freezeAfterTraining"] != true
        || p["executionEnabled"] != false
        || p["formulaChanged"] != false
    {
        return Err("Política de entrenamiento incompatible".into());
    }
    for (name, _) in VARIANTS {
        let expected: Vec<_> = columns(name).iter().map(|i| COLUMNS[*i]).collect();
        if p["variants"][name] != json!(expected) {
            return Err("Columnas del ensayo incompatibles".into());
        }
    }
    Ok(())
}
fn rows(dataset: &Value, now: u64) -> Result<Vec<Row>, String> {
    let mut rows = vec![];
    let mut ids = BTreeSet::new();
    let mut windows = BTreeSet::new();
    let source = dataset["units"].as_array().ok_or("Dataset sin unidades")?;
    if source.len() > 5000 {
        return Err("Dataset excede 5000 unidades".into());
    }
    let registered = dataset["trialPolicy"]["registeredMs"].as_u64().unwrap();
    for u in source {
        let id = u["id"].as_str().ok_or("Unidad sin id")?.to_owned();
        let date = u["date"].as_str().ok_or("Unidad sin fecha")?.to_owned();
        let s = session(&date)?;
        let cutoff = u["sharedCutoffMs"].as_u64().ok_or("Unidad sin cutoff")?;
        let anchor = u["sharedAnchorMs"].as_u64().ok_or("Unidad sin ancla")?;
        let start = u["windowStartMs"].as_u64().ok_or("Unidad sin inicio")?;
        let end = u["windowEndMs"].as_u64().ok_or("Unidad sin cierre")?;
        let label = u["labels"]
            .as_array()
            .and_then(|a| a.iter().find(|v| v["horizonMinutes"] == 15))
            .ok_or("Etiqueta principal ausente")?;
        if u["status"] != "ELIGIBLE_SHARED_UNIT"
            || u["underlying"] != "QQQ"
            || !ids.insert(id.clone())
            || !windows.insert((date.clone(), start))
            || start < s.open_ms
            || start >= s.close_ms
            || end != start + 300_000
            || !(start - s.open_ms).is_multiple_of(300_000)
            || end > s.close_ms
            || cutoff < end
            || cutoff > end + 120_000
            || cutoff <= registered
            || cutoff >= s.close_ms
            || anchor > s.close_ms
            || anchor != (cutoff / 60_000 + 1) * 60_000
            || label["status"] != "LABELED"
            || label["startMs"] != anchor
            || label["endMs"] != anchor + 900_000
            || anchor + 900_000 > s.close_ms
            || label["priceCollectedMs"]
                .as_u64()
                .is_none_or(|t| t < anchor + 900_000 || t > now)
        {
            return Err("Unidad duplicada o temporalmente inválida".into());
        }
        let mut x = [0.; 6];
        for (i, c) in COLUMNS.iter().enumerate() {
            x[i] = u["features"][*c]
                .as_f64()
                .filter(|v| v.is_finite())
                .ok_or("Variable ausente o no finita")?;
        }
        if !(-1. ..=1.).contains(&x[0])
            || x[1] < 0.
            || x[1] > 1.
            || x[2..].iter().any(|v| !(0. ..=1.).contains(v))
        {
            return Err("Variables fuera de rango".into());
        }
        let y = u["primaryReturnPct"]
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or("Resultado ausente")?;
        if label["returnPct"].as_f64() != Some(y) {
            return Err("Resultado principal contradictorio".into());
        }
        rows.push(Row { id, date, x, y });
    }
    rows.sort_by(|a, b| (&a.date, &a.id).cmp(&(&b.date, &b.id)));
    Ok(rows)
}
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Result<Vec<f64>, String> {
    for k in 0..b.len() {
        let pivot = (k..b.len())
            .max_by(|i, j| a[*i][k].abs().total_cmp(&a[*j][k].abs()))
            .unwrap();
        a.swap(k, pivot);
        b.swap(k, pivot);
        let d = a[k][k];
        if !d.is_finite() || d.abs() < 1e-12 {
            return Err("Sistema numérico degenerado".into());
        }
        for i in k + 1..b.len() {
            let f = a[i][k] / d;
            for j in k..b.len() {
                a[i][j] -= f * a[k][j];
            }
            b[i] -= f * b[k];
        }
    }
    let mut x = vec![0.; b.len()];
    for i in (0..b.len()).rev() {
        x[i] = (b[i] - (i + 1..b.len()).map(|j| a[i][j] * x[j]).sum::<f64>()) / a[i][i];
    }
    if x.iter().any(|v| !v.is_finite()) {
        return Err("Coeficiente no finito".into());
    }
    Ok(x)
}
fn fit(rows: &[&Row], name: &str) -> Result<Value, String> {
    let cols = columns(name);
    let n = cols.len();
    let mut per_date = BTreeMap::new();
    for r in rows {
        *per_date.entry(&r.date).or_insert(0usize) += 1;
    }
    let weights: Vec<_> = rows
        .iter()
        .map(|r| 1. / (per_date.len() * per_date[&r.date]) as f64)
        .collect();
    let mean_y = rows.iter().zip(&weights).map(|(r, w)| w * r.y).sum::<f64>();
    let means: Vec<_> = cols
        .iter()
        .map(|c| {
            rows.iter()
                .zip(&weights)
                .map(|(r, w)| w * r.x[*c])
                .sum::<f64>()
        })
        .collect();
    let scales: Vec<_> = cols
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let sd = rows
                .iter()
                .zip(&weights)
                .map(|(r, w)| w * (r.x[*c] - means[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            if sd <= 1e-12 {
                1.
            } else {
                sd
            }
        })
        .collect();
    let mut a = vec![vec![0.; n]; n];
    let mut b = vec![0.; n];
    for (r, w) in rows.iter().zip(weights) {
        let z: Vec<_> = cols
            .iter()
            .enumerate()
            .map(|(i, c)| (r.x[*c] - means[i]) / scales[i])
            .collect();
        for i in 0..n {
            b[i] += w * z[i] * (r.y - mean_y);
            for j in 0..n {
                a[i][j] += w * z[i] * z[j];
            }
        }
    }
    for (i, row) in a.iter_mut().enumerate() {
        row[i] += 1.;
    }
    let coefficients = solve(a, b)?;
    if !mean_y.is_finite() || means.iter().chain(&scales).any(|v| !v.is_finite()) {
        return Err("Normalización no finita".into());
    }
    Ok(
        json!({"variant":name,"columns":cols.iter().map(|i|COLUMNS[*i]).collect::<Vec<_>>(),"means":means,"scales":scales,
        "coefficients":coefficients,"intercept":mean_y,"lambda":1,"interceptPenalized":false}),
    )
}
/// Research artifacts only. Late construction remains explicitly retrospective.
pub fn train(dataset: &Value, now: u64) -> Result<Value, String> {
    if dataset["version"] != "option-trial-shared-units-v1"
        || dataset["executionEnabled"] != false
        || dataset["formulaChanged"] != false
        || dataset["generatedMs"].as_u64().is_none_or(|t| t > now)
    {
        return Err("Dataset incompatible o futuro".into());
    }
    policy(&dataset["trialPolicy"], now)?;
    if dataset["summary"]["auditErrors"].as_u64() != Some(0) {
        return Ok(
            json!({"version":"option-trial-training-v1","builtMs":now,"status":"PENDING_AUDIT_ERRORS","models":null,"trainingImplemented":true,"evaluationImplemented":false,"formulaChanged":false,"executionEnabled":false,"ordersSent":0}),
        );
    }
    let all = rows(dataset, now)?;
    let dates: Vec<_> = all
        .iter()
        .map(|r| r.date.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let pending = |status: &str| {
        json!({"version":"option-trial-training-v1","builtMs":now,"status":status,"eligibleSessions":dates.len(),
        "requiredTrainingSessions":20,"models":null,"trainingImplemented":true,"evaluationImplemented":false,"formulaChanged":false,"executionEnabled":false,"ordersSent":0})
    };
    if dates.len() < 20 {
        return Ok(pending("PENDING_TRAINING_SESSIONS"));
    }
    let selected = &dates[..20];
    let last = session(&selected[19])?;
    if now < last.close_ms {
        return Ok(pending("WAITING_LAST_TRAINING_SESSION_CLOSE"));
    }
    let training: Vec<_> = all.iter().filter(|r| selected.contains(&r.date)).collect();
    let models: Vec<_> = VARIANTS
        .iter()
        .map(|(name, _)| fit(&training, name))
        .collect::<Result<_, _>>()?;
    let deadline = next_open(&selected[19])?;
    let timely = now < deadline;
    let mut counts = BTreeMap::new();
    for r in &training {
        *counts.entry(&r.date).or_insert(0usize) += 1;
    }
    let control = training
        .iter()
        .map(|r| r.y / (20 * counts[&r.date]) as f64)
        .sum::<f64>();
    Ok(
        json!({"version":"option-trial-training-v1","builtMs":now,"status":if timely{"FROZEN_RESEARCH_MODELS"}else{"RETROSPECTIVE_DIAGNOSTIC_FIT"},
        "trialPolicy":dataset["trialPolicy"],"trainingDates":selected,"trainingUnitIds":training.iter().map(|r|&r.id).collect::<Vec<_>>(),
        "trainingUnits":training.len(),"trainingSessions":20,"trainingUnitsByDate":counts,"models":models,
        "controlTrainingMeanReturnPct":control,"prospectiveFreezeDeadlineMs":deadline,"availableBeforeEvaluation":timely,
        "remainingEligibleSessions":dates.len()-20,"evaluationPredictions":null,"evaluationMetrics":null,
        "limits":["Fixed ridge research calibration of archived confluence; live formula and its weights remain unchanged.",
            "Late construction is diagnostic, never a previously available forecast.","Evaluation labels do not enter fitting or normalization; no performance claim is made.",
            "Model files are immutable snapshots, not signed artifacts or an active model registry."],
        "trainingImplemented":true,"evaluationImplemented":false,"calibratedForProduction":false,"formulaChanged":false,"executionEnabled":false,"ordersSent":0}),
    )
}

/// Verify the frozen training artifact against its archived source, then score
/// the next complete block without fitting on evaluation observations.
pub fn evaluate(
    dataset: &Value,
    frozen: &Value,
    source: &Value,
    now: u64,
) -> Result<Value, String> {
    let pending = |status: &str, count: usize| {
        json!({"version":"option-trial-evaluation-v1",
        "builtMs":now,"status":status,"eligibleEvaluationSessions":count,"requiredEvaluationSessions":20,
        "metrics":null,"comparisons":null,"predictions":null,"evaluationImplemented":true,
        "formulaChanged":false,"executionEnabled":false,"ordersSent":0})
    };
    if frozen["version"] != "option-trial-training-v1" {
        return Err("Archivo de entrenamiento incompatible".into());
    }
    if frozen["models"].is_null() {
        return Ok(pending("PENDING_FROZEN_MODELS", 0));
    }
    let built = frozen["builtMs"]
        .as_u64()
        .filter(|t| *t <= now)
        .ok_or("Modelo futuro o sin fecha")?;
    let verified = train(source, built)?;
    for field in [
        "status",
        "models",
        "trialPolicy",
        "trainingDates",
        "trainingUnitIds",
        "trainingUnits",
        "trainingSessions",
        "trainingUnitsByDate",
        "controlTrainingMeanReturnPct",
        "prospectiveFreezeDeadlineMs",
        "availableBeforeEvaluation",
        "formulaChanged",
        "executionEnabled",
    ] {
        if frozen[field] != verified[field] {
            return Err(format!("Modelo o fuente alterados: {field}"));
        }
    }
    if verified["status"] != "FROZEN_RESEARCH_MODELS" {
        return Err("Ajuste retrospectivo no admisible para evaluación prospectiva".into());
    }
    if dataset["version"] != "option-trial-shared-units-v1"
        || dataset["generatedMs"].as_u64().is_none_or(|t| t > now)
        || dataset["executionEnabled"] != false
        || dataset["formulaChanged"] != false
        || dataset["trialPolicy"] != source["trialPolicy"]
    {
        return Err("Dataset de evaluación incompatible".into());
    }
    let u = &dataset["trialPolicy"]["uncertainty"];
    if dataset["trialPolicy"]["primaryMetric"] != "equal-session-mean-absolute-error"
        || u["method"] != "paired-session-percentile-bootstrap"
        || u["replicates"] != 10000
        || u["seed"] != 20261010
        || u["primaryComparisons"] != 3
        || u["multiplicity"] != "Bonferroni"
        || u["individualConfidence"]
            .as_f64()
            .is_none_or(|v| !v.is_finite() || (v - 0.9833333333333333).abs() > 1e-14)
    {
        return Err("Política de evaluación incompatible".into());
    }
    if dataset["summary"]["auditErrors"] != 0 {
        return Ok(pending("PENDING_AUDIT_ERRORS", 0));
    }
    let original = rows(source, built)?;
    let all = rows(dataset, now)?;
    let training_dates: Vec<_> = verified["trainingDates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let last = training_dates[19];
    let old: Vec<_> = original
        .iter()
        .filter(|r| training_dates.contains(&r.date.as_str()))
        .collect();
    let current: Vec<_> = all.iter().filter(|r| r.date.as_str() <= last).collect();
    if old != current {
        return Err("Unidades de entrenamiento cambiaron o falta historial original".into());
    }
    let dates: Vec<_> = all
        .iter()
        .filter(|r| r.date.as_str() > last)
        .map(|r| r.date.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if dates.len() < 20 {
        return Ok(pending("PENDING_EVALUATION_SESSIONS", dates.len()));
    }
    let selected = &dates[..20];
    if now < session(&selected[19])?.close_ms {
        return Ok(pending("WAITING_LAST_EVALUATION_SESSION_CLOSE", 20));
    }
    let evaluation: Vec<_> = all.iter().filter(|r| selected.contains(&r.date)).collect();
    let models = frozen["models"].as_array().unwrap();
    let control = frozen["controlTrainingMeanReturnPct"].as_f64().unwrap();
    let mut per_date: BTreeMap<String, (usize, [f64; 5])> = BTreeMap::new();
    let mut predictions = vec![];
    for r in &evaluation {
        if r.date.as_str() <= last || session(&r.date)?.open_ms <= built {
            return Err("Evaluación anterior a congelación".into());
        }
        let mut estimates = [control; 5];
        for (i, m) in models.iter().enumerate() {
            estimates[i] = m["intercept"].as_f64().unwrap();
            for (j, c) in columns(m["variant"].as_str().unwrap()).iter().enumerate() {
                estimates[i] += m["coefficients"][j].as_f64().unwrap()
                    * (r.x[*c] - m["means"][j].as_f64().unwrap())
                    / m["scales"][j].as_f64().unwrap();
            }
        }
        let errors = estimates.map(|v| (v - r.y).abs());
        if estimates.iter().chain(&errors).any(|v| !v.is_finite()) {
            return Err("Predicción o error no finitos".into());
        }
        let entry = per_date.entry(r.date.clone()).or_insert((0, [0.; 5]));
        entry.0 += 1;
        for (i, e) in errors.iter().enumerate() {
            entry.1[i] += e;
        }
        predictions.push(json!({"id":r.id,"date":r.date,"actualReturnPct":r.y,"modelReturnPct":&estimates[..4],"controlReturnPct":control,"absoluteErrors":errors}));
    }
    let session_mae: Vec<_> = per_date
        .iter()
        .map(|(date, (count, sums))| (date, sums.map(|v| v / *count as f64)))
        .collect();
    let means: Vec<_> = (0..5)
        .map(|i| session_mae.iter().map(|(_, v)| v[i] / 20.).sum::<f64>())
        .collect();
    if means.iter().any(|v| !v.is_finite()) {
        return Err("Métrica no finita".into());
    }
    let mut comparisons = vec![];
    for i in 1..4 {
        let diffs: Vec<_> = session_mae.iter().map(|(_, v)| v[0] - v[i]).collect();
        let (low, high) = bootstrap(&diffs);
        comparisons.push(json!({"variant":format!("M{i}"),"reference":"M0","meanImprovementPctPoints":means[0]-means[i],
            "confidence":0.9833333333333333,"lower":low,"upper":high,"positiveInterval":low>0.}));
    }
    Ok(
        json!({"version":"option-trial-evaluation-v1","builtMs":now,"status":"EVALUATION_COMPLETE_RESEARCH_ONLY",
        "frozenModelBuiltMs":built,"evaluationDates":selected,"evaluationSessions":20,"evaluationUnits":evaluation.len(),
        "ignoredLaterEligibleSessions":dates.len()-20,"metricOrder":["M0","M1","M2","M3","control"],
        "metrics":{"equalSessionMAE":means,"unit":"percentage-points"},"sessionMAE":session_mae.iter().map(|(d,v)|json!({"date":d,"mae":v})).collect::<Vec<_>>(),
        "comparisons":comparisons,"predictions":predictions,"bootstrapReplicates":10000,"bootstrapSeed":20261010,
        "replicationRequiredBeforeIntegration":true,"economicMinimumImprovement":null,"finiteSampleCoverageGuaranteed":false,
        "limits":["Frozen parameters applied offline; this does not prove live forecast delivery or executable trading returns.","Session bootstrap does not guarantee coverage with 20 sessions or serial dependence.","Secondary horizons are not used to choose the primary result."],
        "evaluationImplemented":true,"formulaChanged":false,"executionEnabled":false,"ordersSent":0}),
    )
}
fn bootstrap(diffs: &[f64]) -> (f64, f64) {
    let mut state = 20261010u64;
    let mut samples = Vec::with_capacity(10000);
    for _ in 0..10000 {
        let mut sum = 0.;
        for _ in diffs {
            // Rejection sampling avoids modulo bias for the paired session index.
            let bound = diffs.len() as u64;
            let limit = u64::MAX - u64::MAX % bound;
            let draw = loop {
                state = state.wrapping_add(0x9e3779b97f4a7c15);
                let mut z = state;
                z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
                z ^= z >> 31;
                if z < limit {
                    break z;
                }
            };
            sum += diffs[(draw % bound) as usize] / diffs.len() as f64;
        }
        samples.push(sum);
    }
    samples.sort_by(f64::total_cmp);
    let quantile = |p: f64| {
        let pos = p * (samples.len() - 1) as f64;
        let lo = pos.floor() as usize;
        samples[lo] + (samples[pos.ceil() as usize] - samples[lo]) * (pos - lo as f64)
    };
    (quantile(1. / 120.), quantile(119. / 120.))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data(n: usize) -> (Value, u64) {
        let mut day =
            calendar::date_at(calendar::legacy_eastern_ms("2026-10-12 09:30:00").unwrap()).unwrap();
        let mut dates = vec![];
        while dates.len() < n {
            if let Day::Open(s) = calendar::schedule(day) {
                dates.push(s);
            }
            day = day.succ_opt().unwrap();
        }
        let policy = json!({"version":"option-incremental-trial-policy-v1","registeredMs":dates[0].open_ms-86_400_000,
   "primaryTarget":{"field":"returnPct","horizonMinutes":15,"unit":"percentage-points"},"model":{"family":"ridge","lambda":1,"interceptPenalized":false,"standardization":"training-only","lossWeighting":"equal-session-then-equal-unit","tuningOnEvaluation":false},
   "split":{"trainingEligibleSessions":20,"evaluationEligibleSessions":20,"order":"chronological-ET-sessions","freezeAfterTraining":true},"variants":{"M0":["signedConfluence","sessionFraction"],"M1":["signedConfluence","sessionFraction","volCall","volPut"],"M2":["signedConfluence","sessionFraction","premCall","premPut"],"M3":COLUMNS},"executionEnabled":false,"formulaChanged":false});
        let now = dates.last().unwrap().close_ms + 1;
        let units:Vec<_>=dates.iter().enumerate().map(|(i,s)|{let x=if i%2==0{-1.}else{1.};let start=s.open_ms+300_000;let cutoff=start+301_000;let anchor=(cutoff/60_000+1)*60_000;let y=7.+2.*x;
   json!({"id":format!("QQQ:{}:{start}",s.date),"underlying":"QQQ","date":s.date,"status":"ELIGIBLE_SHARED_UNIT","windowStartMs":start,"windowEndMs":start+300_000,"sharedCutoffMs":cutoff,"sharedAnchorMs":anchor,
   "features":{"signedConfluence":x,"sessionFraction":0.1,"volCall":0.,"volPut":0.,"premCall":0.,"premPut":0.},"primaryReturnPct":y,
   "labels":[{"status":"LABELED","horizonMinutes":15,"startMs":anchor,"endMs":anchor+900_000,"priceCollectedMs":s.close_ms,"returnPct":y}]})}).collect();
        (
            json!({"version":"option-trial-shared-units-v1","generatedMs":now,"summary":{"auditErrors":0},"trialPolicy":policy,"units":units,"executionEnabled":false,"formulaChanged":false}),
            now,
        )
    }
    #[test]
    fn no_model_with_insufficient_dates() {
        let (d, n) = data(19);
        let r = train(&d, n).unwrap();
        assert_eq!(r["status"], "PENDING_TRAINING_SESSIONS");
        assert!(r["models"].is_null());
    }
    fn evaluation_data(n: usize) -> (Value, Value, Value, u64) {
        let (mut source, built) = data(20);
        source["trialPolicy"]["primaryMetric"] = json!("equal-session-mean-absolute-error");
        source["trialPolicy"]["uncertainty"] = json!({"method":"paired-session-percentile-bootstrap","replicates":10000,"seed":20261010,"primaryComparisons":3,"multiplicity":"Bonferroni","individualConfidence":0.9833333333333333});
        let frozen = train(&source, built).unwrap();
        let (mut dataset, now) = data(n);
        dataset["trialPolicy"] = source["trialPolicy"].clone();
        (dataset, frozen, source, now)
    }
    #[test]
    fn evaluation_analytic_mae_and_paired_bootstrap() {
        let (d, f, s, n) = evaluation_data(40);
        let r = evaluate(&d, &f, &s, n).unwrap();
        assert_eq!(r["status"], "EVALUATION_COMPLETE_RESEARCH_ONLY");
        assert!((r["metrics"]["equalSessionMAE"][0].as_f64().unwrap() - 1.).abs() < 1e-10);
        assert!((r["metrics"]["equalSessionMAE"][4].as_f64().unwrap() - 2.).abs() < 1e-10);
        assert_eq!(r["comparisons"][0]["lower"], 0.);
        assert_eq!(
            bootstrap(&[2.; 20]),
            (2.0000000000000004, 2.0000000000000004)
        );
        assert_eq!(bootstrap(&[-1., 2., 3., 0.]), bootstrap(&[-1., 2., 3., 0.]));
    }
    #[test]
    fn evaluation_waits_for_complete_block() {
        let (d, f, s, n) = evaluation_data(39);
        let r = evaluate(&d, &f, &s, n).unwrap();
        assert_eq!(r["status"], "PENDING_EVALUATION_SESSIONS");
        assert!(r["metrics"].is_null());
    }
    #[test]
    fn evaluation_accepts_serialized_confidence_rounding_only() {
        let (mut d, _, mut s, n) = evaluation_data(40);
        s["trialPolicy"]["uncertainty"]["individualConfidence"] = json!(0.9833333333333332);
        d["trialPolicy"] = s["trialPolicy"].clone();
        let f = train(&s, s["generatedMs"].as_u64().unwrap()).unwrap();
        assert!(evaluate(&d, &f, &s, n).is_ok());
        s["trialPolicy"]["uncertainty"]["individualConfidence"] = json!(0.95);
        d["trialPolicy"] = s["trialPolicy"].clone();
        let f = train(&s, s["generatedMs"].as_u64().unwrap()).unwrap();
        assert!(evaluate(&d, &f, &s, n).is_err());
    }
    #[test]
    fn evaluation_rejects_tampering_and_changed_training_units() {
        let (mut d, mut f, s, n) = evaluation_data(40);
        f["models"][0]["coefficients"][0] = json!(99.);
        assert!(evaluate(&d, &f, &s, n).is_err());
        f = train(&s, s["generatedMs"].as_u64().unwrap()).unwrap();
        d["units"][0]["features"]["signedConfluence"] = json!(0.);
        assert!(evaluate(&d, &f, &s, n).is_err());
        let late = train(&s, n).unwrap();
        assert!(evaluate(&d, &late, &s, n).is_err());
    }
    #[test]
    fn later_sessions_do_not_change_evaluation_block() {
        let (mut d, f, s, n) = evaluation_data(41);
        let first = evaluate(&d, &f, &s, n).unwrap();
        d["units"][40]["primaryReturnPct"] = json!(900.);
        d["units"][40]["labels"][0]["returnPct"] = json!(900.);
        let second = evaluate(&d, &f, &s, n).unwrap();
        assert_eq!(first["metrics"], second["metrics"]);
        assert_eq!(first["comparisons"], second["comparisons"]);
    }
    #[test]
    fn freeze_at_next_open_is_late_and_extreme_clocks_are_rejected() {
        let (mut d, n) = data(20);
        let deadline = train(&d, n).unwrap()["prospectiveFreezeDeadlineMs"]
            .as_u64()
            .unwrap();
        assert_eq!(
            train(&d, deadline).unwrap()["status"],
            "RETROSPECTIVE_DIAGNOSTIC_FIT"
        );
        d["units"][0]["windowStartMs"] = json!(u64::MAX);
        assert!(train(&d, n).is_err());
    }
    #[test]
    fn analytical_ridge_and_constant_feature_match() {
        let (d, n) = data(20);
        let r = train(&d, n).unwrap();
        assert_eq!(r["status"], "FROZEN_RESEARCH_MODELS");
        let m = &r["models"][0];
        assert!((m["intercept"].as_f64().unwrap() - 7.).abs() < 1e-10);
        assert!((m["coefficients"][0].as_f64().unwrap() - 1.).abs() < 1e-10);
        assert!(m["coefficients"][1].as_f64().unwrap().abs() < 1e-10);
    }
    #[test]
    fn evaluation_data_cannot_change_coefficients() {
        let (mut d, n) = data(21);
        let first = train(&d, n).unwrap();
        d["units"][20]["primaryReturnPct"] = json!(1000.);
        d["units"][20]["labels"][0]["returnPct"] = json!(1000.);
        d["units"][20]["features"]["signedConfluence"] = json!(0.);
        let second = train(&d, n).unwrap();
        assert_eq!(first["models"], second["models"]);
        assert_eq!(first["status"], "RETROSPECTIVE_DIAGNOSTIC_FIT");
    }
    #[test]
    fn duplicated_rows_within_one_day_do_not_overweight_it() {
        let (mut d, n) = data(20);
        let first = train(&d, n).unwrap();
        let mut r = d["units"][0].clone();
        let s = session(r["date"].as_str().unwrap()).unwrap();
        r["id"] = json!("additional-same-day");
        r["windowStartMs"] = json!(s.open_ms + 600_000);
        r["windowEndMs"] = json!(s.open_ms + 900_000);
        let cut = s.open_ms + 901_000;
        let a = (cut / 60_000 + 1) * 60_000;
        r["sharedCutoffMs"] = json!(cut);
        r["sharedAnchorMs"] = json!(a);
        r["labels"][0]["startMs"] = json!(a);
        r["labels"][0]["endMs"] = json!(a + 900_000);
        d["units"].as_array_mut().unwrap().push(r);
        let second = train(&d, n).unwrap();
        for i in 0..4 {
            assert!(
                (first["models"][i]["intercept"].as_f64().unwrap()
                    - second["models"][i]["intercept"].as_f64().unwrap())
                .abs()
                    < 1e-10
            );
            assert!(
                (first["models"][i]["coefficients"][0].as_f64().unwrap()
                    - second["models"][i]["coefficients"][0].as_f64().unwrap())
                .abs()
                    < 1e-10
            );
        }
    }
    #[test]
    fn duplicate_units_future_prices_and_policy_tuning_rejected() {
        for mode in 0..3 {
            let (mut d, n) = data(20);
            match mode {
                0 => {
                    let r = d["units"][0].clone();
                    d["units"].as_array_mut().unwrap().push(r);
                }
                1 => d["units"][0]["labels"][0]["priceCollectedMs"] = json!(n + 1),
                _ => d["trialPolicy"]["model"]["lambda"] = json!(0.),
            }
            assert!(train(&d, n).is_err());
        }
    }
}
