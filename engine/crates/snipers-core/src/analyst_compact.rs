//! Legacy narrative field allowlists. These functions cannot grant admission.
use serde_json::{json, Value};
use std::sync::OnceLock;
fn rules() -> &'static Value {
    static RULES: OnceLock<Value> = OnceLock::new();
    RULES.get_or_init(|| {
        serde_json::from_str(include_str!("../resources/analyst-compact-fields.json"))
            .expect("checked analyst field schema")
    })
}
fn pick(name: &str, index: usize, data: &Value) -> Value {
    let mut out = json!({});
    for key in rules()[name]["take"][index]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if let Some(v) = data.get(key) {
            out[key] = v.clone();
        }
    }
    out
}
fn direct(name: &str, data: &Value) -> Value {
    let mut out = json!({});
    for (key, source) in rules()[name]["direct"].as_object().into_iter().flatten() {
        out[key] = data[source.as_str().unwrap()].clone();
    }
    out
}
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64() != Some(0.0),
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}
fn first<'a>(items: &[&'a Value]) -> &'a Value {
    items
        .iter()
        .copied()
        .find(|v| truthy(v))
        .unwrap_or(&Value::Null)
}
fn sequence(data: &Value, limit: usize, contracts: bool) -> Value {
    Value::Array(
        data.as_array()
            .into_iter()
            .flatten()
            .take(limit)
            .filter(|v| !contracts || v.is_object())
            .map(|v| {
                if v.is_object() {
                    pick(
                        if contracts {
                            "_compact_contract_rows"
                        } else {
                            "_compact_sequence"
                        },
                        0,
                        v,
                    )
                } else {
                    v.clone()
                }
            })
            .collect(),
    )
}
fn nested(out: &mut Value, data: &Value, keys: &[&str], function: &str, objects_only: bool) {
    for key in keys {
        if let Some(v) = data.get(*key).filter(|v| !objects_only || v.is_object()) {
            out[*key] = compact_inner(function, v, false);
        }
    }
}
fn selected_levels(data: &Value) -> Value {
    let mut out = json!({});
    for key in rules()["_compact_levels"]["take"][0]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
    {
        if let Some(v) = data.get(key) {
            out[key] = compact_inner(
                if ["upside_vacuum", "downside_vacuum"].contains(&key) && v.is_object() {
                    "_compact_vacuum"
                } else {
                    "_compact_level_row"
                },
                v,
                false,
            );
        }
    }
    out
}
fn wall_live(data: &Value) -> Value {
    let name = "_compact_wall_live_flow";
    let mut out = pick(name, 0, data);
    if data["subscription"].is_object() {
        out["subscription"] = pick(name, 1, &data["subscription"]);
    }
    let mut windows = json!({});
    for label in ["1m", "5m", "15m"] {
        let window = &data["windows"][label];
        let mut row = json!({});
        for zone in ["call_zone", "put_zone"] {
            if window[zone].is_object() {
                row[zone] = pick(name, 2, &window[zone]);
            }
        }
        let mut levels = json!({});
        for key in [
            "CALL_WALL",
            "PUT_WALL",
            "CALL_PULL_TARGET",
            "PUT_PULL_TARGET",
        ] {
            if let Some(v) = window["levels"].get(key) {
                levels[key] = compact_inner("_compact_wall_live_level", v, false);
            }
        }
        if truthy(&levels) {
            row["levels"] = levels;
        }
        if truthy(&row) {
            windows[label] = row;
        }
    }
    if truthy(&windows) {
        out["windows"] = windows;
    }
    out
}
fn horizon(data: &Value, live: bool) -> Value {
    let name = "_compact_horizon_payload";
    let mut out = direct(name, data);
    out["expirations"] = sequence(&data["expirations"], 6, false);
    out["levels"] = selected_levels(&data["levels"]);
    for (field, function) in [
        ("wall_pull_decision", "_compact_wall_pull_decision"),
        ("position_map_confirmation", "_compact_position_map"),
        ("call_position_map", "_compact_position_map"),
        ("put_position_map", "_compact_position_map"),
        ("call_wall_detail", "_compact_level_row"),
        ("put_wall_detail", "_compact_level_row"),
        ("call_wall_context", "_compact_wall_context"),
        ("put_wall_context", "_compact_wall_context"),
        ("call_pull", "_compact_pull"),
        ("put_pull", "_compact_pull"),
        ("volume_context", "_compact_volume_context"),
        ("dark_pool_overlay", "_compact_dark_pool_overlay"),
    ] {
        out[field] = compact_inner(function, &data[field], false);
    }
    for field in ["upside_vacuum", "downside_vacuum", "primary_vacuum"] {
        out[field] = if data[field].is_object() {
            compact_inner("_compact_vacuum", &data[field], false)
        } else {
            data[field].clone()
        };
    }
    out["dex"] = pick(name, 0, &data["dex"]);
    if live {
        out["live_flow"] = if data["live_flow"].is_object() {
            compact_inner("_compact_flow_window", &data["live_flow"], false)
        } else {
            data["live_flow"].clone()
        };
        out["wall_live_flow"] = wall_live(&data["wall_live_flow"]);
    }
    out
}
fn structure(data: &Value) -> Value {
    let mut out = pick("_compact_structure_context", 0, data);
    for field in ["call_wall_detail", "put_wall_detail"] {
        out[field] = compact_inner("_compact_level_row", &data[field], false);
    }
    for field in ["call_pull", "put_pull"] {
        out[field] = compact_inner("_compact_pull", &data[field], false);
    }
    for field in ["upside_vacuum", "downside_vacuum", "primary_vacuum"] {
        out[field] = if data[field].is_object() {
            compact_inner("_compact_vacuum", &data[field], false)
        } else {
            data[field].clone()
        };
    }
    out
}
fn scanner_candidate(data: &Value) -> Value {
    let mut out = pick("_compact_scanner_candidate", 0, data);
    nested(
        &mut out,
        data,
        &["flow_1m", "flow_5m", "flow_15m", "flow_30m"],
        "_compact_flow_window",
        true,
    );
    for (field, function) in [
        ("live_option_liquidity", "_compact_live_liquidity"),
        ("wall_live_flow", "_compact_wall_live_flow"),
        ("structure_context", "_compact_structure_context"),
        ("position_map_confirmation", "_compact_position_map"),
        ("wall_pull_confirmation", "_compact_wall_pull_decision"),
    ] {
        out[field] = compact_inner(function, &data[field], false);
    }
    out["position_map_components"] = pick(
        "_compact_scanner_candidate",
        1,
        &data["confirmation_components"],
    );
    out
}
fn quick(data: &Value) -> Value {
    let name = "build_quick_read_payload";
    let mut out = direct(name, data);
    let liquidity = &data["liquidity"];
    out["expirations"] = sequence(&data["expirations"], 6, false);
    out["pressure"] = pick(name, 0, &data["pressure"]);
    out["quick_read_decision"] = compact_inner(
        "_compact_wall_pull_decision",
        &data["quick_read_decision"],
        false,
    );
    out["levels"] = selected_levels(&data["levels"]);
    for side in ["call", "put"] {
        let field = format!("{side}_wall");
        out[&field] = compact_inner(
            "_compact_level_row",
            first(&[&data[&field], &liquidity[&field]]),
            false,
        );
        let field = format!("{side}_wall_context");
        out[&field] = compact_inner(
            "_compact_wall_context",
            first(&[
                &data[&field],
                &liquidity[&field],
                &liquidity["wall_analysis"][side],
            ]),
            false,
        );
        let field = format!("{side}_pull");
        out[&field] = compact_inner(
            "_compact_pull",
            first(&[
                &data[&field],
                &liquidity[&field],
                &liquidity["pull_analysis"][side],
            ]),
            false,
        );
    }
    out["liquidity_summary"] = json!({"liquidity_magnet":compact_inner("_compact_level_row",&liquidity["liquidity_magnet"],false),"breakout_targets":pick(name,1,&liquidity["breakout_targets"]),"liquidity_vacuum":pick(name,2,&liquidity["liquidity_vacuum"]),"live_incoming_wall_volume_available":liquidity["live_incoming_wall_volume_available"],"liquidity_engine_build":liquidity["liquidity_engine_build"]});
    out["magnet_confirmation"] = compact_inner(
        "_compact_dark_pool_overlay",
        &data["magnet_confirmation"],
        false,
    );
    for field in ["top_calls", "top_puts"] {
        out[field] = sequence(&data[field], 4, true);
    }
    out
}
fn live_flow(data: &Value) -> Value {
    let name = "build_live_flow_payload";
    let mut out = direct(name, data);
    for label in ["1m", "5m", "15m", "30m"] {
        let field = format!("flow_{label}");
        out[&field] = compact_inner(
            "_compact_flow_window",
            first(&[&data[&field], &data["windows"][label]]),
            false,
        );
    }
    for (field, function) in [
        ("liquidity", "_compact_live_liquidity"),
        ("wall_live_flow", "_compact_wall_live_flow"),
        ("volume_context", "_compact_volume_context"),
    ] {
        out[field] = compact_inner(function, &data[field], false);
    }
    out["last_trade"] = pick(name, 0, &data["last_trade"]);
    out
}
fn scanner(data: &Value) -> Value {
    let name = "build_scanner_payload";
    let mut out = direct(name, data);
    out["top_5_watch"] = Value::Array(
        first(&[&data["top_5_watch"], &data["top_5"]])
            .as_array()
            .into_iter()
            .flatten()
            .take(5)
            .filter(|v| v.is_object())
            .map(scanner_candidate)
            .collect(),
    );
    out["top_5_frozen_symbols"] = sequence(&data["top_5_frozen_symbols"], 5, false);
    for (field, function) in [
        ("top_5_structure", "_compact_structure_context"),
        ("top_5_live_flow", "build_live_flow_payload"),
        ("top_5_options_levels", "_compact_options_level_record"),
    ] {
        let mut members = json!({});
        for (symbol, row) in data[field].as_object().into_iter().flatten().take(5) {
            members[symbol] = compact_inner(function, row, false);
        }
        out[field] = members;
    }
    out["summary"] = pick(name, 0, &data["summary"]);
    out
}
fn compact_inner(name: &str, data: &Value, live: bool) -> Value {
    match name {
        "_compact_sequence" => sequence(data, 5, false),
        "_compact_contract_rows" => sequence(data, 4, true),
        "_compact_level_row" => {
            if data.is_object() {
                pick(name, 0, data)
            } else {
                data.clone()
            }
        }
        "_compact_wall_context" | "_compact_pull" => {
            let mut out = pick(name, 0, data);
            nested(
                &mut out,
                data,
                if name == "_compact_pull" {
                    &["support", "target", "path", "vacuum"]
                } else {
                    &["wall", "below", "above"]
                },
                "_compact_level_row",
                false,
            );
            out
        }
        "_compact_position_map" => {
            let mut out = pick(name, 0, data);
            nested(
                &mut out,
                data,
                &["directional_vacuum", "primary_vacuum"],
                "_compact_vacuum",
                true,
            );
            out
        }
        "_compact_levels" => selected_levels(data),
        "_compact_wall_live_flow" => wall_live(data),
        "_compact_wall_pull_decision" => {
            let mut out = pick(name, 0, data);
            for field in ["call", "put", "call_scenario", "put_scenario"] {
                if data[field].is_object() {
                    out[field] = pick(name, 1, &data[field]);
                }
            }
            out
        }
        "_compact_horizon_payload" => horizon(data, live),
        "build_market_interpretation_compact" => {
            let mut out = direct(name, data);
            out["alignment"] = pick(name, 0, &data["alignment"]);
            for (field, source, live) in [
                ("day_or_next_session", "day", true),
                ("week", "week", false),
                ("four_weeks", "four_weeks", false),
            ] {
                out[field] = horizon(&data[source], live);
            }
            out["dark_pool_context"] = compact_inner(
                "_compact_dark_pool_overlay",
                &data["dark_pool_context"],
                false,
            );
            out
        }
        "build_quick_read_payload" => quick(data),
        "build_live_flow_payload" => live_flow(data),
        "build_gex_payload" => {
            let mut out = pick(name, 0, data);
            for field in [
                "dominant_calls",
                "dominant_puts",
                "top_positive_strikes",
                "top_negative_strikes",
            ] {
                if let Some(v) = data.get(field) {
                    out[field] = sequence(v, 5, false);
                }
            }
            out
        }
        "_compact_structure_context" => structure(data),
        "_compact_scanner_candidate" => scanner_candidate(data),
        "_compact_options_level_record" => {
            let mut out = direct(name, data);
            for field in ["main_options_magnet", "resistance_1", "support_1"] {
                out[field] = compact_inner("_compact_level_row", &data["data"][field], false);
            }
            out
        }
        "build_scanner_payload" => scanner(data),
        "_compact_massive_options" => {
            let mut out = direct(name, data);
            out["levels"] = selected_levels(&data["levels"]);
            out["top_strikes"] = sequence(&data["top_strikes"], 8, false);
            out
        }
        "_compact_decision_runtime" => {
            let mut out = direct(name, data);
            for (index, field) in ["decision", "session_status", "entry_guard", "paper_event"]
                .iter()
                .enumerate()
            {
                out[*field] = pick(name, index, &data[*field]);
            }
            out["position_map_confirmation"] = compact_inner(
                "_compact_position_map",
                &data["decision"]["position_map_confirmation"],
                false,
            );
            out["volume_context"] =
                compact_inner("_compact_volume_context", &data["volume_context"], false);
            out
        }
        "_compact_vwap_2m" => {
            let mut out = pick(name, 0, data);
            if let Some(v) = data["bars"].as_array().and_then(|v| v.last()) {
                out["latest_bar"] = pick(name, 1, v);
            }
            out
        }
        _ => pick(name, 0, data),
    }
}
/// Reject oversized or sensitive input before compaction, so dropping an unknown
/// field cannot disguise a misrouted configuration object. Top5 maps must already
/// contain at most five selected members; do not choose them by JSON key order.
pub fn compact(
    name: &str,
    data: &Value,
    include_live: bool,
) -> Result<Value, crate::analyst::Error> {
    crate::analyst::validate(data, 0)?;
    if rules().get(name).is_none() {
        return Err(crate::analyst::Error::Payload);
    }
    if serde_json::to_string(data)
        .map_err(|_| crate::analyst::Error::Payload)?
        .chars()
        .count()
        > crate::analyst::MAX_SNAPSHOT_CHARS
    {
        return Err(crate::analyst::Error::Budget);
    }
    for field in ["top_5_structure", "top_5_live_flow", "top_5_options_levels"] {
        if data[field].as_object().is_some_and(|v| v.len() > 5) {
            return Err(crate::analyst::Error::Budget);
        }
    }
    Ok(compact_inner(name, data, include_live))
}

/// Build the legacy section layout with an explicit caller timestamp. Unlike the
/// old final budget trimmer, this rejects excess data instead of removing gates.
pub fn system(data: &Value, generated_ms: u64) -> Result<Value, crate::analyst::Error> {
    crate::analyst::validate(data, 0)?;
    if serde_json::to_string(data)
        .map_err(|_| crate::analyst::Error::Payload)?
        .len()
        > 2_000_000
    {
        return Err(crate::analyst::Error::Budget);
    }
    let symbol = data["symbol"]
        .as_str()
        .filter(|s| crate::universe::stock_asset(s).is_some())
        .ok_or(crate::analyst::Error::Identity)?;
    let mut out = json!({"snapshot_build":"SNIPERS_RUST_COMPACT_SYSTEM_V1","symbol":symbol,"snapshot_generated_at_ms":generated_ms});
    for (field, alias, function) in [
        (
            "market_interpretation",
            "market_interpretation_data",
            "build_market_interpretation_compact",
        ),
        ("quick_read", "quick_read_data", "build_quick_read_payload"),
        ("live_flow", "live_flow_data", "build_live_flow_payload"),
        ("gex", "gex_data", "build_gex_payload"),
        ("gex_map", "gex_map_data", "build_gex_payload"),
        (
            "market_scanner",
            "scanner_snapshot",
            "build_scanner_payload",
        ),
        ("massive", "massive_data", "_compact_massive_options"),
        (
            "decision_runtime",
            "decision_runtime_data",
            "_compact_decision_runtime",
        ),
    ] {
        let input = first(&[&data[field], &data[alias]]);
        if input
            .get("symbol")
            .and_then(Value::as_str)
            .is_some_and(|s| s != symbol)
        {
            return Err(crate::analyst::Error::Identity);
        }
        out[if field == "massive" {
            "massive_options"
        } else {
            field
        }] = compact(function, input, false)?;
    }
    out["vwap_2m"] = compact(
        "_compact_vwap_2m",
        first(&[
            &data["vwap_2m"],
            &data["quick_vwap_snapshot"],
            &data["chart_context"],
        ]),
        false,
    )?;
    out["magnet_confirmation"] = compact(
        "_compact_magnet_confirmation",
        first(&[
            &data["magnet_confirmation"],
            &data["quick_read"]["magnet_confirmation"],
        ]),
        false,
    )?;
    out["source_status"] = json!({});
    for key in [
        "symbol",
        "market_open",
        "massive",
        "live_flow",
        "dark_pool",
        "gex",
        "scanner",
        "vwap",
        "last_update",
        "error",
    ] {
        if let Some(v) = data["source_status"].get(key) {
            out["source_status"][key] = v.clone();
        }
    }
    // Rust adds field-level clocks/coverage explicitly; never relabel a snapshot
    // consultation timestamp as the provider's own Greek or open-interest clock.
    out["data_quality"] = data["data_quality"].clone();
    if serde_json::to_string(&out)
        .map_err(|_| crate::analyst::Error::Payload)?
        .chars()
        .count()
        > crate::analyst::MAX_SNAPSHOT_CHARS
    {
        return Err(crate::analyst::Error::Budget);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_system_preserves_quality_and_rejects_identity_or_budget_loss() {
        let data = json!({"symbol":"QQQ","decision_runtime":{"entry_guard":{"allowed":false,"reason":"BLOCKED"}},"data_quality":{"snapshot_observed_ms":123,"greeks_provider_timestamp":null,"oi_semantics":"DAILY"}});
        let out = system(&data, 456).unwrap();
        assert_eq!(out["decision_runtime"]["entry_guard"]["allowed"], false);
        assert_eq!(out["data_quality"], data["data_quality"]);
        let mut bad = data.clone();
        bad["live_flow"] = json!({"symbol":"SPY"});
        assert_eq!(system(&bad, 456), Err(crate::analyst::Error::Identity));
        bad = data;
        bad["decision_runtime"]["entry_guard"]["reason"] = json!("x".repeat(85_001));
        assert_eq!(system(&bad, 456), Err(crate::analyst::Error::Budget));
    }
    #[test]
    fn legacy_compactors_preserve_fields_limits_and_gates() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../fixtures/legacy-analyst-compact.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let name = case["function"].as_str().unwrap();
            assert_eq!(
                compact_inner(
                    name,
                    &case["input"],
                    case["include_live"].as_bool().unwrap_or(false)
                ),
                case["expected"],
                "{name}"
            );
        }
    }
    #[test]
    fn input_boundaries_reject_secrets_and_ambiguous_members_instead_of_trimming() {
        assert!(matches!(
            compact(
                "build_scanner_payload",
                &json!({"ignored":{"api_key":"secret"}}),
                false
            ),
            Err(crate::analyst::Error::SensitiveField)
        ));
        assert!(matches!(
            compact(
                "build_scanner_payload",
                &json!({"top_5_structure":{"A":{},"B":{},"C":{},"D":{},"E":{},"F":{}}}),
                false
            ),
            Err(crate::analyst::Error::Budget)
        ));
        assert!(compact("UNKNOWN", &json!({}), false).is_err());
        let result=compact("_compact_decision_runtime",&json!({"decision":{"ready_for_paper_trade":false,"entry_confirmed":false,"reason":"BLOCKED"},"entry_guard":{"allowed":false,"reason":"SESSION_CLOSED"},"session_status":{"new_entries_enabled":false}}),false).unwrap();
        assert_eq!(result["entry_guard"]["allowed"], false);
        assert_eq!(result["decision"]["reason"], "BLOCKED");
    }
}
