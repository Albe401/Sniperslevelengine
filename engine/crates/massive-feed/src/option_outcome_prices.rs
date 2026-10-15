//! One-shot post-session stock evidence for previously audited option observations.
use crate::{
    credentials::Credentials,
    history::checked_url,
    live::now_ms,
    option_activity_dataset::save_json,
    option_activity_history,
    option_future_outcomes::{self, Policy, PriceBar, PriceEvidence},
};
use serde_json::{json, Value};
use snipers_core::calendar::{self, Day, Session};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::Duration,
};

fn session(date: &str) -> Result<Session, String> {
    match calendar::day_bounds(date)
        .and_then(|(t, _)| calendar::date_at(t))
        .map(calendar::schedule)
    {
        Some(Day::Open(s)) => Ok(s),
        _ => Err("Fecha sin sesión regular válida".into()),
    }
}
fn sessions_needed(joint: &Value, now: u64) -> Result<Vec<(String, Session, bool)>, String> {
    let mut keys = BTreeSet::new();
    for row in joint["reconciliation"]
        .as_array()
        .ok_or("Conciliación ausente")?
    {
        keys.insert((
            row["underlying"]
                .as_str()
                .ok_or("Símbolo ausente")?
                .to_owned(),
            row["date"].as_str().ok_or("Fecha ausente")?.to_owned(),
        ));
    }
    if keys.len() > 60 {
        return Err("Máximo 60 sesiones de precios por ejecución".into());
    }
    keys.into_iter()
        .map(|(symbol, date)| {
            if snipers_core::universe::stock_asset(&symbol).is_none() {
                return Err("Símbolo no habilitado".into());
            }
            let s = session(&date)?;
            let ready = now >= s.close_ms;
            Ok((symbol, s, ready))
        })
        .collect()
}
fn parse_pages(
    symbol: &str,
    s: &Session,
    pages: &[Value],
    collected: u64,
) -> Result<(PriceEvidence, usize), String> {
    if pages.is_empty() || pages.len() > 4 || collected < s.close_ms {
        return Err("Paginación vacía o sesión sin cerrar".into());
    }
    let mut bars = BTreeMap::<u64, PriceBar>::new();
    let mut duplicates = 0;
    for (i, page) in pages.iter().enumerate() {
        if page["status"] != "OK" || page["ticker"] != symbol || page["adjusted"] != false {
            return Err("Fuente de precios no confirmada o ajustada".into());
        }
        let terminal = page["next_url"].is_null() || page["next_url"] == "";
        if terminal != (i + 1 == pages.len()) {
            return Err("Paginación incompleta o terminal anticipado".into());
        }
        let rows = page["results"]
            .as_array()
            .ok_or("Resultados de precios ausentes")?;
        if rows.len() > 390 {
            return Err("Demasiados minutos por página".into());
        }
        for row in rows {
            let t = row["t"].as_u64().ok_or("Timestamp de precio inválido")?;
            let [Some(o), Some(h), Some(l), Some(c), Some(v)] =
                ["o", "h", "l", "c", "v"].map(|k| row[k].as_f64())
            else {
                return Err("OHLC o volumen incompleto".into());
            };
            let b = PriceBar {
                timestamp_ms: t,
                open: o,
                high: h,
                low: l,
                close: c,
                volume: v,
            };
            if let Some(old) = bars.get(&t) {
                if [old.open, old.high, old.low, old.close, old.volume] != [o, h, l, c, v] {
                    return Err("Minutos duplicados en conflicto".into());
                }
                duplicates += 1;
                continue;
            }
            bars.insert(t, b);
        }
    }
    let evidence = PriceEvidence {
        version: "stock-context-evidence-v1".into(),
        underlying: symbol.into(),
        date: s.date.clone(),
        adjusted: false,
        collected_ms: collected,
        bars: bars.into_values().collect(),
    };
    option_future_outcomes::validate_prices(&evidence, collected)?;
    Ok((evidence, duplicates))
}
async fn fetch(
    key: &Credentials,
    symbol: &str,
    s: &Session,
) -> Result<(PriceEvidence, Value), String> {
    let path = format!(
        "/v2/aggs/ticker/{symbol}/range/1/minute/{}/{}",
        s.open_ms,
        s.close_ms - 1
    );
    let mut url = checked_url(
        &format!("https://api.massive.com{path}?adjusted=false&sort=asc&limit=50000"),
        &path,
    )
    .map_err(str::to_owned)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "REST no disponible")?;
    let started = now_ms();
    let mut visited = BTreeSet::new();
    let mut pages = vec![];
    let mut requests = vec![];
    for _ in 0..4 {
        if !visited.insert(url.to_string()) {
            return Err("Cursor repetido".into());
        }
        requests.push(url.to_string());
        let mut page = option_activity_history::get(&client, key, url).await?;
        let next = match &page["next_url"] {
            Value::Null => None,
            Value::String(v) if v.is_empty() => None,
            Value::String(v) => Some(checked_url(v, &path).map_err(str::to_owned)?),
            _ => return Err("Cursor inválido".into()),
        };
        // Archive the sanitized cursor, never a query-string API key.
        if let Some(next) = &next {
            page["next_url"] = json!(next.as_str());
        }
        pages.push(page);
        if let Some(next) = next {
            url = next;
        } else {
            let collected = now_ms();
            let (evidence, duplicates) = parse_pages(symbol, s, &pages, collected)?;
            return Ok((
                evidence,
                json!({"version":"option-outcome-price-source-v1","underlying":symbol,"date":s.date,
                "session":s,"requestStartedMs":started,"collectedMs":collected,"adjusted":false,"requests":requests,
                "retrievalComplete":true,"identicalDuplicates":duplicates,"pages":pages}),
            ));
        }
    }
    Err("Paginación incompleta: no se guardan precios utilizables".into())
}
/// Audits first; credentials are loaded only when a certified session has closed.
pub async fn collect(
    volume: &[PathBuf],
    premium: &[PathBuf],
    policy_path: &Path,
    output: &Path,
) -> Result<Value, String> {
    if std::fs::metadata(policy_path)
        .map_err(|_| "Política ausente")?
        .len()
        > 64_000
    {
        return Err("Política demasiado grande".into());
    }
    let policy: Policy =
        serde_json::from_slice(&std::fs::read(policy_path).map_err(|_| "Política inaccesible")?)
            .map_err(|_| "Política inválida")?;
    let now = now_ms();
    // Also validates the registered policy and its availability against every certified reading.
    let initial = option_future_outcomes::build(volume, premium, &[], &policy, now)?;
    let joint = &initial["jointAudit"];
    let plan = sessions_needed(joint, now)?;
    let ready = plan.iter().any(|(_, _, r)| *r);
    let credentials = if ready {
        Some(Credentials::load().map_err(str::to_owned)?)
    } else {
        None
    };
    let mut results = vec![];
    let mut files = vec![];
    let mut failures = 0;
    for (symbol, s, ready) in plan {
        if !ready {
            results
                .push(json!({"underlying":symbol,"date":s.date,"status":"WAITING_SESSION_CLOSE"}));
            continue;
        }
        match fetch(credentials.as_ref().ok_or("Falta credencial")?, &symbol, &s).await {
            Ok((evidence, source)) => {
                let raw = save_json(
                    output,
                    &format!("stock-source-{symbol}-{}", s.date),
                    &source,
                )?;
                let mut value =
                    serde_json::to_value(&evidence).map_err(|_| "Evidencia no serializable")?;
                value["sourceFile"] = json!(raw);
                value["session"] = json!(s);
                value["retrievalComplete"] = json!(true);
                let expected = (s.close_ms - s.open_ms) / 60_000;
                let missing = expected - evidence.bars.len() as u64;
                let file = save_json(output, &format!("stock-prices-{symbol}-{}", s.date), &value)?;
                files.push(file.clone());
                results.push(json!({"underlying":symbol,"date":s.date,"status":if missing==0{"PRICES_ARCHIVED"}else{"PARTIAL_MINUTE_COVERAGE"},
                    "observedMinutes":evidence.bars.len(),"expectedSessionMinutes":expected,"missingMinutes":missing,"file":file,"sourceFile":raw}));
            }
            Err(reason) => {
                failures += 1;
                results.push(json!({"underlying":symbol,"date":s.date,"status":"PENDING_SOURCE","reason":reason}));
            }
        }
    }
    let manifest = save_json(
        output,
        "outcome-price-manifest",
        &json!({"volumeDirectories":volume,"premiumDirectories":premium,
        "policyFile":policy_path,"priceEvidenceFiles":files}),
    )?;
    let summary = json!({"version":"option-outcome-price-collection-v1","generatedMs":now_ms(),
        "status":if joint["summary"]["auditErrors"].as_u64().unwrap_or(0)>0{"AUDIT_ERRORS"}else if failures>0{"PENDING_SOURCE"}else if joint["reconciliation"].as_array().is_some_and(Vec::is_empty){"WAITING_VALID_CAPTURE"}else if files.is_empty(){"WAITING_SESSION_CLOSE"}else{"PRICE_EVIDENCE_ARCHIVED"},
        "archivedPriceFiles":files.len(),"sourceFailures":failures,"credentialsLoaded":ready,"sessions":results,
        "jointAuditStatus":joint["status"],"jointAuditErrors":joint["summary"]["auditErrors"],"outcomeManifestFile":manifest,
        "calibrated":false,"formulaChanged":false,"executionEnabled":false,"ordersSent":0});
    let audit = save_json(output, "outcome-price-collection-audit", &summary)?;
    Ok(json!({"summary":summary,"auditFile":audit}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn no_certified_capture_does_not_load_credentials_or_fetch_prices() {
        let root = std::env::temp_dir().join(format!(
            "price-collector-empty-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir(&root).unwrap();
        let policy = root.join("policy.json");
        std::fs::write(
            &policy,
            serde_json::to_vec(&json!({"version":"option-future-outcome-policy-v1",
            "registeredMs":now_ms()-1,"horizonsMinutes":[5,15,30],"primaryHorizonMinutes":15,
            "anchor":"first-full-minute-open-after-reading"}))
            .unwrap(),
        )
        .unwrap();
        let missing = root.join("no-capture");
        let output = root.join("output");
        let r = collect(&[missing.clone()], &[missing], &policy, &output)
            .await
            .unwrap();
        assert_eq!(r["summary"]["status"], "WAITING_VALID_CAPTURE");
        assert_eq!(r["summary"]["credentialsLoaded"], false);
        assert_eq!(r["summary"]["archivedPriceFiles"], 0);
        let manifest: Value = serde_json::from_slice(
            &std::fs::read(r["summary"]["outcomeManifestFile"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        assert!(manifest["priceEvidenceFiles"]
            .as_array()
            .unwrap()
            .is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn complete_multi_page_evidence_is_compatible_with_outcome_schema() {
        let (s, mut first) = fixture();
        let mut second = first.clone();
        first["next_url"] = json!("sanitized-cursor");
        second["results"][0]["t"] = json!(s.open_ms + 60_000);
        let (e, _) = parse_pages("QQQ", &s, &[first, second], s.close_ms).unwrap();
        let replay: PriceEvidence =
            serde_json::from_slice(&serde_json::to_vec(&e).unwrap()).unwrap();
        assert_eq!(replay.bars.len(), 2);
        assert!(option_future_outcomes::validate_prices(&replay, s.close_ms).is_ok());
    }
    fn fixture() -> (Session, Value) {
        let s = session("2026-10-12").unwrap();
        let p = json!({"status":"OK","ticker":"QQQ","adjusted":false,
        "results":[{"t":s.open_ms,"o":100.,"h":101.,"l":99.,"c":100.,"v":10.}]});
        (s, p)
    }
    #[test]
    fn missing_minutes_are_archived_as_missing_not_zero() {
        let (s, p) = fixture();
        let (e, _) = parse_pages("QQQ", &s, &[p], s.close_ms).unwrap();
        assert_eq!(e.bars.len(), 1);
        assert!(option_future_outcomes::validate_prices(&e, s.close_ms).is_ok());
    }
    #[test]
    fn duplicates_equal_deduplicated_but_conflicts_rejected() {
        let (s, mut p) = fixture();
        let r = p["results"][0].clone();
        p["results"].as_array_mut().unwrap().push(r);
        assert_eq!(
            parse_pages("QQQ", &s, &[p.clone()], s.close_ms).unwrap().1,
            1
        );
        p["results"][1]["v"] = json!(11.);
        assert!(parse_pages("QQQ", &s, &[p], s.close_ms).is_err());
    }
    #[test]
    fn malformed_source_or_incomplete_pagination_rejected() {
        for mode in 0..5 {
            let (s, mut p) = fixture();
            match mode {
                0 => p["adjusted"] = json!(true),
                1 => p["ticker"] = json!("SPY"),
                2 => p["next_url"] = json!("cursor"),
                3 => p["results"][0]["t"] = json!(s.close_ms),
                _ => p["results"][0]["l"] = json!(999.),
            }
            assert!(parse_pages("QQQ", &s, &[p], s.close_ms).is_err());
        }
    }
    #[test]
    fn collection_plan_deduplicates_contracts_and_waits_for_close() {
        let (s, _) = fixture();
        let j = json!({"reconciliation":[{"underlying":"QQQ","date":s.date},{"underlying":"QQQ","date":s.date}]});
        let plan = sessions_needed(&j, s.close_ms - 1).unwrap();
        assert_eq!(plan.len(), 1);
        assert!(!plan[0].2);
        assert!(sessions_needed(&j, s.close_ms).unwrap()[0].2);
    }
    #[test]
    fn closed_market_and_unknown_sessions_do_not_invent_targets() {
        assert!(session("2026-10-10").is_err());
        assert!(sessions_needed(&json!({"reconciliation":[]}), now_ms())
            .unwrap()
            .is_empty());
    }
}
