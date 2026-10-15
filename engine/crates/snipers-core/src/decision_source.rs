//! Provenance check only. Passing this check never grants a paper entry.
//! JSON field names preserve the legacy runtime audit contract during migration.
use serde::Serialize;
use serde_json::Value;
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub accepted: bool,
    pub reason: String,
    pub entry_authorized: bool,
}
fn result(accepted: bool, reason: &str) -> Check {
    Check {
        accepted,
        reason: if accepted {
            reason.into()
        } else {
            format!("BLOQUEO DP: {reason}")
        },
        entry_authorized: false,
    }
}
fn num(v: &Value) -> Option<f64> {
    v.as_f64().filter(|v| v.is_finite())
}
fn label(v: &Value) -> String {
    v.as_str().unwrap_or("").trim().to_ascii_uppercase()
}
/// `structural_path_verified` must come from the validated options/underlying
/// coordinator, not from a UI flag or an imported legacy `ready` field alone.
pub fn check(reading: &Value, now: u64, structural_path_verified: bool) -> Check {
    let reject = |reason| result(false, reason);
    let Some(object) = reading.as_object() else {
        return reject("LECTURA INVALIDA");
    };
    let route = &reading["structural_path_setup"];
    if structural_path_verified
        && route["ready"] == true
        && matches!(label(&route["side"]).as_str(), "CALL" | "PUT")
    {
        return result(
            true,
            "STRUCTURAL PATH OPTIONS/UNDERLYING: DARK POOL NO REQUERIDO",
        );
    }
    if [
        "dark_pool_in_calculation",
        "dark_pool_context_applied",
        "dark_pool_adjustment_points",
        "dark_pool_score_adjustment_points",
        "confidence_before_dark_pool",
        "call_score_before_dark_pool",
        "put_score_before_dark_pool",
    ]
    .iter()
    .any(|k| !object.contains_key(*k))
    {
        return reject("FALTA TRAZABILIDAD DEL CALCULO");
    }
    let (Some(in_calc), Some(applied)) = (
        reading["dark_pool_in_calculation"].as_bool(),
        reading["dark_pool_context_applied"].as_bool(),
    ) else {
        return reject("ESTADO DE CALCULO AMBIGUO");
    };
    let (Some(confidence_delta), Some(score_delta)) = (
        num(&reading["dark_pool_adjustment_points"]),
        num(&reading["dark_pool_score_adjustment_points"]),
    ) else {
        return reject("AJUSTE SIN VALOR VERIFICABLE");
    };
    let active = reading["dark_pool_overlay"]["active"] == true;
    let used = in_calc
        || applied
        || active
        || confidence_delta.abs() > 0.0001
        || score_delta.abs() > 0.0001;
    if !used {
        for (final_key, base) in [
            ("confidence", "confidence_before_dark_pool"),
            ("call_score", "call_score_before_dark_pool"),
            ("put_score", "put_score_before_dark_pool"),
        ] {
            if num(&reading[final_key])
                .zip(num(&reading[base]))
                .is_none_or(|(a, b)| (a - b).abs() > 0.11)
            {
                return reject("SCORES NO COINCIDEN CON BASE DE OPCIONES");
            }
        }
        return result(true, "SOLO OPCIONES: DARK POOL SIN AJUSTE");
    }
    if !(in_calc && applied && active) {
        return reject("CONFIRMACION DARK POOL INCONSISTENTE");
    }
    let context = &reading["_sn_dp_entry_context"];
    if context.as_object().is_none_or(|o| o.is_empty()) {
        return reject("SIN CONTEXTO LIVE VERIFICABLE");
    }
    let symbol = label(&reading["symbol"]);
    if symbol.is_empty() || symbol == "UNKNOWN" || label(&context["symbol"]) != symbol {
        return reject("ACTIVO NO COINCIDE");
    }
    if context["dark_pool_history_integrity_reference"] != false
        || !(context["dark_pool_reference_date"].is_null()
            || context["dark_pool_reference_date"].as_str() == Some(""))
    {
        return reject("REFERENCIA HISTORICA");
    }
    // Fail closed on unknown states too; the legacy guard listed only four failures.
    if label(&context["dark_pool_history_integrity_state"]) != "READY" {
        return reject("HISTORIAL INCOMPLETO O NO VERIFICADO");
    }
    let count = num(&context["dark_pool_live_print_count"]);
    let timestamp = num(&context["dark_pool_last_live_timestamp"]);
    let magnet = num(&context["dark_pool_magnet"]);
    if count.is_none_or(|n| n < 1.0) || timestamp.is_none() || magnet.is_none_or(|n| n <= 0.0) {
        return reject("SIN PRINTS LIVE VERIFICABLES");
    }
    let seconds = timestamp.unwrap();
    if seconds < 0.0 || seconds * 1000.0 >= i64::MAX as f64 {
        return reject("HORA DE PRINT LIVE INVALIDA");
    }
    let event = (seconds * 1000.0).round() as u64;
    let Some(date) = crate::calendar::date_at(now) else {
        return reject("HORA DE PRINT LIVE INVALIDA");
    };
    if crate::calendar::date_at(event).is_none() {
        return reject("HORA DE PRINT LIVE INVALIDA");
    }
    if seconds > now as f64 / 1000.0
        || now as f64 / 1000.0 - seconds > 300.0
        || crate::calendar::date_at(event) != Some(date)
    {
        return reject("PRINT LIVE NO ES DE HOY O ESTA VENCIDO");
    }
    if context["dark_pool_history_integrity_date"]
        .as_str()
        .is_some_and(|s| !s.is_empty() && s != date.to_string())
    {
        return reject("HISTORIAL DE OTRA SESION");
    }
    result(true, "DARK POOL LIVE RECIENTE DEL MISMO ACTIVO")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_provenance_cases_and_unverified_route_cannot_grant_entry() {
        let fixtures: Value = serde_json::from_str(include_str!(
            "../../../fixtures/legacy-decision-source.json"
        ))
        .unwrap();
        let now = fixtures["nowMs"].as_u64().unwrap();
        for case in fixtures["cases"].as_array().unwrap() {
            let actual = check(&case["reading"], now, true);
            assert_eq!(
                actual.accepted,
                case["allowed"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
            assert_eq!(
                actual.reason,
                case["reason"].as_str().unwrap(),
                "{}",
                case["name"]
            );
            assert!(!actual.entry_authorized);
        }
        assert!(
            !check(
                &serde_json::json!({"structural_path_setup":{"ready":true,"side":"CALL"}}),
                now,
                false
            )
            .accepted
        );
        let mut unknown = fixtures["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "live_valid")
            .unwrap()["reading"]
            .clone();
        unknown["_sn_dp_entry_context"]["dark_pool_history_integrity_state"] = "UNKNOWN".into();
        assert!(!check(&unknown, now, false).accepted);
    }
}
