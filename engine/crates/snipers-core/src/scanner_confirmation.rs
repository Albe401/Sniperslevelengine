//! Scanner confirmation components. These never grant paper/broker admission.
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FlowInput {
    direction: String,
    flow_direction: String,
    bullish_pct: Option<f64>,
    bearish_pct: Option<f64>,
    event_count: Option<f64>,
    premium: Option<f64>,
    connected: bool,
    authenticated: bool,
    subscribed: bool,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowRead {
    pub ready: bool,
    pub aligned: Option<bool>,
    pub reason: String,
    pub entry_confirmed: bool,
}
fn flow_rule(i: &FlowInput) -> FlowRead {
    let live = i.connected && i.authenticated && i.subscribed;
    let positive = |v: Option<f64>| v.is_some_and(|v| v.is_finite() && v > 0.0);
    let ready = live && positive(i.event_count) && positive(i.premium);
    let threshold =
        |v: Option<f64>| v.is_some_and(|v| v.is_finite() && (55.0..=100.0).contains(&v));
    let aligned = if ready {
        match i.direction.trim().to_uppercase().as_str() {
            "ALCISTA" => Some(
                i.flow_direction.trim().to_uppercase() == "BULLISH FLOW"
                    && threshold(i.bullish_pct),
            ),
            "BAJISTA" => Some(
                i.flow_direction.trim().to_uppercase() == "BEARISH FLOW"
                    && threshold(i.bearish_pct),
            ),
            _ => None,
        }
    } else {
        None
    };
    let reason = if !live {
        "FLOW LIVE CONECTANDO"
    } else if !ready {
        "ESPERANDO TRADES DE OPCIONES"
    } else {
        match aligned {
            Some(true) => "FLOW 1M ALINEADO",
            Some(false) => "FLOW 1M NO ALINEADO",
            None => "DIRECCION BASE NO VERIFICADA",
        }
    };
    FlowRead {
        ready,
        aligned,
        reason: reason.into(),
        entry_confirmed: false,
    }
}

pub struct FlowEvidence<'a> {
    pub symbol: &'a str,
    pub calculated_ms: u64,
    pub verified: bool,
    /// Must cover every selected contract for the whole one-minute interval.
    pub coverage_complete: bool,
    pub window: &'a crate::flow::Window,
}
pub fn flow(
    symbol: &str,
    direction: &str,
    evidence: Option<FlowEvidence<'_>>,
    now: u64,
) -> FlowRead {
    let Some(e) = evidence else {
        return FlowRead {
            reason: "ESPERANDO FLOW LIVE".into(),
            ..Default::default()
        };
    };
    let w = e.window;
    let valid = crate::universe::stock_asset(symbol).is_some()
        && e.symbol == symbol
        && e.calculated_ms <= now
        && now - e.calculated_ms <= 15_000
        && crate::calendar::date_at(now).is_some()
        && crate::calendar::date_at(e.calculated_ms) == crate::calendar::date_at(now)
        && e.coverage_complete
        && w.minutes == 1
        && w.premium_complete
        && [w.bullish_pct, w.bearish_pct]
            .iter()
            .all(|p| p.is_none_or(|p| p.is_finite() && (0.0..=100.0).contains(&p)));
    if !valid {
        return FlowRead {
            reason: "FLOW 1M SIN COBERTURA O IDENTIDAD VIGENTE".into(),
            ..Default::default()
        };
    }
    flow_rule(&FlowInput {
        direction: direction.into(),
        flow_direction: w.direction.clone(),
        bullish_pct: w.bullish_pct,
        bearish_pct: w.bearish_pct,
        event_count: Some(w.event_count as f64),
        premium: Some(w.premium),
        connected: e.verified,
        authenticated: e.verified,
        subscribed: e.verified,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Continuity {
    pub ok: bool,
    pub gap_ms: Option<i128>,
    pub reason: &'static str,
}
pub fn continuity(previous: Option<u64>, current: Option<u64>, now: u64) -> Continuity {
    let (Some(previous), Some(current)) = (previous, current) else {
        return Continuity {
            ok: false,
            gap_ms: None,
            reason: "MISSING_BAR_TIME",
        };
    };
    let gap = current as i128 - previous as i128;
    let reason = if current > now || previous > now {
        "FUTURE_BAR"
    } else if crate::calendar::date_at(now).is_none()
        || crate::calendar::date_at(current) != crate::calendar::date_at(now)
        || crate::calendar::date_at(previous) != crate::calendar::date_at(now)
    {
        "SESSION_CHANGED"
    } else if gap < 0 {
        "BAR_REGRESSED"
    } else if gap > 360_000 || now - current > 120_000 {
        "BAR_GAP_OR_STALE"
    } else {
        "CONTINUITY_OK"
    };
    Continuity {
        ok: reason == "CONTINUITY_OK",
        gap_ms: Some(gap),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_flow_gate_matches_576_cases() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-scanner-flow.json"))
                .unwrap();
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 576);
        for (n, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
            let i: FlowInput = serde_json::from_value(case["input"].clone()).unwrap();
            let r = flow_rule(&i);
            assert_eq!(
                serde_json::json!({"ready":r.ready,"aligned":r.aligned,"reason":r.reason}),
                case["expected"],
                "case {n}"
            );
        }
    }
    #[test]
    fn flow_context_rejects_other_assets_stale_future_incomplete_and_disconnected_sources() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut engine = crate::flow::Engine::default();
        let mut window = engine.snapshot(now).windows.remove(0);
        window.event_count = 1;
        window.premium = 100.0;
        window.premium_complete = true;
        window.direction = "BULLISH FLOW".into();
        window.bullish_pct = Some(70.0);
        window.bearish_pct = Some(30.0);
        let make = |symbol, ms, verified, coverage| FlowEvidence {
            symbol,
            calculated_ms: ms,
            verified,
            coverage_complete: coverage,
            window: &window,
        };
        assert_eq!(
            flow("QQQ", "ALCISTA", Some(make("QQQ", now, true, true)), now).aligned,
            Some(true)
        );
        for e in [
            make("SPY", now, true, true),
            make("QQQ", now - 15001, true, true),
            make("QQQ", now + 1, true, true),
            make("QQQ", now, false, true),
            make("QQQ", now, true, false),
        ] {
            let read = flow("QQQ", "ALCISTA", Some(e), now);
            assert!(!read.ready);
            assert!(!read.entry_confirmed);
        }
    }
    #[test]
    fn continuity_accepts_repeated_bar_but_rejects_gap_regression_stale_and_rollover() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        for gap in [0, 120_000, 360_000] {
            assert!(continuity(Some(now - gap), Some(now), now).ok);
        }
        for (old, new, clock) in [
            (now - 360001, now, now),
            (now, now - 1, now),
            (now, now + 1, now),
            (now, now, now + 120001),
            (now - 86_400_000, now, now),
        ] {
            assert!(!continuity(Some(old), Some(new), clock).ok);
        }
        assert!(!continuity(None, Some(now), now).ok);
    }
}
