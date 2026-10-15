//! The scanner always requires live wall coverage, including its NEXT4 horizon.
//! This differs from the general interpretation panel's horizon-dependent policy.
use crate::{
    calendar, flow::ContractWindow, horizons::Horizon, liquidity, position_map, wall_flow,
    wall_pull,
};
use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallRead {
    pub ready: bool,
    pub support_ready: bool,
    pub pull_ready: bool,
    pub static_ready: bool,
    pub live_ready: bool,
}
fn wall_rule(
    support: Option<f64>,
    support_strength: &str,
    pull: Option<f64>,
    pull_strength: &str,
    pull_ready: bool,
    contexts: bool,
    windows: &[wall_pull::LiveWindow],
) -> WallRead {
    let strong = |score: Option<f64>, strength: &str| {
        score.is_some_and(|n| n.is_finite() && (60.0..=100.0).contains(&n))
            && matches!(strength, "HIGH" | "EXTREME")
    };
    let support_ready = strong(support, support_strength);
    let pull_ready = pull_ready && strong(pull, pull_strength);
    let static_ready = support_ready && pull_ready && contexts;
    let live_ready = windows
        .iter()
        .any(|w| matches!(w.minutes, 1 | 5) && w.ready);
    WallRead {
        ready: static_ready && live_ready,
        support_ready,
        pull_ready,
        static_ready,
        live_ready,
    }
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub targets: Vec<f64>,
    pub structure_ready: bool,
    pub ready: bool,
    pub reason: String,
    pub wall: WallRead,
    pub position: position_map::Map,
    pub live_windows: Vec<wall_pull::LiveWindow>,
    pub entry_confirmed: bool,
}
fn target_levels(base: &[Option<f64>], pull: Option<f64>, price: f64, bullish: bool) -> Vec<f64> {
    if !price.is_finite() || price <= 0.0 {
        return vec![];
    }
    let ahead =
        |n: &f64| n.is_finite() && *n > 0.0 && if bullish { *n > price } else { *n < price };
    let round = |n: f64| format!("{n:.6}").parse::<f64>().unwrap_or(n);
    let order = |a: &f64, b: &f64| {
        if bullish {
            a.total_cmp(b)
        } else {
            b.total_cmp(a)
        }
    };
    let mut values: Vec<_> = base
        .iter()
        .flatten()
        .copied()
        .filter(ahead)
        .map(round)
        .collect();
    values.sort_by(order);
    values.dedup();
    values.truncate(2);
    if let Some(pull) = pull.filter(ahead) {
        values.push(round(pull));
    }
    values.retain(ahead);
    values.sort_by(order);
    values.dedup();
    values.truncate(2);
    values
}
pub fn evaluate(
    horizon: Option<&Horizon>,
    price: f64,
    bullish: bool,
    contracts: &[ContractWindow],
    verified: bool,
    now: u64,
) -> Read {
    let waiting = || Read {
        reason: "ESPERANDO ESTRUCTURA NEXT4 VIGENTE".into(),
        ..Default::default()
    };
    let Some(h) = horizon else {
        return waiting();
    };
    let same_day = calendar::date_at(now).is_some()
        && calendar::date_at(now) == calendar::date_at(h.observed_ms);
    if !verified
        || h.mode != "NEXT4"
        || h.observed_ms > now
        || now - h.observed_ms > 900_000
        || !same_day
        || !price.is_finite()
        || price <= 0.0
        || h.expirations.is_empty()
    {
        return waiting();
    }
    let s = &h.structure;
    let score = |n: f64| n.is_finite() && (0.0..=100.0).contains(&n);
    let positive = |n: Option<f64>| n.is_some_and(|n| n.is_finite() && n > 0.0);
    let structure_ready = s.complete
        && s.liquidity_complete
        && s.rows > 0
        && h.exposure.contracts > 0
        && h.exposure.gex_complete
        && h.exposure.scan_complete
        && positive(h.exposure.zero_gamma)
        && positive(s.magnet)
        && !matches!(h.exposure.gamma_regime.as_str(), "" | "-" | "N/A" | "NONE")
        && s.call_wall
            .as_ref()
            .is_some_and(|w| w.zone_available && score(w.score) && positive(Some(w.strike)))
        && s.put_wall
            .as_ref()
            .is_some_and(|w| w.zone_available && score(w.score) && positive(Some(w.strike)))
        && ["CALL", "PUT"].iter().all(|side| {
            s.routes
                .iter()
                .any(|r| r.direction == *side && r.score.is_some_and(score))
        });
    let side = if bullish { "CALL" } else { "PUT" };
    let (support, target) = if bullish {
        (&s.put_wall, &s.call_wall)
    } else {
        (&s.call_wall, &s.put_wall)
    };
    let route = s.routes.iter().find(|r| r.direction == side);
    let specs = wall_flow::structural_specs(s);
    let live_windows: Vec<_> =
        wall_flow::windows(&specs, &h.expirations, contracts, structure_ready)
            .iter()
            .map(|w| wall_pull::summarize(w, side))
            .collect();
    let wall = wall_rule(
        support.as_ref().map(|w| w.score),
        support.as_ref().map_or("", |w| w.strength.as_str()),
        route.and_then(|r| r.score),
        route
            .and_then(|r| r.score)
            .map_or("SIN DATOS", liquidity::strength),
        route.is_some_and(|r| r.structural_ready),
        support.as_ref().is_some_and(|w| w.zone_available)
            && target.as_ref().is_some_and(|w| w.zone_available),
        &live_windows,
    );
    let position = position_map::calculate(&position_map::Input {
        side: side.into(),
        price: Some(price),
        put_wall: s.put_wall.as_ref().map(|w| w.strike),
        call_wall: s.call_wall.as_ref().map(|w| w.strike),
        pull_target: route.and_then(|r| r.pull_target),
        main_magnet: s.selected.get("M1").copied().flatten(),
        liquidity_magnet: s.magnet,
        vacuum_low: route.and_then(|r| r.vacuum_low),
        vacuum_high: route.and_then(|r| r.vacuum_high),
    });
    let ready = structure_ready && wall.ready && position.ready;
    let reason = if !structure_ready {
        "ESTRUCTURA NEXT4 INCOMPLETA".into()
    } else if !wall.static_ready {
        "WALL / PULL SIN FUERZA VERIFICADA".into()
    } else if !wall.live_ready {
        "ESPERANDO COBERTURA LIVE DEL WALL, ADYACENTES Y PULL TARGET".into()
    } else if !position.ready {
        position.reason.clone()
    } else {
        "ESTRUCTURA + WALL/PULL LIVE + POSICION ALINEADOS".into()
    };
    let major = h
        .exposure
        .strikes
        .iter()
        .filter(|r| {
            if bullish {
                r.net_gex > 0.0
            } else {
                r.net_gex < 0.0
            }
        })
        .max_by(|a, b| {
            a.net_gex
                .abs()
                .total_cmp(&b.net_gex.abs())
                .then(b.strike.total_cmp(&a.strike))
        })
        .map(|r| r.strike);
    let targets = if structure_ready {
        target_levels(
            &[
                s.magnet,
                target.as_ref().map(|w| w.strike),
                major,
                if bullish {
                    s.dominant_call
                } else {
                    s.dominant_put
                },
                h.exposure.zero_gamma,
            ],
            route.and_then(|r| r.pull_target),
            price,
            bullish,
        )
    } else {
        vec![]
    };
    Read {
        targets,
        structure_ready,
        ready,
        reason,
        wall,
        position,
        live_windows,
        entry_confirmed: false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn targets_match_432_legacy_cases_including_rounding_and_direction() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/legacy-scanner-targets.json"
        ))
        .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 432);
        for (n, case) in cases.iter().enumerate() {
            let i = &case["input"];
            let base: Vec<_> = i["base"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64())
                .collect();
            assert_eq!(
                serde_json::json!(target_levels(
                    &base,
                    i["pull"].as_f64(),
                    100.,
                    i["bullish"].as_bool().unwrap()
                )),
                case["expected"],
                "case {n}"
            );
        }
    }
    #[test]
    fn full_structure_requires_live_coverage_geometry_and_current_snapshot() {
        use crate::{
            options::Observation,
            structure::{Route, Wall},
        };
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let observation = Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: now,
            observed_date: "2026-09-24".into(),
            oi_reference_date: None,
            contracts: Default::default(),
        };
        let mut h = crate::horizons::calculate(&observation, 100., "TEST")
            .unwrap()
            .pop()
            .unwrap();
        h.expirations = vec!["2026-09-25".into()];
        h.exposure.contracts = 10;
        h.exposure.gex_complete = true;
        h.exposure.scan_complete = true;
        h.exposure.zero_gamma = Some(100.);
        h.exposure.gamma_regime = "POSITIVE".into();
        let s = &mut h.structure;
        s.complete = true;
        s.liquidity_complete = true;
        s.rows = 10;
        s.magnet = Some(103.);
        let wall = |strike| Wall {
            zone_volume: None,
            zone_oi: None,
            strike,
            score: 80.,
            strength: "EXTREME".into(),
            zone_available: true,
            below: Some(strike - 1.),
            above: Some(strike + 1.),
            oi: 100.,
            volume: 100.,
            zone_score: 80.,
            ready: true,
        };
        s.put_wall = Some(wall(98.));
        s.call_wall = Some(wall(104.));
        s.routes = [("CALL", Some(106.)), ("PUT", Some(96.))]
            .into_iter()
            .map(|(side, target)| Route {
                path_liquidity: None,
                path_openness: None,
                vacuum_score: None,
                direction: side.into(),
                pull_target: target,
                barrier: None,
                barrier_score: None,
                path_state: "CLEAR".into(),
                target,
                extended: None,
                vacuum_low: None,
                vacuum_high: None,
                vacuum_status: "N/A".into(),
                score: Some(80.),
                structural_ready: true,
            })
            .collect();
        let specs = wall_flow::structural_specs(s);
        let rows: Vec<_> = specs
            .iter()
            .enumerate()
            .map(|(n, s)| ContractWindow {
                ticker: format!("TEST{n}"),
                kind: s.kind.clone(),
                strike: s.strike,
                expiration: "2026-09-25".into(),
                minutes: 1,
                observed_since_ms: Some(now - 60_000),
                covered: true,
                contracts: Some(0.),
                premium: Some(0.),
                trades: Some(0),
                reason: "TEST".into(),
            })
            .collect();
        for bullish in [true, false] {
            let r = evaluate(Some(&h), 100., bullish, &rows, true, now);
            assert!(r.ready, "{r:?}");
            assert!(!r.entry_confirmed);
        }
        assert!(!evaluate(Some(&h), 100., true, &[], true, now).ready);
        assert!(!evaluate(Some(&h), 107., true, &rows, true, now).ready);
        assert!(!evaluate(Some(&h), 100., true, &rows, false, now).ready);
        assert!(!evaluate(Some(&h), 100., true, &rows, true, now + 900_001).ready);
        assert!(!evaluate(Some(&h), 100., true, &rows, true, now - 1).ready);
        h.exposure.zero_gamma = None;
        assert!(!evaluate(Some(&h), 100., true, &rows, true, now).ready);
    }
    #[test]
    fn scanner_specific_wall_gate_matches_216_legacy_cases() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-scanner-wall.json"))
                .unwrap();
        let cases = fixtures["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 216);
        for (n, case) in cases.iter().enumerate() {
            let i = &case["input"];
            let support = i["support"].as_f64();
            let pull = i["pull"].as_f64();
            let w = i["window"].as_u64().unwrap();
            let windows = vec![wall_pull::LiveWindow {
                minutes: w,
                ready: w > 0,
                zones_covered: w > 0,
                pull_target_covered: w > 0,
            }];
            let r = wall_rule(
                support,
                support.map_or("SIN DATOS", liquidity::strength),
                pull,
                pull.map_or("SIN DATOS", liquidity::strength),
                i["pullReady"].as_bool().unwrap(),
                i["context"].as_bool().unwrap(),
                &windows,
            );
            assert_eq!(
                serde_json::json!({"ready":r.ready,"support_ready":r.support_ready,"pull_ready":r.pull_ready,"static_ready":r.static_ready,"live_ready":r.live_ready}),
                case["expected"],
                "case {n}"
            );
        }
    }
}
