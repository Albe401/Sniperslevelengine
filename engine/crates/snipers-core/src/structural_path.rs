//! Normalized Magnet -> Vacuum -> Wall candidate. Never an admission permission.
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Input {
    pub side: String,
    pub magnet: Option<f64>,
    pub wall: Option<f64>,
    pub wall_score: Option<f64>,
    pub wall_strength: String,
    pub vacuum_low: Option<f64>,
    pub vacuum_high: Option<f64>,
    pub vacuum_confirmed: bool,
    pub trigger_ready: bool,
    pub entry: Option<f64>,
    pub tolerance: Option<f64>,
    pub volume_ratio: Option<f64>,
    pub flow_transport_ready: bool,
    pub flow_side: String,
    pub buy_contracts: f64,
    pub rvol_ready: bool,
    pub option_liquidity_ready: bool,
    pub position_ready: bool,
    pub position_side: String,
    pub barrier: Option<f64>,
    pub barrier_score: Option<f64>,
    pub barrier_strength: String,
}
#[derive(Clone, Default, Serialize)]
pub struct Candidate {
    pub available: bool,
    pub ready: bool,
    pub side: String,
    pub invalidation_level: Option<f64>,
    pub risk_reward: Option<f64>,
    pub flow_ready: bool,
    pub participation_ready: bool,
    pub position_map_direct_conflict: bool,
    pub barrier_blocked: bool,
    pub blocks: Vec<String>,
    pub entry_authorized: bool,
}
pub fn evaluate(i: &Input, verified: bool) -> Candidate {
    let mut r = Candidate {
        side: i.side.clone(),
        ..Default::default()
    };
    let nums = [
        i.magnet,
        i.wall,
        i.wall_score,
        i.vacuum_low,
        i.vacuum_high,
        i.entry,
        i.tolerance,
        i.volume_ratio,
        i.barrier,
        i.barrier_score,
        Some(i.buy_contracts),
    ];
    if !verified
        || !matches!(i.side.as_str(), "CALL" | "PUT")
        || nums.into_iter().flatten().any(|v| !v.is_finite())
    {
        r.blocks.push("UNVERIFIED_INPUT".into());
        return r;
    }
    let Some(((magnet, wall), (low, high))) =
        i.magnet.zip(i.wall).zip(i.vacuum_low.zip(i.vacuum_high))
    else {
        r.blocks.push("MISSING_GEOMETRY".into());
        return r;
    };
    let call = i.side == "CALL";
    let center = low / 2.0 + high / 2.0;
    let strong = matches!(i.wall_strength.as_str(), "HIGH" | "EXTREME")
        || i.wall_score.is_some_and(|v| v >= 55.0);
    if ![magnet, wall, low, high].into_iter().all(|v| v > 0.0)
        || low > high
        || !i.vacuum_confirmed
        || !strong
        || !(if call {
            magnet < center && center < wall
        } else {
            wall < center && center < magnet
        })
    {
        r.blocks.push("INCOMPLETE_MAGNET_VACUUM_WALL".into());
        return r;
    }
    r.available = true;
    let tolerance = i.tolerance.unwrap_or(0.0);
    let stop = if call {
        magnet - tolerance
    } else {
        magnet + tolerance
    };
    if !stop.is_finite() || stop <= 0.0 || tolerance < 0.0 {
        r.blocks.push("INVALID_STOP".into());
        return r;
    }
    r.invalidation_level = Some(stop);
    let rr = i.entry.filter(|v| *v > 0.0).and_then(|entry| {
        let risk = if call { entry - stop } else { stop - entry };
        let reward = if call { wall - entry } else { entry - wall };
        (risk > 0.0)
            .then_some(reward / risk)
            .filter(|v| v.is_finite())
    });
    r.risk_reward = rr.map(|v| format!("{v:.3}").parse().unwrap_or(v));
    r.flow_ready = i.flow_transport_ready && i.flow_side == i.side && i.buy_contracts > 0.0;
    r.participation_ready = i.rvol_ready || i.volume_ratio.is_some_and(|v| v >= 1.15);
    r.position_map_direct_conflict = i.position_ready
        && matches!(i.position_side.as_str(), "CALL" | "PUT")
        && i.position_side != i.side;
    let barrier_strong = matches!(i.barrier_strength.as_str(), "HIGH" | "EXTREME")
        || i.barrier_score.is_some_and(|v| v >= 55.0);
    r.barrier_blocked = barrier_strong
        && i.entry.zip(i.barrier).is_some_and(|(entry, b)| {
            if call {
                entry < b && b < wall
            } else {
                wall < b && b < entry
            }
        });
    for (failed, reason) in [
        (!i.trigger_ready, "MAGNET_2M_TRIGGER"),
        (!r.flow_ready, "LIVE_FLOW"),
        (!r.participation_ready, "PARTICIPATION"),
        (!i.option_liquidity_ready, "OPTION_LIQUIDITY"),
        (r.barrier_blocked, "INTERMEDIATE_BARRIER"),
        (r.position_map_direct_conflict, "POSITION_CONFLICT"),
        (rr.is_none_or(|v| v < 1.8), "RISK_REWARD"),
    ] {
        if failed {
            r.blocks.push(reason.into());
        }
    }
    r.ready = r.blocks.is_empty();
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_structural_candidates_preserve_gates_without_entry_permission() {
        let fixtures: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/legacy-structural-path.json"
        ))
        .unwrap();
        for case in fixtures["cases"].as_array().unwrap() {
            let input: Input = serde_json::from_value(case["input"].clone()).unwrap();
            let result = evaluate(&input, true);
            assert!(!result.entry_authorized);
            let actual = serde_json::to_value(result).unwrap();
            for (key, value) in case["expected"].as_object().unwrap() {
                assert_eq!(&actual[key], value, "{} {key}", case["name"]);
            }
            assert!(!evaluate(&input, false).ready);
        }
        let input = Input {
            side: "CALL".into(),
            magnet: Some(f64::NAN),
            ..Default::default()
        };
        assert!(!evaluate(&input, true).ready);
    }
}
