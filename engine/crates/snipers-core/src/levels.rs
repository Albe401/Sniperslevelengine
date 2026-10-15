//! Massive V4 structural scores. Scores measure concentration, never probability.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Contract {
    pub strike: f64,
    pub kind: String,
    pub oi: Option<f64>,
    pub volume: Option<f64>,
    pub gamma: Option<f64>,
    /// Absolute exposure from the same contract universe, or unknown.
    pub gex: Option<f64>,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Components {
    pub call_oi: f64,
    pub put_oi: f64,
    pub oi_change: f64,
    pub volume: f64,
    pub dominance: f64,
    pub sigma: f64,
    pub gex: f64,
}
pub fn weights(history: bool, gex: bool) -> Components {
    let [call_oi, put_oi, oi_change, volume, dominance, sigma, gex] = match (history, gex) {
        (true, true) => [12.5, 12.5, 15.0, 15.0, 15.0, 10.0, 20.0],
        (true, false) => [15.0, 15.0, 15.0, 20.0, 20.0, 15.0, 0.0],
        (false, true) => [15.0, 15.0, 0.0, 20.0, 15.0, 10.0, 25.0],
        (false, false) => [20.0, 20.0, 0.0, 25.0, 20.0, 15.0, 0.0],
    };
    Components {
        call_oi,
        put_oi,
        oi_change,
        volume,
        dominance,
        sigma,
        gex,
    }
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Row {
    pub strike: f64,
    pub call_oi: f64,
    pub put_oi: f64,
    pub call_volume: f64,
    pub put_volume: f64,
    pub call_gamma_weight: f64,
    pub put_gamma_weight: f64,
    pub call_gex: f64,
    pub put_gex: f64,
    pub net_gex: f64,
    pub gross_gex: f64,
    pub total_oi: f64,
    pub total_volume: f64,
    pub total_gamma_weight: f64,
    pub oi_dominance: f64,
    pub volume_dominance: f64,
    pub gamma_dominance: f64,
    pub dominance: f64,
    pub activity: f64,
    pub sigma: f64,
    pub call_oi_strength: f64,
    pub put_oi_strength: f64,
    pub oi_strength: f64,
    pub volume_strength: f64,
    pub gamma_strength: f64,
    pub dominance_strength: f64,
    pub sigma_strength: f64,
    pub gex_strength: f64,
    pub oi_change_strength: f64,
    pub call_oi_change: Option<f64>,
    pub put_oi_change: Option<f64>,
    pub score: f64,
    pub score_weights: Components,
    pub score_breakdown: Components,
    pub dominant_side: String,
    pub classification: String,
    pub distance: f64,
    pub distance_pct: f64,
}
#[derive(Clone, Default, Debug, Serialize)]
pub struct ResultSet {
    pub complete: bool,
    pub missing_contracts: usize,
    pub gex_available: bool,
    pub history_available: bool,
    pub rows: Vec<Row>,
    pub selected: BTreeMap<String, Option<f64>>,
}
fn nonnegative(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite() && *x >= 0.0)
}
fn ratio(value: f64, max: f64) -> f64 {
    if max > 0.0 {
        (value / max).clamp(0.0, 1.0)
    } else {
        0.0
    }
}
fn bias(a: f64, b: f64) -> f64 {
    if a + b > 0.0 {
        (a - b) / (a + b)
    } else {
        0.0
    }
}
fn rounded(value: f64) -> f64 {
    format!("{value:.2}").parse().expect("finite score")
}
pub type OiChange = (f64, Option<f64>, Option<f64>);

pub fn calculate(
    contracts: &[Contract],
    spot: f64,
    changes: &[OiChange],
    history: bool,
    use_gex: bool,
) -> Result<ResultSet, &'static str> {
    if !spot.is_finite() || spot <= 0.0 || contracts.len() > 20_000 {
        return Err("Entradas de niveles inválidas");
    }
    let mut result = ResultSet {
        complete: !contracts.is_empty(),
        history_available: history,
        ..Default::default()
    };
    let mut positions = BTreeMap::new();
    let mut gex_complete = use_gex && !contracts.is_empty();
    for c in contracts {
        if !c.strike.is_finite() || c.strike <= 0.0 || !["call", "put"].contains(&c.kind.as_str()) {
            return Err("Contrato de niveles inválido");
        }
        let oi = nonnegative(c.oi);
        let volume = nonnegative(c.volume);
        let gex = nonnegative(c.gex);
        if oi.is_none() || volume.is_none() {
            result.complete = false;
            result.missing_contracts += 1;
        }
        if gex.is_none() {
            gex_complete = false;
        }
        let next = result.rows.len();
        let index = *positions.entry(c.strike.to_bits()).or_insert(next);
        if index == next {
            result.rows.push(Row {
                strike: c.strike,
                ..Default::default()
            });
        }
        let row = &mut result.rows[index];
        let oi = oi.unwrap_or(0.0);
        let volume = volume.unwrap_or(0.0);
        let gamma = nonnegative(c.gamma).unwrap_or(0.0) * oi;
        if c.kind == "call" {
            row.call_oi += oi;
            row.call_volume += volume;
            row.call_gamma_weight += gamma;
            row.call_gex += gex.unwrap_or(0.0);
        } else {
            row.put_oi += oi;
            row.put_volume += volume;
            row.put_gamma_weight += gamma;
            row.put_gex += gex.unwrap_or(0.0);
        }
    }
    result.gex_available = gex_complete;
    let mut max_call: f64 = 0.0;
    let mut max_put: f64 = 0.0;
    let mut max_oi: f64 = 0.0;
    let mut max_volume: f64 = 0.0;
    let mut max_gamma: f64 = 0.0;
    let mut max_gex: f64 = 0.0;
    let mut max_change: f64 = 0.0;
    let mut activities = vec![];
    for row in &mut result.rows {
        row.total_oi = row.call_oi + row.put_oi;
        row.total_volume = row.call_volume + row.put_volume;
        row.total_gamma_weight = row.call_gamma_weight + row.put_gamma_weight;
        row.oi_dominance = bias(row.call_oi, row.put_oi);
        row.volume_dominance = bias(row.call_volume, row.put_volume);
        row.gamma_dominance = bias(row.call_gamma_weight, row.put_gamma_weight);
        row.dominance = row.oi_dominance * 0.6 + row.volume_dominance * 0.4;
        row.activity = row.total_oi.ln_1p() * 0.6 + row.total_volume.ln_1p() * 0.4;
        if !gex_complete {
            row.call_gex = 0.0;
            row.put_gex = 0.0;
        }
        row.gross_gex = row.call_gex + row.put_gex;
        row.net_gex = row.call_gex - row.put_gex;
        if history {
            if let Some((_, call, put)) =
                changes.iter().find(|(strike, _, _)| *strike == row.strike)
            {
                row.call_oi_change = call.filter(|c| c.is_finite());
                row.put_oi_change = put.filter(|c| c.is_finite());
            }
        }
        let change = row
            .call_oi_change
            .zip(row.put_oi_change)
            .map_or(0.0, |(c, p)| c.abs() + p.abs());
        max_change = max_change.max(change);
        max_call = max_call.max(row.call_oi);
        max_put = max_put.max(row.put_oi);
        max_oi = max_oi.max(row.total_oi);
        max_volume = max_volume.max(row.total_volume);
        max_gamma = max_gamma.max(row.total_gamma_weight);
        max_gex = max_gex.max(row.gross_gex);
        if ![
            row.total_oi,
            row.total_volume,
            row.total_gamma_weight,
            row.gross_gex,
            change,
            row.activity,
        ]
        .iter()
        .all(|v| v.is_finite())
        {
            return Err("Suma de niveles fuera de rango");
        }
        activities.push(row.activity);
    }
    let mean = if activities.is_empty() {
        0.0
    } else {
        activities.iter().sum::<f64>() / activities.len() as f64
    };
    let std = if activities.len() < 2 {
        0.0
    } else {
        (activities.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / activities.len() as f64)
            .sqrt()
    };
    for row in &mut result.rows {
        row.sigma = if std > 0.0 {
            (row.activity - mean) / std
        } else {
            0.0
        };
        row.call_oi_strength = ratio(row.call_oi, max_call);
        row.put_oi_strength = ratio(row.put_oi, max_put);
        row.oi_strength = ratio(row.total_oi, max_oi);
        row.volume_strength = ratio(row.total_volume, max_volume);
        row.gamma_strength = ratio(row.total_gamma_weight, max_gamma);
        row.dominance_strength = row.dominance.abs().clamp(0.0, 1.0);
        row.sigma_strength = ratio(row.sigma.max(0.0), 3.0);
        row.gex_strength = ratio(row.gross_gex, max_gex);
        row.oi_change_strength = ratio(
            row.call_oi_change
                .zip(row.put_oi_change)
                .map_or(0.0, |(c, p)| c.abs() + p.abs()),
            max_change,
        );
        let w = weights(history, gex_complete);
        let points = Components {
            call_oi: row.call_oi_strength * w.call_oi,
            put_oi: row.put_oi_strength * w.put_oi,
            oi_change: row.oi_change_strength * w.oi_change,
            volume: row.volume_strength * w.volume,
            dominance: row.dominance_strength * w.dominance,
            sigma: row.sigma_strength * w.sigma,
            gex: row.gex_strength * w.gex,
        };
        row.score = rounded(
            (points.call_oi
                + points.put_oi
                + points.oi_change
                + points.volume
                + points.dominance
                + points.sigma
                + points.gex)
                .clamp(0.0, 100.0),
        );
        row.score_weights = w;
        row.score_breakdown = points;
        row.dominant_side = if row.dominance >= 0.15 {
            "CALL"
        } else if row.dominance <= -0.15 {
            "PUT"
        } else {
            "BALANCED"
        }
        .into();
        row.classification = if row.strike > spot {
            if row.dominance >= 0.15 {
                "RESISTANCE"
            } else {
                "UPPER MAGNET"
            }
        } else if row.strike < spot {
            if row.dominance <= -0.15 {
                "SUPPORT"
            } else {
                "LOWER MAGNET"
            }
        } else {
            "ATM MAGNET"
        }
        .into();
        row.distance = row.strike - spot;
        row.distance_pct = row.distance / spot * 100.0;
        if !row.distance_pct.is_finite() {
            return Err("Distancia de nivel fuera de rango");
        }
    }
    for (prefix, class) in [("R", "RESISTANCE"), ("S", "SUPPORT"), ("M", "MAGNET")] {
        let mut ranked: Vec<_> = result
            .rows
            .iter()
            .filter(|r| {
                result.complete
                    && (max_oi > 0.0 || max_volume > 0.0)
                    && r.classification.contains(class)
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| {
                    if prefix == "M" {
                        std::cmp::Ordering::Equal
                    } else {
                        b.dominance.abs().total_cmp(&a.dominance.abs())
                    }
                })
                .then_with(|| a.distance.abs().total_cmp(&b.distance.abs()))
        });
        ranked.truncate(3);
        if prefix == "R" {
            ranked.sort_by(|a, b| a.strike.total_cmp(&b.strike));
        }
        if prefix == "S" {
            ranked.sort_by(|a, b| b.strike.total_cmp(&a.strike));
        }
        for i in 0..3 {
            result.selected.insert(
                format!("{prefix}{}", i + 1),
                ranked.get(i).map(|r| r.strike),
            );
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    #[test]
    fn twenty_legacy_scoring_and_ranking_cases_match() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-levels.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let contracts: Vec<_> = case["contracts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| Contract {
                    strike: c["strike"].as_f64().unwrap(),
                    kind: c["type"].as_str().unwrap().into(),
                    oi: c["open_interest"].as_f64(),
                    volume: c["volume"].as_f64(),
                    gamma: c["gamma"].as_f64(),
                    gex: Some(
                        c["gamma"].as_f64().unwrap()
                            * c["open_interest"].as_f64().unwrap()
                            * 100.0
                            * 500.0
                            * 500.0
                            * 0.01,
                    ),
                })
                .collect();
            let changes: Vec<_> = case["changes"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(s, v)| {
                    (
                        s.parse::<f64>().unwrap(),
                        v["call"].as_f64(),
                        v["put"].as_f64(),
                    )
                })
                .collect();
            let result = calculate(
                &contracts,
                500.0,
                &changes,
                case["history"].as_bool().unwrap(),
                case["gex"].as_bool().unwrap(),
            )
            .unwrap();
            for row in &result.rows {
                let expected = case["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["strike"].as_f64() == Some(row.strike))
                    .unwrap();
                let actual = serde_json::to_value(row).unwrap();
                for key in [
                    "call_oi",
                    "put_oi",
                    "total_volume",
                    "dominance",
                    "sigma",
                    "score",
                    "gex_strength",
                    "oi_change_strength",
                ] {
                    let a = actual[key].as_f64().unwrap();
                    let b = expected[key].as_f64().unwrap();
                    assert!(
                        (a - b).abs() <= 1e-8 * b.abs().max(1.0),
                        "{} {key}: {a} != {b}",
                        case["name"]
                    );
                }
                assert_eq!(actual["classification"], expected["classification"]);
            }
            if case["name"] != "zero_activity" {
                assert_eq!(
                    serde_json::to_value(result.selected).unwrap(),
                    case["selected"],
                    "{}",
                    case["name"]
                );
            } else {
                assert!(result.selected.values().all(Option::is_none));
            }
        }
    }
    #[test]
    fn missing_activity_and_overflow_cannot_produce_certified_levels() {
        let mut c = Contract {
            strike: 500.0,
            kind: "call".into(),
            oi: None,
            volume: Some(20.0),
            gamma: None,
            gex: None,
        };
        let result = calculate(&[c.clone()], 499.0, &[], false, true).unwrap();
        assert!(!result.complete && !result.gex_available);
        assert!(result.selected.values().all(Option::is_none));
        c.oi = Some(f64::MAX);
        c.gamma = Some(10.0);
        assert!(calculate(&[c], 499.0, &[], false, false).is_err());
    }
}
