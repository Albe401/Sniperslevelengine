//! Structural/live confirmation, deliberately separate from entry authorization.
use crate::wall_flow::{Coverage, Window};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveWindow {
    pub minutes: u64,
    pub zones_covered: bool,
    pub pull_target_covered: bool,
    pub ready: bool,
}
fn zone(c: &Coverage) -> bool {
    c.levels_requested > 0
        && c.levels_covered > 0
        && (c.coverage_pct-100.0*c.levels_covered as f64/c.levels_requested as f64).abs()<=1e-9
        && c.levels_covered <= c.levels_requested
        && c.coverage_pct.is_finite()
        && (60.0..=100.0).contains(&c.coverage_pct)
        && c.contracts.is_some_and(|v| v.is_finite() && v >= 0.0)
}
pub fn summarize(w: &Window, side: &str) -> LiveWindow {
    let pull = if side == "CALL" {
        &w.call_pull
    } else {
        &w.put_pull
    };
    let zones_covered = matches!(side, "CALL" | "PUT") && zone(&w.call_zone) && zone(&w.put_zone);
    let pull_target_covered = matches!(side, "CALL" | "PUT")
        && pull.levels_requested == 1
        && pull.levels_covered == 1
        && pull.coverage_pct.is_finite() && (pull.coverage_pct-100.0).abs()<=1e-9
        && pull.contracts.is_some_and(|v| v.is_finite() && v >= 0.0);
    LiveWindow {
        minutes: w.minutes,
        zones_covered,
        pull_target_covered,
        ready: zones_covered && pull_target_covered,
    }
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub side: String,
    pub horizon: String,
    pub support_score: Option<f64>,
    pub support_strength: String,
    pub pull_score: Option<f64>,
    pub pull_strength: String,
    pub pull_ready: bool,
    pub support_context: bool,
    pub target_context: bool,
    pub position_ready: bool,
    pub flow_available: bool,
    pub flow_direction: String,
    pub bullish_pct: Option<f64>,
    pub bearish_pct: Option<f64>,
    pub live_1m: LiveWindow,
    pub live_5m: LiveWindow,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub state: String,
    pub ready: bool,
    pub reason: String,
    pub support_ready: bool,
    pub pull_ready: bool,
    pub static_ready: bool,
    pub flow_aligned: bool,
    pub live_required: bool,
    pub live_ready: bool,
    pub live_window_used: u64,
    pub entry_confirmed: bool,
}
fn strong(score: Option<f64>, strength: &str) -> bool {
    score.is_some_and(|v| v.is_finite() && (60.0..=100.0).contains(&v))
        && matches!(strength, "HIGH" | "EXTREME")
}
pub fn calculate(i: &Input) -> Read {
    let mut r = Read {
        state: "ESPERAR".into(),
        reason: "Dirección u horizonte no verificados".into(),
        live_required: i.horizon == "DAY",
        ..Default::default()
    };
    if !matches!(i.side.as_str(), "CALL" | "PUT")
        || !matches!(
            i.horizon.as_str(),
            "DAY" | "NEXT_SESSION" | "WEEK" | "NEXT4"
        )
    {
        return r;
    }
    r.support_ready = strong(i.support_score, &i.support_strength);
    r.pull_ready = i.pull_ready && strong(i.pull_score, &i.pull_strength);
    r.static_ready = r.support_ready
        && r.pull_ready
        && i.support_context
        && i.target_context
        && i.position_ready;
    let pct = if i.side == "CALL" {
        i.bullish_pct
    } else {
        i.bearish_pct
    };
    let direction = if i.side == "CALL" {
        "BULLISH FLOW"
    } else {
        "BEARISH FLOW"
    };
    r.flow_aligned = i.flow_available
        && (i.flow_direction == direction
            || pct.is_some_and(|v| v.is_finite() && (55.0..=100.0).contains(&v)));
    r.live_ready = i.live_1m.ready || i.live_5m.ready;
    r.live_window_used = if i.live_1m.ready {
        1
    } else if i.live_5m.ready {
        5
    } else if i.live_1m.zones_covered || i.live_1m.pull_target_covered {
        1
    } else {
        5
    };
    r.ready = r.static_ready && (!r.live_required || (r.flow_aligned && r.live_ready));
    r.reason = if !r.support_ready {
        "Soporte sin fuerza suficiente"
    } else if !r.pull_ready {
        "Pull sin fuerza suficiente"
    } else if !i.support_context || !i.target_context {
        "Falta contexto de las zonas"
    } else if !i.position_ready {
        "Mapa de posición no confirmado"
    } else if r.live_required && !r.flow_aligned {
        "Esperando flujo DAY alineado y observado"
    } else if r.live_required && !r.live_ready {
        "Esperando cobertura en muros, zonas y objetivo"
    } else if r.live_required {
        "Estructura, flujo y volumen de zonas confirmados"
    } else {
        "Contexto estructural favorable"
    }
    .into();
    if r.ready {
        r.state = if r.live_required {
            format!("LISTO PARA {}", i.side)
        } else {
            format!("ESTRUCTURA {} FAVORABLE", i.side)
        };
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_rules_match_seventy_eight_cases_without_entry_permission() {
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-wall-pull.json")).unwrap();
        for case in data["cases"].as_array().unwrap() {
            let input: Input = serde_json::from_value(case["input"].clone()).unwrap();
            let read = calculate(&input);
            let actual = serde_json::to_value(&read).unwrap();
            for (field, expected) in case["expected"].as_object().unwrap() {
                assert_eq!(
                    &actual[field], expected,
                    "field {field}, case {}",
                    case["input"]
                );
            }
            assert!(!read.entry_confirmed);
            if !input.support_context || !input.target_context {
                assert!(!read.ready);
                assert_eq!(read.reason, "Falta contexto de las zonas");
            }
        }
    }
    #[test]
    fn zone_thresholds_preserve_zero_and_reject_unknown_or_invalid() {
        let c = Coverage {
            levels_requested: 5,
            levels_covered: 3,
            coverage_pct: 60.0,
            contracts: Some(0.0),
            ..Default::default()
        };
        let pull = Coverage {
            levels_requested: 1,
            levels_covered: 1,
            coverage_pct: 100.0,
            contracts: Some(0.0),
            ..Default::default()
        };
        let mut w = Window {
            minutes: 1,
            call_zone: c.clone(),
            put_zone: c,
            call_pull: pull.clone(),
            put_pull: pull,
        };
        assert!(summarize(&w, "CALL").ready);
        assert!(summarize(&w, "PUT").ready);
        w.call_zone.coverage_pct = 59.99;
        assert!(!summarize(&w, "CALL").ready);
        w.call_zone.coverage_pct = 60.0;
        w.call_pull.contracts = None;
        assert!(!summarize(&w, "CALL").ready);
        assert!(!summarize(&w, "NEUTRAL").ready);
        w.call_zone.contracts = Some(f64::INFINITY);
        assert!(!summarize(&w, "PUT").ready);
    }
}
