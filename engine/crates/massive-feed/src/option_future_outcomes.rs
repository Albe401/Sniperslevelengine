//! Future labels isolated from detector inputs. Descriptive stock OHLC, never option P&L.
use crate::option_joint_audit;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use snipers_core::calendar::{self, Day};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
const MINUTE: u64 = 60_000;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceBar {
    pub timestamp_ms: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    pub version: String,
    pub registered_ms: u64,
    pub horizons_minutes: Vec<u64>,
    pub primary_horizon_minutes: u64,
    pub anchor: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceEvidence {
    pub version: String,
    pub underlying: String,
    pub date: String,
    pub adjusted: bool,
    pub collected_ms: u64,
    pub bars: Vec<PriceBar>,
}
fn validate_policy(p: &Policy, now: u64) -> Result<(), String> {
    if p.version != "option-future-outcome-policy-v1"
        || p.registered_ms == 0
        || p.registered_ms > now
        || p.horizons_minutes != [5, 15, 30]
        || p.primary_horizon_minutes != 15
        || p.anchor != "first-full-minute-open-after-reading"
    {
        return Err("Política de resultados incompatible o registrada en el futuro".into());
    }
    Ok(())
}
pub(crate) fn validate_prices(e: &PriceEvidence, now: u64) -> Result<BTreeMap<u64, &PriceBar>, String> {
    let day = calendar::day_bounds(&e.date)
        .and_then(|(t, _)| calendar::date_at(t))
        .ok_or("Fecha de precios inválida")?;
    let Day::Open(session) = calendar::schedule(day) else {
        return Err("Precios fuera de sesión".into());
    };
    if e.version != "stock-context-evidence-v1"
        || e.adjusted
        || e.collected_ms > now
        || e.underlying.is_empty()
        || e.bars.len() > 390
    {
        return Err("Fuente de precios incompatible".into());
    }
    let mut bars = BTreeMap::new();
    for b in &e.bars {
        if ![b.open, b.high, b.low, b.close, b.volume]
            .iter()
            .all(|n| n.is_finite())
            || b.open <= 0.
            || b.low <= 0.
            || b.close <= 0.
            || b.high < b.open.max(b.close)
            || b.low > b.open.min(b.close)
            || b.high < b.low
            || b.volume < 0.
            || !b.timestamp_ms.is_multiple_of(MINUTE)
            || b.timestamp_ms < session.open_ms
            || b.timestamp_ms
                .checked_add(MINUTE)
                .is_none_or(|t| t > session.close_ms || t > e.collected_ms)
            || bars.insert(b.timestamp_ms, b).is_some()
        {
            return Err("Minuto duplicado, futuro o inválido".into());
        }
    }
    Ok(bars)
}
pub(crate) fn label(
    symbol: &str,
    date: &str,
    available: u64,
    horizon: u64,
    e: Option<&PriceEvidence>,
    now: u64,
) -> Value {
    let Some(start) = available
        .checked_div(MINUTE)
        .and_then(|t| t.checked_add(1))
        .and_then(|t| t.checked_mul(MINUTE))
    else {
        return json!({"status":"INVALID","reason":"clock-overflow"});
    };
    let Some(end) = start.checked_add(horizon * MINUTE) else {
        return json!({"status":"INVALID","reason":"clock-overflow"});
    };
    let mut out = json!({"horizonMinutes":horizon,"startMs":start,"endMs":end,"status":"PENDING",
        "returnPct":null,"upsideRangePct":null,"downsideRangePct":null,"anchorPrice":null,
        "endpointPrice":null,"reason":null,"priceCollectedMs":e.map(|e|e.collected_ms)});
    let Some(Day::Open(session)) = calendar::day_bounds(date)
        .and_then(|(t, _)| calendar::date_at(t))
        .map(calendar::schedule)
    else {
        out["status"] = json!("INVALID");
        out["reason"] = json!("invalid-session");
        return out;
    };
    if start < session.open_ms || end > session.close_ms {
        out["status"] = json!("EXCLUDED_SESSION_BOUNDARY");
        return out;
    }
    if end > now {
        out["reason"] = json!("horizon-in-progress");
        return out;
    }
    let Some(e) = e else {
        out["reason"] = json!("missing-price-evidence");
        return out;
    };
    if e.underlying != symbol || e.date != date {
        out["status"] = json!("INVALID");
        out["reason"] = json!("foreign-price-evidence");
        return out;
    }
    let bars = match validate_prices(e, now) {
        Ok(b) => b,
        Err(reason) => {
            out["status"] = json!("INVALID");
            out["reason"] = json!(reason);
            return out;
        }
    };
    if e.collected_ms < end {
        out["reason"] = json!("price-evidence-before-horizon");
        return out;
    }
    let Some(path): Option<Vec<_>> = (start..end)
        .step_by(MINUTE as usize)
        .map(|t| bars.get(&t).copied())
        .collect()
    else {
        out["reason"] = json!("missing-minute-not-filled");
        return out;
    };
    let anchor = path[0].open;
    let endpoint = path.last().unwrap().close;
    let high = path.iter().map(|b| b.high).fold(anchor, f64::max);
    let low = path.iter().map(|b| b.low).fold(anchor, f64::min);
    let values = [
        100. * (endpoint / anchor - 1.),
        100. * (high / anchor - 1.),
        100. * (low / anchor - 1.),
    ];
    if !values.iter().all(|v| v.is_finite()) {
        out["status"] = json!("INVALID");
        out["reason"] = json!("non-finite-result");
        return out;
    }
    out["status"] = json!("LABELED");
    out["returnPct"] = json!(values[0]);
    out["upsideRangePct"] = json!(values[1]);
    out["downsideRangePct"] = json!(values[2]);
    out["anchorPrice"] = json!(anchor);
    out["endpointPrice"] = json!(endpoint);
    out
}

/// Re-audit original capture directories. No trusted retrospective signal file as input.
pub fn build(
    volume_dirs: &[PathBuf],
    premium_dirs: &[PathBuf],
    prices: &[PriceEvidence],
    policy: &Policy,
    now: u64,
) -> Result<Value, String> {
    validate_policy(policy, now)?;
    let joint = option_joint_audit::report(volume_dirs, premium_dirs, now)?;
    let mut price_map = BTreeMap::new();
    for e in prices {
        validate_prices(e, now)?;
        if price_map
            .insert((e.underlying.clone(), e.date.clone()), e)
            .is_some()
        {
            return Err("Precio duplicado por símbolo y fecha".into());
        }
    }
    let mut rows = vec![];
    let mut counts = BTreeMap::<String, usize>::new();
    let mut seen = BTreeSet::new();
    for row in joint["reconciliation"]
        .as_array()
        .ok_or("Conciliación ausente")?
    {
        for motor in ["volume", "premium"] {
            let reading = &row[motor];
            if reading.is_null() {
                continue;
            }
            let available = reading["readingAvailableMs"]
                .as_u64()
                .ok_or("Lectura sin reloj de disponibilidad")?;
            let symbol = row["underlying"].as_str().ok_or("Símbolo ausente")?;
            let date = row["date"].as_str().ok_or("Fecha ausente")?;
            let ticker = row["ticker"].as_str().ok_or("Ticker ausente")?;
            let start = row["startMs"].as_u64().ok_or("Inicio ausente")?;
            let id = format!("{motor}:{symbol}:{date}:{ticker}:{start}");
            if !seen.insert(id.clone()) {
                return Err("Caso duplicado".into());
            }
            if policy.registered_ms >= available {
                return Err("Política posterior a lectura: no es ensayo pre-registrado".into());
            }
            let e = price_map.get(&(symbol.into(), date.into())).copied();
            let labels: Vec<_> = policy
                .horizons_minutes
                .iter()
                .map(|h| label(symbol, date, available, *h, e, now))
                .collect();
            for l in &labels {
                *counts
                    .entry(l["status"].as_str().unwrap_or("INVALID").into())
                    .or_default() += 1;
            }
            // Features are copied from the audited observation; no future price enters this object.
            rows.push(
                json!({"id":id,"motor":motor,"underlying":symbol,"date":date,"ticker":ticker,
                "windowStartMs":start,"windowEndMs":row["endMs"],"readingAvailableMs":available,
                "features":reading,"labels":labels,"baselineForecast":null}),
            );
        }
    }
    let errors = joint["summary"]["auditErrors"].as_u64().unwrap_or(0);
    Ok(
        json!({"version":"option-future-outcome-dataset-v1","generatedMs":now,"policy":policy,
        "status":if errors>0 {"AUDIT_ERRORS"} else if rows.is_empty(){"WAITING_VALID_CAPTURE"}else{"DESCRIPTIVE_OUTCOMES_ONLY"},
        "cases":rows,"labelStatusCounts":counts,"jointAudit":joint,
        "limits":["Next full minute open is an ex-post research anchor, not a guaranteed executable price.",
            "All horizon minutes must exist and be closed; no interpolation or cross-session extension.",
            "Stock price changes are not option returns, directional predictions or net trading profit.",
            "Multiple contracts and overlapping horizons share outcomes; cases are not independent sessions.",
            "Current-formula forecasts are absent; incremental predictive benefit cannot yet be measured.",
            "Archived clocks establish consistency, not signed provenance."],
        "incrementalValidationImplemented":false,"calibrated":false,"formulaChanged":false,"executionEnabled":false,"ordersSent":0}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn example_policy(at: u64) -> Policy {
        Policy {
            version: "option-future-outcome-policy-v1".into(),
            registered_ms: at,
            horizons_minutes: vec![5, 15, 30],
            primary_horizon_minutes: 15,
            anchor: "first-full-minute-open-after-reading".into(),
        }
    }
    #[test]
    fn audited_dataset_keeps_features_separate_from_future_prices() {
        let (a, e) = fixture();
        let root = std::env::temp_dir().join(format!(
            "outcomes-features-{}-{}",
            std::process::id(),
            crate::live::now_ms()
        ));
        let v = option_joint_audit::tests::write_fixture(&root, false);
        let p = option_joint_audit::tests::write_fixture(&root, true);
        let policy = example_policy(a - MINUTE);
        let first = build(
            &[v.clone()],
            &[p.clone()],
            &[e.clone()],
            &policy,
            e.collected_ms,
        )
        .unwrap();
        assert_eq!(first["cases"].as_array().unwrap().len(), 12);
        assert_eq!(first["labelStatusCounts"]["LABELED"], 36);
        let mut changed = e.clone();
        for b in &mut changed.bars {
            b.close = 100.5;
        }
        let second = build(
            &[v.clone()],
            &[p.clone()],
            &[changed],
            &policy,
            e.collected_ms,
        )
        .unwrap();
        assert_eq!(
            first["cases"][0]["features"],
            second["cases"][0]["features"]
        );
        assert_ne!(first["cases"][0]["labels"], second["cases"][0]["labels"]);
        assert!(first["cases"][0]["baselineForecast"].is_null());
        assert!(build(
            &[v.clone()],
            &[p.clone()],
            &[e.clone()],
            &example_policy(a + MINUTE),
            e.collected_ms
        )
        .is_err());
        assert!(build(&[v], &[p], &[e.clone(), e.clone()], &policy, e.collected_ms).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn empty_capture_cannot_create_cases_or_claim_accuracy() {
        let dir = std::env::temp_dir().join(format!("missing-outcomes-{}", crate::live::now_ms()));
        let now = crate::live::now_ms();
        let r = build(&[dir.clone()], &[dir], &[], &example_policy(now - 1), now).unwrap();
        assert_eq!(r["status"], "WAITING_VALID_CAPTURE");
        assert!(r["cases"].as_array().unwrap().is_empty());
        assert_eq!(r["incrementalValidationImplemented"], false);
    }
    fn fixture() -> (u64, PriceEvidence) {
        let available = calendar::legacy_eastern_ms("2026-10-12 09:40:03").unwrap();
        let start = calendar::legacy_eastern_ms("2026-10-12 09:41:00").unwrap();
        let bars = (0..30)
            .map(|i| PriceBar {
                timestamp_ms: start + i * MINUTE,
                open: 100.,
                high: 102.,
                low: 99.,
                close: 101.,
                volume: 10.,
            })
            .collect();
        (
            available,
            PriceEvidence {
                version: "stock-context-evidence-v1".into(),
                underlying: "QQQ".into(),
                date: "2026-10-12".into(),
                adjusted: false,
                collected_ms: start + 30 * MINUTE,
                bars,
            },
        )
    }
    #[test]
    fn labels_all_horizons_after_actual_availability() {
        let (a, e) = fixture();
        for h in [5, 15, 30] {
            let l = label("QQQ", &e.date, a, h, Some(&e), e.collected_ms);
            assert_eq!(l["status"], "LABELED");
            assert!(l["startMs"].as_u64().unwrap() > a);
            assert!((l["returnPct"].as_f64().unwrap() - 1.).abs() < 1e-9);
        }
    }
    #[test]
    fn future_results_remain_hidden() {
        let (a, e) = fixture();
        let l = label("QQQ", &e.date, a, 30, Some(&e), a + MINUTE);
        assert_eq!(l["reason"], "horizon-in-progress");
        assert!(l["returnPct"].is_null());
        assert!(l["anchorPrice"].is_null());
    }
    #[test]
    fn missing_minute_never_becomes_zero() {
        let (a, mut e) = fixture();
        e.bars.remove(2);
        let l = label("QQQ", &e.date, a, 15, Some(&e), e.collected_ms);
        assert_eq!(l["reason"], "missing-minute-not-filled");
        assert!(l["returnPct"].is_null());
    }
    #[test]
    fn invalid_source_duplicate_and_future_bars_rejected() {
        for mode in 0..4 {
            let (_, mut e) = fixture();
            match mode {
                0 => e.adjusted = true,
                1 => e.bars.push(e.bars[0].clone()),
                2 => e.bars[0].low = 200.,
                _ => e.collected_ms -= MINUTE,
            }
            assert!(validate_prices(&e, e.collected_ms).is_err());
        }
    }
    #[test]
    fn close_boundary_not_carried_to_tomorrow() {
        let (a, e) = fixture();
        let a = a + 6 * 60 * MINUTE;
        let l = label("QQQ", &e.date, a, 30, None, a + 60 * MINUTE);
        assert_eq!(l["status"], "EXCLUDED_SESSION_BOUNDARY");
    }
    #[test]
    fn missing_evidence_and_wrong_symbol_not_labeled() {
        let (a, e) = fixture();
        assert_eq!(
            label("QQQ", &e.date, a, 5, None, e.collected_ms)["reason"],
            "missing-price-evidence"
        );
        assert_eq!(
            label("SPY", &e.date, a, 5, Some(&e), e.collected_ms)["status"],
            "INVALID"
        );
    }
}
