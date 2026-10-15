//! Pure legacy per-contract and aggregated scoring. CME transport is separate.
//! Sigma order matters for ties; callers must supply an ordered list.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
pub struct StrikeData {
    pub strike: f64,
    pub option_type: String,
    pub open_interest: f64,
    pub oi_change: f64,
    pub volume: f64,
    pub most_active: bool,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Aggregate {
    pub strike: f64,
    pub call_oi: f64,
    pub put_oi: f64,
    pub call_oi_change: f64,
    pub put_oi_change: f64,
    pub call_volume: f64,
    pub put_volume: f64,
    pub combined_oi: f64,
    pub combined_oi_change: f64,
    pub combined_volume: f64,
    pub dominant_type: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Direct {
    pub strike: f64,
    #[serde(rename = "type")]
    pub kind: String,
    pub open_interest: f64,
    pub oi_change: f64,
    pub volume: f64,
    pub oi_change_points: f64,
    pub active_points: f64,
    pub option_points: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Combined {
    #[serde(flatten)]
    pub aggregate: Aggregate,
    pub change_points: f64,
    pub directional_points: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Scored<T> {
    #[serde(flatten)]
    pub data: T,
    pub score: f64,
    pub oi_points: f64,
    pub volume_points: f64,
    pub sigma_points: f64,
    pub sigma_match: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ResultSet<T> {
    pub reference_price: f64,
    pub resistances: Vec<Scored<T>>,
    pub supports: Vec<Scored<T>>,
    pub all_levels: Vec<Scored<T>>,
}
fn round(v: f64) -> f64 {
    format!("{v:.2}").parse().expect("finite score")
}
fn normalized(v: f64, max: f64, weight: f64) -> f64 {
    if max > 0.0 {
        v / max * weight
    } else {
        0.0
    }
}
fn validate(
    rows: &[StrikeData],
    spot: f64,
    sigma: &[(String, Option<f64>)],
    tolerance: f64,
) -> Result<(), &'static str> {
    if rows.len() > 20_000
        || sigma.len() > 64
        || !spot.is_finite()
        || spot <= 0.0
        || !tolerance.is_finite()
        || tolerance < 0.0
        || sigma
            .iter()
            .any(|(_, v)| v.is_some_and(|v| !v.is_finite() || v <= 0.0))
        || rows.iter().any(|r| {
            !r.strike.is_finite()
                || r.strike <= 0.0
                || !r.open_interest.is_finite()
                || r.open_interest < 0.0
                || !r.volume.is_finite()
                || r.volume < 0.0
                || !r.oi_change.is_finite()
                || !["C", "P", "X"].contains(&r.option_type.to_uppercase().as_str())
        })
    {
        return Err("Entrada de niveles inválida o fuera de límites");
    }
    // Validate totals before aggregation, including opposing changes that could cancel.
    let total: f64 = rows
        .iter()
        .map(|r| r.open_interest + r.volume + r.oi_change.abs())
        .sum();
    if !total.is_finite() {
        return Err("Desbordamiento en niveles");
    }
    Ok(())
}
pub fn aggregate(rows: &[StrikeData]) -> Result<Vec<Aggregate>, &'static str> {
    validate(rows, 1.0, &[], 0.0)?;
    let mut grouped: BTreeMap<u64, Aggregate> = BTreeMap::new();
    for r in rows {
        let a = grouped
            .entry(r.strike.to_bits())
            .or_insert_with(|| Aggregate {
                strike: r.strike,
                ..Default::default()
            });
        // The original aggregator is case sensitive; the direct scorer is not.
        match r.option_type.as_str() {
            "C" => {
                a.call_oi += r.open_interest;
                a.call_oi_change += r.oi_change;
                a.call_volume += r.volume;
            }
            "P" => {
                a.put_oi += r.open_interest;
                a.put_oi_change += r.oi_change;
                a.put_volume += r.volume;
            }
            _ => {}
        }
    }
    Ok(grouped
        .into_values()
        .map(|mut a| {
            a.combined_oi = a.call_oi + a.put_oi;
            a.combined_oi_change = a.call_oi_change + a.put_oi_change;
            a.combined_volume = a.call_volume + a.put_volume;
            a.dominant_type =
                match (a.call_oi + a.call_volume).total_cmp(&(a.put_oi + a.put_volume)) {
                    std::cmp::Ordering::Greater => "C",
                    std::cmp::Ordering::Less => "P",
                    _ => "X",
                }
                .into();
            a
        })
        .collect())
}
fn sigma_score(
    strike: f64,
    sigma: &[(String, Option<f64>)],
    tolerance: f64,
) -> (f64, Option<String>) {
    let mut best = (0.0, None);
    for (name, price) in sigma {
        let weight = match name.as_str() {
            "+1" | "-1" => 10.0,
            "+2" | "-2" => 7.0,
            "+3" | "-3" => 4.0,
            _ => 0.0,
        };
        if price.is_some_and(|v| (strike - v).abs() <= tolerance) && weight > best.0 {
            best = (weight, Some(name.clone()));
        }
    }
    best
}
fn select<T: Clone>(
    mut all: Vec<Scored<T>>,
    spot: f64,
    strike: impl Fn(&T) -> f64,
) -> ResultSet<T> {
    let mut sides = [vec![], vec![]];
    for (side, indices) in sides.iter_mut().enumerate() {
        *indices = (0..all.len())
            .filter(|&i| {
                if side == 0 {
                    strike(&all[i].data) > spot
                } else {
                    strike(&all[i].data) < spot
                }
            })
            .collect();
        indices.sort_by(|&a, &b| all[b].score.total_cmp(&all[a].score));
        indices.truncate(3);
        indices.sort_by(|&a, &b| {
            if side == 0 {
                strike(&all[a].data).total_cmp(&strike(&all[b].data))
            } else {
                strike(&all[b].data).total_cmp(&strike(&all[a].data))
            }
        });
        for (rank, &index) in indices.iter().enumerate() {
            all[index].name = Some(format!("{}{}", if side == 0 { "R" } else { "S" }, rank + 1));
        }
    }
    ResultSet {
        reference_price: spot,
        resistances: sides[0].iter().map(|&i| all[i].clone()).collect(),
        supports: sides[1].iter().map(|&i| all[i].clone()).collect(),
        all_levels: all,
    }
}
pub fn direct(
    rows: &[StrikeData],
    spot: f64,
    sigma: &[(String, Option<f64>)],
    tolerance: f64,
) -> Result<ResultSet<Direct>, &'static str> {
    validate(rows, spot, sigma, tolerance)?;
    let max_oi = rows.iter().map(|r| r.open_interest).fold(0.0, f64::max);
    let max_vol = rows.iter().map(|r| r.volume).fold(0.0, f64::max);
    let max_pos = rows.iter().map(|r| r.oi_change).fold(0.0, f64::max);
    let max_neg = rows.iter().map(|r| -r.oi_change).fold(0.0, f64::max);
    let all = rows
        .iter()
        .map(|r| {
            let oi = normalized(r.open_interest, max_oi, 25.0);
            let vol = normalized(r.volume, max_vol, 20.0);
            let change = if r.oi_change > 0.0 {
                normalized(r.oi_change, max_pos, 25.0)
            } else {
                -normalized(r.oi_change.abs(), max_neg, 15.0)
            };
            let active = if r.most_active { 10.0 } else { 0.0 };
            let kind = r.option_type.to_uppercase();
            let option = if (kind == "C" && r.strike > spot) || (kind == "P" && r.strike < spot) {
                10.0
            } else if kind == "X" {
                5.0
            } else {
                0.0
            };
            let (sp, sm) = sigma_score(r.strike, sigma, tolerance);
            Scored {
                data: Direct {
                    strike: r.strike,
                    kind,
                    open_interest: r.open_interest,
                    oi_change: r.oi_change,
                    volume: r.volume,
                    oi_change_points: round(change),
                    active_points: active,
                    option_points: option,
                },
                score: round((oi + change + vol + active + sp + option).clamp(0.0, 100.0)),
                oi_points: round(oi),
                volume_points: round(vol),
                sigma_points: sp,
                sigma_match: sm,
                name: None,
            }
        })
        .collect();
    Ok(select(all, spot, |r| r.strike))
}
pub fn combined(
    rows: &[StrikeData],
    spot: f64,
    sigma: &[(String, Option<f64>)],
    tolerance: f64,
) -> Result<ResultSet<Combined>, &'static str> {
    validate(rows, spot, sigma, tolerance)?;
    let aggregated = aggregate(rows)?;
    let max_oi = aggregated.iter().map(|r| r.combined_oi).fold(0.0, f64::max);
    let max_vol = aggregated
        .iter()
        .map(|r| r.combined_volume)
        .fold(0.0, f64::max);
    let max_pos = aggregated
        .iter()
        .map(|r| r.combined_oi_change)
        .fold(0.0, f64::max);
    let all = aggregated
        .into_iter()
        .map(|r| {
            let oi = normalized(r.combined_oi, max_oi, 25.0);
            let vol = normalized(r.combined_volume, max_vol, 20.0);
            let change = if r.combined_oi_change > 0.0 {
                normalized(r.combined_oi_change, max_pos, 20.0)
            } else if r.combined_oi_change < 0.0 {
                -10.0
            } else {
                0.0
            };
            let call = r.call_oi + r.call_volume;
            let put = r.put_oi + r.put_volume;
            let directional = if call + put > 0.0 && r.strike != spot {
                ((if r.strike > spot {
                    call - put
                } else {
                    put - call
                }) / (call + put))
                    .max(0.0)
                    * 25.0
            } else {
                0.0
            };
            let (sp, sm) = sigma_score(r.strike, sigma, tolerance);
            Scored {
                data: Combined {
                    aggregate: r,
                    change_points: round(change),
                    directional_points: round(directional),
                },
                score: round((oi + vol + change + directional + sp).clamp(0.0, 100.0)),
                oi_points: round(oi),
                volume_points: round(vol),
                sigma_points: sp,
                sigma_match: sm,
                name: None,
            }
        })
        .collect();
    Ok(select(all, spot, |r| r.aggregate.strike))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    fn compare(actual: &Value, expected: &Value) {
        match (actual, expected) {
            (Value::Number(a), Value::Number(b)) => assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-8,
                "{actual} != {expected}"
            ),
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len());
                for (a, b) in a.iter().zip(b) {
                    compare(a, b);
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                assert_eq!(a.len(), b.len());
                for (key, b) in b {
                    compare(a.get(key).unwrap_or_else(|| panic!("Missing {key}")), b);
                }
            }
            _ => assert_eq!(actual, expected),
        }
    }
    #[test]
    fn legacy_parity_scoring_aggregation_sigma_and_ranking() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-generic-levels.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let rows: Vec<StrikeData> = serde_json::from_value(case["rows"].clone()).unwrap();
            let sigma: Vec<(String, Option<f64>)> =
                serde_json::from_value(case["sigma"].clone()).unwrap();
            let tolerance = case["tolerance"].as_f64().unwrap();
            compare(
                &serde_json::to_value(aggregate(&rows).unwrap()).unwrap(),
                &case["aggregated"],
            );
            let mut d =
                serde_json::to_value(direct(&rows, 500.0, &sigma, tolerance).unwrap()).unwrap();
            let all = d.as_object_mut().unwrap().remove("all_levels").unwrap();
            d["all_strikes"] = all;
            compare(&d, &case["direct"]);
            compare(
                &serde_json::to_value(combined(&rows, 500.0, &sigma, tolerance).unwrap()).unwrap(),
                &case["combined"],
            );
        }
    }
    #[test]
    fn rejects_unknown_and_overflow_instead_of_certifying_zero() {
        let mut row = StrikeData {
            strike: 500.0,
            option_type: "C".into(),
            open_interest: 100.0,
            oi_change: 0.0,
            volume: 20.0,
            most_active: false,
        };
        row.open_interest = f64::NAN;
        assert!(direct(&[row.clone()], 500.0, &[], 25.0).is_err());
        row.open_interest = f64::MAX;
        row.volume = f64::MAX;
        assert!(combined(&[row], 500.0, &[], 25.0).is_err());
        assert!(direct(&[], 500.0, &[("+1".into(), Some(f64::INFINITY))], 25.0).is_err());
    }
}
