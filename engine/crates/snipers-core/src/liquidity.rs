//! Structural snapshot liquidity. Live confirmation and entry permission are separate.
use crate::levels::Row;
use serde::Serialize;

fn clamp(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}
fn norm(v: f64, max: f64) -> f64 {
    if max > 0.0 {
        clamp(v / max)
    } else {
        0.0
    }
}
fn round(v: f64, digits: usize) -> f64 {
    format!("{v:.digits$}")
        .parse()
        .expect("finite liquidity score")
}
pub fn strength(score: f64) -> &'static str {
    if score >= 80.0 {
        "EXTREME"
    } else if score >= 60.0 {
        "HIGH"
    } else if score >= 40.0 {
        "MEDIUM"
    } else {
        "LOW"
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct LiquidityRow {
    #[serde(flatten)]
    pub base: Row,
    pub call_wall_score: f64,
    pub put_wall_score: f64,
    pub liquidity_strength: f64,
    pub liquidity_score: f64,
    pub magnet_score: f64,
    pub liquidity_gex_strength: f64,
}
impl LiquidityRow {
    fn strike(&self) -> f64 {
        self.base.strike
    }
    fn target_strength(&self) -> f64 {
        clamp(self.liquidity_score / 100.0) * 0.6
            + clamp(self.base.score / 100.0) * 0.25
            + clamp(self.liquidity_gex_strength) * 0.15
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Wall {
    pub strike: f64,
    pub wall_score: f64,
    pub wall_ready: bool,
    pub wall_strength: String,
    pub side: String,
    pub row: LiquidityRow,
}
#[derive(Clone, Debug, Serialize)]
pub struct Target {
    pub strike: f64,
    pub breakout_target_score: f64,
    pub rank: usize,
    pub row: LiquidityRow,
}
#[derive(Clone, Debug, Serialize)]
pub struct Vacuum {
    pub status: String,
    pub low_strike: f64,
    pub high_strike: f64,
    pub strike_count: usize,
    pub average_liquidity_strength: f64,
    pub vacuum_score: f64,
    pub threshold: f64,
    pub next_liquidity_pool: Option<LiquidityRow>,
    pub rows: Vec<LiquidityRow>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Zone {
    pub strike: f64,
    pub zone_total_volume: f64,
    pub zone_total_oi: f64,
    pub zone_score: f64,
    pub below: Option<LiquidityRow>,
    pub above: Option<LiquidityRow>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Path {
    pub strike_count: usize,
    pub average_liquidity_score: f64,
    pub maximum_liquidity_score: f64,
    pub total_volume: f64,
    pub total_oi: f64,
    pub openness_score: f64,
    pub levels: Vec<LiquidityRow>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Barrier {
    pub barrier_found: bool,
    pub barrier_strike: Option<f64>,
    pub barrier_score: Option<f64>,
    pub path_liquidity_score: Option<f64>,
    pub path_openness_score: Option<f64>,
    pub path_has_obstacle: bool,
    pub candidate_count: usize,
    pub meaningful_candidate_count: usize,
    pub anchor: f64,
    pub anchor_source: String,
    pub path_state: String,
    pub path_average_state: String,
    pub path: Option<Path>,
    pub entry_confirmed: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Pull {
    pub ready: bool,
    pub path_valid: bool,
    pub near_pull_score: f64,
    pub extended_pull_score: Option<f64>,
    pub pull_score: f64,
    pub preferred_target: LiquidityRow,
    pub near_path: Path,
    pub extended_path: Option<Path>,
    pub support_ready: bool,
    pub live_incoming_volume_required: bool,
}
#[derive(Clone, Default, Debug, Serialize)]
pub struct Liquidity {
    pub complete: bool,
    pub rows: Vec<LiquidityRow>,
    pub call_wall: Option<Wall>,
    pub put_wall: Option<Wall>,
    pub magnet: Option<f64>,
    pub upside_target_1: Option<Target>,
    pub upside_target_2: Option<Target>,
    pub downside_target_1: Option<Target>,
    pub downside_target_2: Option<Target>,
    pub upside_vacuum: Option<Vacuum>,
    pub downside_vacuum: Option<Vacuum>,
    pub upside_liquidity_barrier: Option<Barrier>,
    pub downside_liquidity_barrier: Option<Barrier>,
    pub call_wall_context: Option<Zone>,
    pub put_wall_context: Option<Zone>,
    pub call_pull: Option<Pull>,
    pub put_pull: Option<Pull>,
}
fn maximum(rows: &[Row], field: impl Fn(&Row) -> f64) -> f64 {
    rows.iter().map(field).fold(0.0, f64::max)
}
pub fn enrich(rows: &[Row], spot: f64) -> Vec<LiquidityRow> {
    let co = maximum(rows, |r| r.call_oi);
    let po = maximum(rows, |r| r.put_oi);
    let cv = maximum(rows, |r| r.call_volume);
    let pv = maximum(rows, |r| r.put_volume);
    let cg = maximum(rows, |r| r.call_gamma_weight.abs());
    let pg = maximum(rows, |r| r.put_gamma_weight.abs());
    let oi = maximum(rows, |r| r.total_oi);
    let vol = maximum(rows, |r| r.total_volume);
    let gamma = maximum(rows, |r| r.total_gamma_weight.abs());
    let cc = maximum(rows, |r| r.call_oi_change.unwrap_or(0.0).abs());
    let pc = maximum(rows, |r| r.put_oi_change.unwrap_or(0.0).abs());
    let gx = maximum(rows, |r| r.gross_gex.abs());
    rows.iter()
        .map(|r| {
            let existing = clamp(r.score / 100.0);
            let gs = norm(r.gross_gex.abs(), gx);
            let score = |oi, volume, gamma, dom, change| {
                if gx > 0.0 {
                    oi * 0.35
                        + volume * 0.2
                        + gamma * 0.15
                        + dom * 0.1
                        + change * 0.1
                        + existing * 0.05
                        + gs * 0.05
                } else {
                    oi * 0.38
                        + volume * 0.22
                        + gamma * 0.18
                        + dom * 0.1
                        + change * 0.07
                        + existing * 0.05
                }
            };
            let call = score(
                norm(r.call_oi, co),
                norm(r.call_volume, cv),
                norm(r.call_gamma_weight.abs(), cg),
                clamp(r.dominance.max(0.0)),
                norm(r.call_oi_change.unwrap_or(0.0).abs(), cc),
            );
            let put = score(
                norm(r.put_oi, po),
                norm(r.put_volume, pv),
                norm(r.put_gamma_weight.abs(), pg),
                clamp((-r.dominance).max(0.0)),
                norm(r.put_oi_change.unwrap_or(0.0).abs(), pc),
            );
            let liquidity = norm(r.total_oi, oi) * 0.35
                + norm(r.total_volume, vol) * 0.2
                + norm(r.total_gamma_weight.abs(), gamma) * 0.15
                + gs * 0.15
                + existing * 0.15;
            let proximity = 1.0 / (1.0 + (r.strike - spot).abs() / spot * 100.0);
            LiquidityRow {
                base: r.clone(),
                call_wall_score: round(call * 100.0, 2),
                put_wall_score: round(put * 100.0, 2),
                liquidity_strength: round(liquidity, 6),
                liquidity_score: round(liquidity * 100.0, 2),
                magnet_score: round((liquidity * 0.75 + proximity * 0.25) * 100.0, 2),
                liquidity_gex_strength: round(gs, 6),
            }
        })
        .collect()
}
fn wall(rows: &[LiquidityRow], spot: f64, call: bool) -> Option<Wall> {
    let mut candidates: Vec<_> = rows
        .iter()
        .filter(|r| {
            if call {
                r.strike() >= spot
            } else {
                r.strike() <= spot
            }
        })
        .collect();
    if candidates.is_empty() {
        candidates = rows.iter().collect();
    }
    let score = |r: &LiquidityRow| {
        if call {
            r.call_wall_score
        } else {
            r.put_wall_score
        }
    };
    candidates.sort_by(|a, b| {
        score(b).total_cmp(&score(a)).then_with(|| {
            (a.strike() - spot)
                .abs()
                .total_cmp(&(b.strike() - spot).abs())
        })
    });
    candidates.first().map(|r| Wall {
        strike: r.strike(),
        wall_score: score(r),
        wall_ready: score(r) >= 60.0,
        wall_strength: strength(score(r)).into(),
        side: if call { "CALL" } else { "PUT" }.into(),
        row: (*r).clone(),
    })
}
fn targets(rows: &[LiquidityRow], anchor: f64, up: bool) -> [Option<Target>; 2] {
    let mut candidates: Vec<_> = rows
        .iter()
        .filter(|r| {
            if up {
                r.strike() > anchor
            } else {
                r.strike() < anchor
            }
        })
        .collect();
    candidates.sort_by(|a, b| {
        if up {
            a.strike().total_cmp(&b.strike())
        } else {
            b.strike().total_cmp(&a.strike())
        }
    });
    fn pool<'a>(values: &[&'a LiquidityRow]) -> Vec<&'a LiquidityRow> {
        let good: Vec<_> = values
            .iter()
            .copied()
            .filter(|r| r.liquidity_score >= 25.0)
            .collect();
        if good.is_empty() {
            values.to_vec()
        } else {
            good
        }
    }
    let mut primary_pool = pool(&candidates);
    primary_pool.sort_by(|a, b| {
        (a.strike() - anchor)
            .abs()
            .total_cmp(&(b.strike() - anchor).abs())
            .then_with(|| b.target_strength().total_cmp(&a.target_strength()))
    });
    let Some(primary) = primary_pool.first().copied() else {
        return [None, None];
    };
    let rest: Vec<_> = candidates
        .into_iter()
        .filter(|r| {
            if up {
                r.strike() > primary.strike()
            } else {
                r.strike() < primary.strike()
            }
        })
        .collect();
    let mut rest = pool(&rest);
    rest.sort_by(|a, b| {
        b.target_strength()
            .total_cmp(&a.target_strength())
            .then_with(|| {
                (a.strike() - primary.strike())
                    .abs()
                    .total_cmp(&(b.strike() - primary.strike()).abs())
            })
    });
    let decorate = |row: &LiquidityRow, rank| Target {
        strike: row.strike(),
        breakout_target_score: round(row.target_strength() * 100.0, 2),
        rank,
        row: row.clone(),
    };
    [
        Some(decorate(primary, 1)),
        rest.first().map(|r| decorate(r, 2)),
    ]
}
fn vacuum(rows: &[LiquidityRow], spot: f64, up: bool, target: f64) -> Option<Vacuum> {
    let mut side: Vec<_> = rows
        .iter()
        .filter(|r| {
            if up {
                r.strike() > spot
            } else {
                r.strike() < spot
            }
        })
        .collect();
    side.sort_by(|a, b| {
        if up {
            a.strike().total_cmp(&b.strike())
        } else {
            b.strike().total_cmp(&a.strike())
        }
    });
    let limited: Vec<_> = side
        .iter()
        .copied()
        .filter(|r| {
            if up {
                r.strike() <= target
            } else {
                r.strike() >= target
            }
        })
        .collect();
    if !limited.is_empty() {
        side = limited;
    }
    if side.is_empty() {
        return None;
    }
    let mut values: Vec<_> = side.iter().map(|r| clamp(r.liquidity_strength)).collect();
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let median = if n % 2 == 0 {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    } else {
        values[n / 2]
    };
    let threshold = (median * 0.7).clamp(0.12, 0.35);
    let mut groups: Vec<Vec<&LiquidityRow>> = vec![];
    let mut group = vec![];
    for r in &side {
        if clamp(r.liquidity_strength) <= threshold {
            group.push(*r);
        } else if !group.is_empty() {
            groups.push(std::mem::take(&mut group));
        }
    }
    if !group.is_empty() {
        groups.push(group);
    }
    if groups.is_empty() {
        groups.push(vec![*side
            .iter()
            .min_by(|a, b| a.liquidity_strength.total_cmp(&b.liquidity_strength))
            .unwrap()]);
    }
    let average = |g: &Vec<&LiquidityRow>| {
        g.iter().map(|r| clamp(r.liquidity_strength)).sum::<f64>() / g.len() as f64
    };
    let quality = |g: &Vec<&LiquidityRow>| {
        (1.0 - average(g)) * 0.65
            + (g.len() as f64 / 5.0).min(1.0) * 0.25
            + ((g.first().unwrap().strike() - g.last().unwrap().strike()).abs() / 5.0).min(1.0)
                * 0.1
    };
    groups.sort_by(|a, b| quality(b).total_cmp(&quality(a)));
    let best = &groups[0];
    let avg = average(best);
    let low = best
        .iter()
        .map(|r| r.strike())
        .fold(f64::INFINITY, f64::min);
    let high = best.iter().map(|r| r.strike()).fold(0.0, f64::max);
    let next = side.iter().find(|r| {
        (if up {
            r.strike() > high
        } else {
            r.strike() < low
        }) && r.liquidity_strength > threshold
    });
    Some(Vacuum {
        status: if best.len() >= 2 && avg <= threshold {
            "VACUUM"
        } else {
            "LOW-LIQUIDITY POCKET"
        }
        .into(),
        low_strike: low,
        high_strike: high,
        strike_count: best.len(),
        average_liquidity_strength: round(avg, 6),
        vacuum_score: round((1.0 - avg) * 100.0, 2),
        threshold: round(threshold, 6),
        next_liquidity_pool: next.map(|r| (*r).clone()),
        rows: best.iter().map(|r| (*r).clone()).collect(),
    })
}
fn zone(rows: &[LiquidityRow], wall: &Wall) -> Zone {
    let below = rows
        .iter()
        .filter(|r| r.strike() < wall.strike)
        .max_by(|a, b| a.strike().total_cmp(&b.strike()));
    let above = rows
        .iter()
        .filter(|r| r.strike() > wall.strike)
        .min_by(|a, b| a.strike().total_cmp(&b.strike()));
    let group: Vec<_> = below
        .into_iter()
        .chain(std::iter::once(&wall.row))
        .chain(above)
        .collect();
    let avg = group.iter().map(|r| r.liquidity_score).sum::<f64>() / group.len() as f64;
    Zone {
        strike: wall.strike,
        zone_total_volume: round(group.iter().map(|r| r.base.total_volume).sum(), 2),
        zone_total_oi: round(group.iter().map(|r| r.base.total_oi).sum(), 2),
        zone_score: round(wall.wall_score * 0.65 + avg * 0.35, 2),
        below: below.cloned(),
        above: above.cloned(),
    }
}
fn path(rows: &[LiquidityRow], start: f64, end: f64) -> Path {
    let selected: Vec<_> = rows
        .iter()
        .filter(|r| r.strike() > start.min(end) && r.strike() < start.max(end))
        .cloned()
        .collect();
    let avg = if selected.is_empty() {
        0.0
    } else {
        selected.iter().map(|r| r.liquidity_score).sum::<f64>() / selected.len() as f64
    };
    Path {
        strike_count: selected.len(),
        average_liquidity_score: round(avg, 2),
        maximum_liquidity_score: selected
            .iter()
            .map(|r| r.liquidity_score)
            .fold(0.0, f64::max),
        total_volume: round(selected.iter().map(|r| r.base.total_volume).sum(), 2),
        total_oi: round(selected.iter().map(|r| r.base.total_oi).sum(), 2),
        openness_score: round(100.0 - clamp(avg / 100.0) * 100.0, 2),
        levels: selected,
    }
}
fn path_label(v: f64) -> &'static str {
    if v >= 60.0 {
        "CARGADO"
    } else if v >= 40.0 {
        "MODERADO"
    } else if v >= 25.0 {
        "LIGERO"
    } else {
        "LIMPIO"
    }
}
fn barrier(
    rows: &[LiquidityRow],
    spot: f64,
    magnet: Option<f64>,
    wall: &Wall,
    up: bool,
) -> Barrier {
    let between = magnet.is_some_and(|m| {
        if up {
            spot < m && m < wall.strike
        } else {
            spot > m && m > wall.strike
        }
    });
    let anchor = if between { magnet.unwrap() } else { spot };
    let mut result = Barrier {
        barrier_found: false,
        barrier_strike: None,
        barrier_score: None,
        path_liquidity_score: None,
        path_openness_score: None,
        path_has_obstacle: false,
        candidate_count: 0,
        meaningful_candidate_count: 0,
        anchor,
        anchor_source: if between { "MAIN_MAGNET" } else { "PRICE" }.into(),
        path_state: "SIN DATOS".into(),
        path_average_state: "SIN DATOS".into(),
        path: None,
        entry_confirmed: false,
    };
    if !(if up {
        wall.strike > anchor
    } else {
        wall.strike < anchor
    }) {
        return result;
    }
    let metrics = path(rows, anchor, wall.strike);
    let mut meaningful: Vec<_> = metrics
        .levels
        .iter()
        .filter(|r| r.liquidity_score >= 40.0)
        .collect();
    meaningful.sort_by(|a, b| {
        if up {
            a.strike().total_cmp(&b.strike())
        } else {
            b.strike().total_cmp(&a.strike())
        }
    });
    result.candidate_count = metrics.strike_count;
    result.meaningful_candidate_count = meaningful.len();
    result.path_liquidity_score = Some(metrics.average_liquidity_score);
    result.path_openness_score = Some(metrics.openness_score);
    result.path_average_state = path_label(metrics.average_liquidity_score).into();
    result.path_state = result.path_average_state.clone();
    if let Some(r) = meaningful.first() {
        result.barrier_found = true;
        result.path_has_obstacle = true;
        result.barrier_strike = Some(r.strike());
        result.barrier_score = Some(r.liquidity_score);
        result.path_state = format!("OBSTÁCULO {}", strength(r.liquidity_score));
    }
    result.path = Some(metrics);
    result
}
fn pull_score(support: f64, target: f64, open: f64, vacuum: f64) -> f64 {
    round(
        (clamp(support / 100.0) * 0.3
            + clamp(target / 100.0) * 0.35
            + clamp(open / 100.0) * 0.2
            + clamp(vacuum / 100.0) * 0.15)
            * 100.0,
        2,
    )
}
fn pull(
    rows: &[LiquidityRow],
    spot: f64,
    support: &Wall,
    near: &Wall,
    extended: Option<&Target>,
    vacuum: Option<&Vacuum>,
) -> Pull {
    let near_path = path(rows, spot, near.strike);
    let vacuum = vacuum.map_or(0.0, |v| v.vacuum_score);
    let near_score = pull_score(
        support.wall_score,
        near.row.liquidity_score,
        near_path.openness_score,
        vacuum,
    );
    let extended_path = extended.map(|t| path(rows, spot, t.strike));
    let extended_score = extended.zip(extended_path.as_ref()).map(|(t, p)| {
        pull_score(
            support.wall_score,
            t.row.liquidity_score,
            p.openness_score,
            vacuum,
        )
    });
    let use_extended = extended_score.is_some_and(|v| v >= 60.0);
    let preferred = if use_extended {
        &extended.unwrap().row
    } else {
        &near.row
    };
    let score = if use_extended {
        extended_score.unwrap()
    } else {
        near_score
    };
    let path_valid = if support.side == "PUT" {
        support.strike <= spot && preferred.strike() > spot
    } else {
        support.strike >= spot && preferred.strike() < spot
    };
    Pull {
        ready: path_valid
            && support.wall_score >= 60.0
            && preferred.liquidity_score >= 40.0
            && score >= 60.0,
        path_valid,
        near_pull_score: near_score,
        extended_pull_score: extended_score,
        pull_score: score,
        preferred_target: preferred.clone(),
        near_path,
        extended_path,
        support_ready: support.wall_score >= 60.0,
        live_incoming_volume_required: true,
    }
}
pub fn calculate(
    rows: &[Row],
    spot: f64,
    preferred_magnet: Option<f64>,
    complete: bool,
) -> Result<Liquidity, &'static str> {
    if !spot.is_finite() || spot <= 0.0 || rows.len() > 20_000 {
        return Err("Entradas de liquidez inválidas");
    }
    for r in rows {
        if ![
            r.strike,
            r.call_oi,
            r.put_oi,
            r.call_volume,
            r.put_volume,
            r.call_gamma_weight,
            r.put_gamma_weight,
            r.gross_gex,
            r.total_oi,
            r.total_volume,
            r.total_gamma_weight,
            r.dominance,
            r.score,
        ]
        .iter()
        .all(|v| v.is_finite())
            || r.strike <= 0.0
            || r.call_oi_change.is_some_and(|v| !v.is_finite())
            || r.put_oi_change.is_some_and(|v| !v.is_finite())
        {
            return Err("Fila de liquidez inválida");
        }
    }
    if !rows.iter().map(|r| r.total_oi).sum::<f64>().is_finite()
        || !rows.iter().map(|r| r.total_volume).sum::<f64>().is_finite()
    {
        return Err("Suma de liquidez fuera de rango");
    }
    let enriched = enrich(rows, spot);
    let mut result = Liquidity {
        complete: complete && !rows.is_empty(),
        rows: enriched,
        ..Default::default()
    };
    if !result.complete
        || rows
            .iter()
            .all(|r| r.total_oi == 0.0 && r.total_volume == 0.0)
    {
        result.complete = false;
        return Ok(result);
    }
    let rows = &result.rows;
    result.call_wall = wall(rows, spot, true);
    result.put_wall = wall(rows, spot, false);
    result.magnet = preferred_magnet
        .filter(|m| rows.iter().any(|r| r.strike() == *m))
        .or_else(|| {
            let mut ranked: Vec<_> = rows.iter().collect();
            ranked.sort_by(|a, b| {
                b.magnet_score.total_cmp(&a.magnet_score).then_with(|| {
                    (a.strike() - spot)
                        .abs()
                        .total_cmp(&(b.strike() - spot).abs())
                })
            });
            ranked.first().map(|r| r.strike())
        });
    let call = result.call_wall.as_ref().unwrap();
    let put = result.put_wall.as_ref().unwrap();
    [result.upside_target_1, result.upside_target_2] = targets(rows, call.strike, true);
    [result.downside_target_1, result.downside_target_2] = targets(rows, put.strike, false);
    result.upside_vacuum = vacuum(
        rows,
        spot,
        true,
        result
            .upside_target_1
            .as_ref()
            .map_or(call.strike, |t| t.strike),
    );
    result.downside_vacuum = vacuum(
        rows,
        spot,
        false,
        result
            .downside_target_1
            .as_ref()
            .map_or(put.strike, |t| t.strike),
    );
    result.call_wall_context = Some(zone(rows, call));
    result.put_wall_context = Some(zone(rows, put));
    result.upside_liquidity_barrier = Some(barrier(rows, spot, result.magnet, call, true));
    result.downside_liquidity_barrier = Some(barrier(rows, spot, result.magnet, put, false));
    result.call_pull = Some(pull(
        rows,
        spot,
        put,
        call,
        result.upside_target_1.as_ref(),
        result.upside_vacuum.as_ref(),
    ));
    result.put_pull = Some(pull(
        rows,
        spot,
        call,
        put,
        result.downside_target_1.as_ref(),
        result.downside_vacuum.as_ref(),
    ));
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    fn subset(a: &Value, b: &Value, path: &str) {
        match b {
            Value::Object(obj) => {
                for (k, v) in obj {
                    subset(&a[k], v, &format!("{path}.{k}"));
                }
            }
            Value::Array(values) => {
                let actual = a.as_array().unwrap();
                assert_eq!(actual.len(), values.len(), "{path}");
                for (i, (a, b)) in actual.iter().zip(values).enumerate() {
                    subset(a, b, &format!("{path}[{i}]"));
                }
            }
            Value::Number(n) => {
                let av = a.as_f64().unwrap();
                let bv = n.as_f64().unwrap();
                assert!(
                    (av - bv).abs() < 1e-7 * bv.abs().max(1.0),
                    "{path}: {av} != {bv}"
                );
            }
            _ => assert_eq!(a, b, "{path}"),
        }
    }
    #[test]
    fn legacy_walls_targets_vacuums_barriers_and_pull_match() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-liquidity.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let rows: Vec<Row> = serde_json::from_value(case["rows"].clone()).unwrap();
            let actual = calculate(
                &rows,
                case["spot"].as_f64().unwrap(),
                case["preferredMagnet"].as_f64(),
                true,
            )
            .unwrap();
            let value = serde_json::to_value(&actual).unwrap();
            let mut expected = case["expected"].clone();
            // Intentional correction: fallback/ATM walls cannot authorize an impossible path.
            for (key, pull) in [
                ("call_pull", &actual.call_pull),
                ("put_pull", &actual.put_pull),
            ] {
                if pull.as_ref().is_some_and(|p| !p.path_valid) {
                    assert!(!pull.as_ref().unwrap().ready);
                    expected[key]["ready"] = Value::Bool(false);
                }
            }
            if case["name"] == "zero_activity" {
                subset(&value["rows"], &case["expected"]["rows"], "zero rows");
                assert!(!actual.complete && actual.call_wall.is_none());
            } else {
                subset(&value, &expected, case["name"].as_str().unwrap());
            }
        }
    }
    #[test]
    fn incomplete_inputs_never_produce_walls_or_pull() {
        let row = Row {
            strike: 500.0,
            call_oi: 100.0,
            total_oi: 100.0,
            ..Default::default()
        };
        let result = calculate(&[row], 499.0, None, false).unwrap();
        assert!(!result.complete && result.call_wall.is_none() && result.call_pull.is_none());
    }
    #[test]
    fn strong_fallback_walls_on_wrong_side_do_not_make_a_valid_pull() {
        let row = Row {
            strike: 501.0,
            call_oi: 100.0,
            put_oi: 100.0,
            total_oi: 200.0,
            call_volume: 100.0,
            put_volume: 100.0,
            total_volume: 200.0,
            call_gamma_weight: 1.0,
            put_gamma_weight: 1.0,
            total_gamma_weight: 2.0,
            score: 100.0,
            ..Default::default()
        };
        let result = calculate(&[row], 500.0, None, true).unwrap();
        for pull in [result.call_pull.unwrap(), result.put_pull.unwrap()] {
            assert!(pull.pull_score > 60.0);
            assert!(!pull.path_valid && !pull.ready);
        }
    }
}
