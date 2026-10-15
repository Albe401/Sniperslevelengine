//! Bounded view of structural analytics for one explicit expiration universe.
use crate::{levels, liquidity, options::Observation};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub strike: f64,
    pub score: f64,
    pub kind: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Wall {
    pub zone_volume: Option<f64>,
    pub zone_oi: Option<f64>,
    pub zone_available: bool,
    pub below: Option<f64>,
    pub above: Option<f64>,
    pub strike: f64,
    pub score: f64,
    pub strength: String,
    pub oi: f64,
    pub volume: f64,
    pub zone_score: f64,
    pub ready: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    pub path_liquidity: Option<f64>,
    pub path_openness: Option<f64>,
    pub vacuum_score: Option<f64>,
    pub pull_target: Option<f64>,
    pub direction: String,
    pub barrier: Option<f64>,
    pub barrier_score: Option<f64>,
    pub path_state: String,
    pub target: Option<f64>,
    pub extended: Option<f64>,
    pub vacuum_low: Option<f64>,
    pub vacuum_high: Option<f64>,
    pub vacuum_status: String,
    pub score: Option<f64>,
    pub structural_ready: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Structure {
    pub level_quality: Vec<LevelQuality>,
    /// Display telemetry from the same Snipers universe; does not change signals.
    pub strike_rows: Vec<levels::Row>,
    pub top_contracts: ContractRanking,
    /// Display-only subset. Never used to certify the full universe or authorize signals.
    pub provisional: Option<Provisional>,
    pub quick_pressure: Option<f64>,
    pub dominant_call: Option<f64>,
    pub dominant_put: Option<f64>,
    pub reading: crate::reading::Reading,
    pub complete: bool,
    pub liquidity_complete: bool,
    pub missing_contracts: usize,
    pub history_available: bool,
    pub gex_available: bool,
    pub rows: usize,
    pub selected: BTreeMap<String, Option<f64>>,
    pub top: Vec<Point>,
    pub call_wall: Option<Wall>,
    pub put_wall: Option<Wall>,
    pub magnet: Option<f64>,
    pub main_magnet_score: Option<f64>,
    pub liquidity_magnet_score: Option<f64>,
    pub routes: Vec<Route>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelQuality {
    pub strike: f64,
    pub total: usize,
    pub covered: usize,
    pub oi_coverage: Option<f64>,
    pub missing_oi: usize,
    pub missing_volume: usize,
    pub missing_gamma: usize,
}
fn level_quality(contracts: &[&crate::options::ContractSnapshot]) -> Vec<LevelQuality> {
    let mut sorted = contracts.to_vec();
    sorted.sort_by(|a, b| a.strike.total_cmp(&b.strike));
    sorted.chunk_by(|a, b| a.strike == b.strike).map(|local| {
        let strike = local[0].strike;
        let valid = |c: &&&crate::options::ContractSnapshot| c.open_interest.is_some() && c.volume.is_some() && (c.open_interest == Some(0.0) || c.gamma.is_some());
        let total_oi: f64 = local.iter().filter_map(|c| c.open_interest).sum();
        let covered_oi: f64 = local.iter().filter(valid).filter_map(|c| c.open_interest).sum();
        let missing_oi = local.iter().filter(|c| c.open_interest.is_none()).count();
        LevelQuality { strike, total: local.len(), covered: local.iter().filter(valid).count(),
            oi_coverage: if missing_oi == 0 && total_oi > 0.0 { Some(100.0 * covered_oi / total_oi) } else { None },
            missing_oi, missing_volume: local.iter().filter(|c| c.volume.is_none()).count(),
            missing_gamma: local.iter().filter(|c| c.open_interest != Some(0.0) && c.gamma.is_none()).count() }
    }).collect()
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provisional {
    /// Verified subset exposure for display only, never full-universe signals.
    pub strike_rows: Vec<levels::Row>,
    pub covered_contracts: usize,
    pub total_contracts: usize,
    pub selected: BTreeMap<String, Option<f64>>,
    pub call_wall: Option<Wall>,
    pub put_wall: Option<Wall>,
    pub magnet: Option<f64>,
    pub main_magnet_score: Option<f64>,
    pub liquidity_magnet_score: Option<f64>,
    pub routes: Vec<Route>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedContract {
    pub ticker: String,
    pub expiration: String,
    pub strike: f64,
    pub score: f64,
    pub oi: f64,
    pub volume: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractRanking {
    pub covered: usize,
    pub total: usize,
    pub calls: Vec<RankedContract>,
    pub puts: Vec<RankedContract>,
}
fn rank_contracts(contracts: &[&crate::options::ContractSnapshot]) -> ContractRanking {
    // Legacy rank_contracts weights: OI 40%, volume 40%, |gamma| * OI 20%.
    // Unknown fields are excluded, not silently replaced by zero.
    let eligible: Vec<_> = contracts
        .iter()
        .filter_map(|c| {
            let (oi, volume) = (c.open_interest?, c.volume?);
            let gamma = if oi == 0.0 { 0.0 } else { c.gamma?.abs() * oi };
            (oi.is_finite()
                && volume.is_finite()
                && gamma.is_finite()
                && oi >= 0.0
                && volume >= 0.0)
                .then_some((*c, oi, volume, gamma))
        })
        .collect();
    let max = |index: usize| {
        eligible
            .iter()
            .map(|(_, o, v, g)| [*o, *v, *g][index])
            .fold(0.0_f64, f64::max)
    };
    let maxima = [max(0), max(1), max(2)].map(|v| if v > 0.0 { v } else { 1.0 });
    let side = |kind: &str| {
        let mut rows: Vec<_> = eligible
            .iter()
            .filter(|(c, _, _, _)| c.kind == kind)
            .map(|(c, oi, volume, gamma)| RankedContract {
                ticker: c.ticker.clone(),
                expiration: c.expiration.clone(),
                strike: c.strike,
                score: (oi / maxima[0] * 40.0
                    + volume / maxima[1] * 40.0
                    + gamma / maxima[2] * 20.0)
                    .min(100.0),
                oi: *oi,
                volume: *volume,
            })
            .collect();
        rows.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.ticker.cmp(&b.ticker)));
        rows.truncate(5);
        rows
    };
    ContractRanking {
        covered: eligible.len(),
        total: contracts.len(),
        calls: side("call"),
        puts: side("put"),
    }
}
pub fn calculate(
    current: &Observation,
    previous: Option<&Observation>,
    expirations: &[String],
    spot: f64,
) -> Result<Structure, &'static str> {
    let contracts: Vec<_> = current
        .contracts
        .values()
        .filter(|c| expirations.contains(&c.expiration))
        .collect();
    let input: Vec<_> = contracts
        .iter()
        .map(|c| levels::Contract {
            strike: c.strike,
            kind: c.kind.clone(),
            oi: c.open_interest,
            volume: c.volume,
            gamma: c.gamma,
            gex: if c.open_interest == Some(0.0) {
                Some(0.0)
            } else {
                c.gamma
                    .zip(c.open_interest)
                    .zip(c.premium_multiplier)
                    .map(|((g, oi), m)| g * oi * m * spot * spot * 0.01)
            },
        })
        .collect();
    let previous =
        previous.filter(|p| p.symbol == current.symbol && p.observed_date < current.observed_date);
    let mut grouped: BTreeMap<u64, (Option<f64>, Option<f64>)> = BTreeMap::new();
    for c in &contracts {
        let group = grouped
            .entry(c.strike.to_bits())
            .or_insert((Some(0.0), Some(0.0)));
        let change = previous
            .and_then(|p| p.contracts.get(&c.ticker))
            .and_then(|c| c.open_interest)
            .zip(c.open_interest)
            .map(|(old, new)| new - old);
        let side = if c.kind == "call" {
            &mut group.0
        } else {
            &mut group.1
        };
        *side = side.zip(change).map(|(sum, change)| sum + change);
    }
    let changes: Vec<_> = grouped
        .into_iter()
        .map(|(s, (c, p))| (f64::from_bits(s), c, p))
        .collect();
    // Apply daily-change weights only to a fully comparable contract universe.
    // A newly listed or missing contract is unknown, not a zero daily change.
    let history = previous.is_some()
        && !changes.is_empty()
        && changes.iter().all(|(_, c, p)| c.is_some() && p.is_some());
    let levels = levels::calculate(&input, spot, &changes, history, true)?;
    let gamma_complete = contracts
        .iter()
        .all(|c| c.open_interest == Some(0.0) || c.gamma.is_some());
    let magnet = levels.selected.get("M1").copied().flatten();
    let liquidity = liquidity::calculate(
        &levels.rows,
        spot,
        magnet,
        levels.complete && gamma_complete,
    )?;
    let mut ranked = levels.rows.iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.score.total_cmp(&a.score));
    let top = ranked
        .into_iter()
        .take(12)
        .map(|r| Point {
            strike: r.strike,
            score: r.score,
            kind: r.classification.clone(),
        })
        .collect();
    let wall = |w: &liquidity::Wall, z: &Option<liquidity::Zone>| Wall {
        zone_volume: z.as_ref().map(|z| z.zone_total_volume),
        zone_oi: z.as_ref().map(|z| z.zone_total_oi),
        zone_available: z.is_some(),
        below: z
            .as_ref()
            .and_then(|z| z.below.as_ref())
            .map(|r| r.base.strike),
        above: z
            .as_ref()
            .and_then(|z| z.above.as_ref())
            .map(|r| r.base.strike),
        strike: w.strike,
        score: w.wall_score,
        strength: w.wall_strength.clone(),
        oi: if w.side == "CALL" {
            w.row.base.call_oi
        } else {
            w.row.base.put_oi
        },
        volume: if w.side == "CALL" {
            w.row.base.call_volume
        } else {
            w.row.base.put_volume
        },
        zone_score: z.as_ref().map_or(0.0, |z| z.zone_score),
        ready: w.wall_ready,
    };
    let mut routes = vec![];
    for (direction, barrier, pull, target, extended, vacuum) in [
        (
            "CALL",
            &liquidity.upside_liquidity_barrier,
            &liquidity.call_pull,
            &liquidity.upside_target_1,
            &liquidity.upside_target_2,
            &liquidity.upside_vacuum,
        ),
        (
            "PUT",
            &liquidity.downside_liquidity_barrier,
            &liquidity.put_pull,
            &liquidity.downside_target_1,
            &liquidity.downside_target_2,
            &liquidity.downside_vacuum,
        ),
    ] {
        routes.push(Route {
            path_liquidity: barrier.as_ref().and_then(|b| b.path_liquidity_score),
            path_openness: barrier.as_ref().and_then(|b| b.path_openness_score),
            vacuum_score: vacuum.as_ref().map(|v| v.vacuum_score),
            pull_target: pull.as_ref().map(|p| p.preferred_target.base.strike),
            direction: direction.into(),
            barrier: barrier.as_ref().and_then(|b| b.barrier_strike),
            barrier_score: barrier.as_ref().and_then(|b| b.barrier_score),
            path_state: barrier
                .as_ref()
                .map_or("SIN DATOS".into(), |b| b.path_state.clone()),
            target: target.as_ref().map(|t| t.strike),
            extended: extended.as_ref().map(|t| t.strike),
            vacuum_low: vacuum.as_ref().map(|v| v.low_strike),
            vacuum_high: vacuum.as_ref().map(|v| v.high_strike),
            vacuum_status: vacuum
                .as_ref()
                .map(|v| v.status.clone())
                .unwrap_or_default(),
            score: pull.as_ref().map(|p| p.pull_score),
            structural_ready: pull.as_ref().is_some_and(|p| p.ready),
        });
    }
    let dominant = |call: bool| {
        let mut candidates: Vec<_> = levels
            .rows
            .iter()
            .filter(|r| {
                if call {
                    r.dominance >= 0.15
                } else {
                    r.dominance <= -0.15
                }
            })
            .collect();
        candidates.sort_by(|a, b| {
            b.dominance
                .abs()
                .total_cmp(&a.dominance.abs())
                .then(b.score.total_cmp(&a.score))
        });
        levels
            .complete
            .then(|| candidates.first().map(|r| r.strike))
            .flatten()
    };
    let provisional = if !levels.complete || !gamma_complete {
        let mut subset = current.clone();
        subset.contracts.retain(|_, c| {
            expirations.contains(&c.expiration)
                && c.open_interest.is_some()
                && c.volume.is_some()
                && (c.open_interest == Some(0.0) || c.gamma.is_some())
        });
        let covered = subset.contracts.len();
        if covered > 0 && covered < contracts.len() {
            // All retained contracts satisfy the guards, so recursion stops after one level.
            let display = calculate(&subset, previous, expirations, spot)?;
            Some(Provisional {
                strike_rows: display.strike_rows,
                covered_contracts: covered,
                total_contracts: contracts.len(),
                selected: display.selected,
                call_wall: display.call_wall.map(|mut w| {
                    w.ready = false;
                    w
                }),
                put_wall: display.put_wall.map(|mut w| {
                    w.ready = false;
                    w
                }),
                magnet: display.magnet,
                main_magnet_score: display.main_magnet_score,
                liquidity_magnet_score: display.liquidity_magnet_score,
                routes: display
                    .routes
                    .into_iter()
                    .map(|mut r| {
                        r.structural_ready = false;
                        r
                    })
                    .collect(),
            })
        } else {
            None
        }
    } else {
        None
    };
    Ok(Structure {
        level_quality: level_quality(&contracts),
        strike_rows: levels.rows.clone(),
        top_contracts: rank_contracts(&contracts),
        provisional,
        quick_pressure: quick_pressure(&levels.rows, levels.complete && gamma_complete),
        dominant_call: dominant(true),
        dominant_put: dominant(false),
        reading: crate::reading::calculate(
            &levels.rows,
            levels.complete && levels.gex_available,
            levels.rows.iter().map(|r| r.call_gex).sum(),
            levels.rows.iter().map(|r| r.put_gex).sum(),
        ),
        complete: levels.complete,
        liquidity_complete: liquidity.complete,
        missing_contracts: levels.missing_contracts,
        history_available: history,
        gex_available: levels.gex_available,
        rows: levels.rows.len(),
        selected: levels.selected,
        top,
        call_wall: liquidity
            .call_wall
            .as_ref()
            .map(|w| wall(w, &liquidity.call_wall_context)),
        put_wall: liquidity
            .put_wall
            .as_ref()
            .map(|w| wall(w, &liquidity.put_wall_context)),
        magnet: liquidity.magnet,
        main_magnet_score: if liquidity.complete {magnet.and_then(|strike|liquidity.rows.iter().find(|r|r.base.strike==strike).map(|r|r.magnet_score))}else{None},
        liquidity_magnet_score: if liquidity.complete {liquidity.magnet.and_then(|strike|liquidity.rows.iter().find(|r|r.base.strike==strike).map(|r|r.magnet_score))}else{None},
        routes,
    })
}

// Python Quick Read: OI 45%, accumulated volume 35%, gamma weight 20%.
// Missing observations remain unavailable, rather than becoming neutral zeros.
fn quick_pressure(rows: &[levels::Row], complete: bool) -> Option<f64> {
    if !complete || rows.is_empty() {
        return None;
    }
    let mut totals = [0.0; 6];
    for r in rows {
        let values = [
            r.call_oi,
            r.put_oi,
            r.call_volume,
            r.put_volume,
            r.call_gamma_weight,
            r.put_gamma_weight,
        ];
        for (sum, value) in totals.iter_mut().zip(values) {
            if !value.is_finite() || value < 0.0 {
                return None;
            }
            *sum += value;
        }
    }
    if totals.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let share = |a: f64, b: f64| if a + b > 0.0 { a / (a + b) } else { 0.5 };
    Some(
        100.0
            * (share(totals[0], totals[1]) * 0.45
                + share(totals[2], totals[3]) * 0.35
                + share(totals[4], totals[5]) * 0.20),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn quick_pressure_matches_python_weights_and_rejects_missing_data() {
        let row = crate::levels::Row {
            call_oi: 80.0,
            put_oi: 20.0,
            call_volume: 60.0,
            put_volume: 40.0,
            call_gamma_weight: 30.0,
            put_gamma_weight: 70.0,
            ..Default::default()
        };
        assert!(
            (super::quick_pressure(std::slice::from_ref(&row), true).unwrap() - 63.0).abs() < 1e-9
        );
        assert_eq!(super::quick_pressure(&[row], false), None);
        assert_eq!(super::quick_pressure(&[], true), None);
        assert_eq!(
            super::quick_pressure(&[Default::default()], true),
            Some(50.0)
        );
    }
    use super::*;
    use crate::options::normalize;
    use serde_json::json;
    #[test]
    fn provisional_subset_does_not_promote_incomplete_universe() {
        let mut current = observation();
        let dates = vec!["2026-09-25".into(), "2026-10-02".into()];
        current
            .contracts
            .get_mut("O:QQQ261002C00520000")
            .unwrap()
            .volume = None;
        let result = calculate(&current, None, &dates, 500.0).unwrap();
        assert!(!result.complete && !result.reading.available);
        assert!(result.selected.values().all(Option::is_none));
        assert!(result.call_wall.is_none());
        assert!(result.level_quality.iter().filter(|q| q.strike != 520.0).all(|q| q.covered == q.total));
        assert_eq!(result.level_quality.iter().find(|q| q.strike == 520.0).unwrap().missing_volume, 1);
        let display = result.provisional.unwrap();
        assert_eq!((display.covered_contracts, display.total_contracts), (2, 3));
        assert_eq!(display.strike_rows.len(), 2);
        assert_eq!(display.main_magnet_score.is_some(), display.selected["M1"].is_some());
        assert!(display.main_magnet_score.is_none_or(|v| v.is_finite() && (0.0..=100.0).contains(&v)));
        assert!(display.liquidity_magnet_score.is_some_and(|v| v.is_finite() && (0.0..=100.0).contains(&v)));
        assert!(display.strike_rows.iter().all(|r| r.gross_gex > 0.));
        assert_eq!(display.call_wall.unwrap().strike, 510.0);
        assert_eq!(display.put_wall.unwrap().strike, 490.0);
        assert!(display.routes.iter().all(|r| !r.structural_ready));
        for c in current.contracts.values_mut() {
            c.volume = None;
        }
        assert!(calculate(&current, None, &dates, 500.0)
            .unwrap()
            .provisional
            .is_none());
        assert!(calculate(&observation(), None, &dates, 500.0)
            .unwrap()
            .provisional
            .is_none());
    }
    fn observation() -> Observation {
        let contracts=[("O:QQQ260925P00490000",490.0,"put","2026-09-25"),
            ("O:QQQ260925C00510000",510.0,"call","2026-09-25"),
            ("O:QQQ261002C00520000",520.0,"call","2026-10-02")]
            .into_iter().map(|(ticker,strike,kind,expiration)| {
                let mut c=normalize(&json!({"details":{"ticker":ticker,"strike_price":strike,"contract_type":kind,"expiration_date":expiration},"open_interest":100,"day":{"volume":20},"greeks":{"gamma":0.02}}),"QQQ").unwrap();
                c.premium_multiplier=Some(100.0);
                (ticker.into(),c)
            }).collect();
        Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: 1,
            observed_date: "2026-09-24".into(),
            oi_reference_date: None,
            contracts,
        }
    }
    #[test]
    fn contract_ranking_preserves_legacy_weights_and_unknowns() {
        let mut obs = observation();
        let key = "O:QQQ260925C00510000";
        obs.contracts.get_mut(key).unwrap().open_interest = Some(50.0);
        obs.contracts.get_mut(key).unwrap().volume = Some(10.0);
        let ranking = rank_contracts(&obs.contracts.values().collect::<Vec<_>>());
        assert_eq!(ranking.covered, 3);
        assert_eq!(ranking.calls[0].strike, 520.0);
        assert_eq!(ranking.calls[0].score, 100.0);
        assert_eq!(ranking.calls[1].score, 50.0);
        assert_eq!(ranking.puts.len(), 1);
        obs.contracts.get_mut(key).unwrap().volume = None;
        let ranking = rank_contracts(&obs.contracts.values().collect::<Vec<_>>());
        assert_eq!((ranking.covered, ranking.total), (2, 3));
        assert_eq!(ranking.calls.len(), 1);
    }
    #[test]
    fn isolates_expirations_asset_and_daily_comparable_universe() {
        let current = observation();
        let dates = vec!["2026-09-25".into()];
        let mut previous = current.clone();
        previous.observed_date = "2026-09-23".into();
        for c in previous.contracts.values_mut() {
            c.open_interest = Some(90.0);
        }
        let result = calculate(&current, Some(&previous), &dates, 500.0).unwrap();
        assert_eq!(result.rows, 2);
        assert!(result.complete && result.liquidity_complete && result.history_available);
        assert!(result.reading.available);
        assert_eq!(result.reading.side, "NEUTRAL");
        assert_eq!(result.selected["R1"], Some(510.0));
        assert_eq!(result.selected["S1"], Some(490.0));
        assert_eq!(result.call_wall.as_ref().unwrap().below, Some(490.0));
        assert_eq!(result.call_wall.as_ref().unwrap().above, None);
        assert_eq!(result.put_wall.as_ref().unwrap().above, Some(510.0));
        assert_eq!(result.put_wall.as_ref().unwrap().below, None);
        assert_eq!(result.main_magnet_score.is_some(),result.selected["M1"].is_some());
        assert!(result.liquidity_magnet_score.is_some_and(|score|score>=0.0&&score<=100.0));
        if result.selected["M1"]==result.magnet {assert_eq!(result.main_magnet_score,result.liquidity_magnet_score);}
        assert!(result.top.iter().all(|r| r.strike != 520.0));
        previous.symbol = "SPY".into();
        assert!(
            !calculate(&current, Some(&previous), &dates, 500.0)
                .unwrap()
                .history_available
        );
        previous.symbol = "QQQ".into();
        previous.observed_date = current.observed_date.clone();
        assert!(
            !calculate(&current, Some(&previous), &dates, 500.0)
                .unwrap()
                .history_available
        );
        previous.observed_date = "2026-09-23".into();
        previous.contracts.remove("O:QQQ260925C00510000");
        assert!(
            !calculate(&current, Some(&previous), &dates, 500.0)
                .unwrap()
                .history_available
        );
    }
    #[test]
    fn incomplete_inputs_never_publish_certified_routes() {
        let mut current = observation();
        let dates = vec!["2026-09-25".into()];
        current
            .contracts
            .get_mut("O:QQQ260925C00510000")
            .unwrap()
            .gamma = None;
        let result = calculate(&current, None, &dates, 500.0).unwrap();
        assert!(result.complete);
        assert!(!result.liquidity_complete && !result.gex_available);
        assert!(result.main_magnet_score.is_none()&&result.liquidity_magnet_score.is_none());
        assert!(!result.reading.available);
        assert!(result.call_wall.is_none() && result.put_wall.is_none());
        assert!(result
            .routes
            .iter()
            .all(|r| !r.structural_ready && r.target.is_none()));
        current
            .contracts
            .get_mut("O:QQQ260925C00510000")
            .unwrap()
            .volume = None;
        let result = calculate(&current, None, &dates, 500.0).unwrap();
        assert!(!result.complete);
        assert!(!result.reading.available);
        assert_eq!(result.missing_contracts, 1);
        assert!(result.selected.values().all(Option::is_none));
        let empty = calculate(&current, None, &["2027-01-01".into()], 500.0).unwrap();
        assert!(empty.top.is_empty() && empty.call_wall.is_none());
    }
}
