//! Legacy score route on normalized, verified inputs. This is analysis only;
//! session, provenance, freshness and structural-path coordination are separate.
use serde::{Deserialize, Serialize};
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Input {
    pub spot: Option<f64>,
    pub call_score: Option<f64>,
    pub put_score: Option<f64>,
    pub confidence: Option<f64>,
    pub side: String,
    pub position_map_confirmation: Option<Position>,
    pub dex_side: String,
    pub dex_bias: Option<f64>,
    pub live_flow_direction: String,
    pub live_flow_bias: Option<f64>,
    pub zero_gamma: Option<f64>,
    pub oi_bias: Option<f64>,
    pub volume_bias: Option<f64>,
    pub dominance_bias: Option<f64>,
    pub upside_target_1: Option<f64>,
    pub upside_target_2: Option<f64>,
    pub downside_target_1: Option<f64>,
    pub downside_target_2: Option<f64>,
    pub call_wall: Option<f64>,
    pub put_wall: Option<f64>,
    pub liquidity_magnet: Option<f64>,
    pub primary_vacuum_direction: String,
}
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Position {
    pub ready: bool,
    pub side: String,
}
#[derive(Default, Serialize)]
pub struct Decision {
    pub decision: String,
    pub primary_side: String,
    pub decision_score: Option<f64>,
    pub directional_score: Option<f64>,
    pub score_gap: Option<f64>,
    pub target_1: Option<f64>,
    pub target_2: Option<f64>,
    pub suggested_stop: Option<f64>,
    pub aligned_count: usize,
    pub conflict_count: usize,
    pub blocks: Vec<String>,
    pub entry_authorized: bool,
}
fn bias_side(bias: Option<f64>) -> &'static str {
    match bias {
        Some(v) if v >= 0.05 => "CALL",
        Some(v) if v <= -0.05 => "PUT",
        _ => "NEUTRAL",
    }
}
fn direction(text: &str, bias: Option<f64>) -> &'static str {
    let text = text.trim().to_ascii_uppercase();
    if ["CALL", "BULL", "BULLISH", "UPSIDE", "UP", "LONG", "BUY"]
        .iter()
        .any(|v| text.contains(v))
    {
        "CALL"
    } else if [
        "PUT", "BEAR", "BEARISH", "DOWNSIDE", "DOWN", "SHORT", "SELL",
    ]
    .iter()
    .any(|v| text.contains(v))
    {
        "PUT"
    } else {
        bias_side(bias)
    }
}
fn directional(side: &str) -> bool {
    matches!(side, "CALL" | "PUT")
}
fn round1(n: f64) -> f64 {
    format!("{n:.1}").parse().unwrap_or(n)
}
pub fn evaluate(i: &Input, verified: bool) -> Decision {
    let mut r = Decision {
        decision: "WAIT".into(),
        primary_side: "WAIT".into(),
        ..Default::default()
    };
    let values = [
        i.spot,
        i.call_score,
        i.put_score,
        i.confidence,
        i.dex_bias,
        i.live_flow_bias,
        i.zero_gamma,
        i.oi_bias,
        i.volume_bias,
        i.dominance_bias,
        i.upside_target_1,
        i.upside_target_2,
        i.downside_target_1,
        i.downside_target_2,
        i.call_wall,
        i.put_wall,
        i.liquidity_magnet,
    ];
    if !verified || values.iter().flatten().any(|v| !v.is_finite()) {
        r.blocks.push("UNVERIFIED_OR_INVALID_INPUT".into());
        return r;
    }
    let Some(spot) = i.spot.filter(|v| *v > 0.0) else {
        r.blocks.push("MISSING_SPOT".into());
        return r;
    };
    let Some((call, put)) = i
        .call_score
        .zip(i.put_score)
        .filter(|(c, p)| (0.0..=100.0).contains(c) && (0.0..=100.0).contains(p))
    else {
        r.blocks.push("INVALID_SCORES".into());
        return r;
    };
    let confidence = i.confidence.unwrap_or(0.0);
    if !(0.0..=100.0).contains(&confidence) {
        r.blocks.push("INVALID_CONFIDENCE".into());
        return r;
    }
    let primary = if call > put {
        "CALL"
    } else if put > call {
        "PUT"
    } else {
        "WAIT"
    };
    r.primary_side = primary.into();
    let base = call.max(put);
    let mut adjusted = base;
    let gap = (call - put).abs();
    let source = i.side.trim().to_ascii_uppercase();
    let source_conflict = directional(&source) && directional(primary) && source != primary;
    if directional(primary) {
        if source == primary {
            r.aligned_count += 1;
        } else if source_conflict {
            r.conflict_count += 1;
        }
    }
    let position_ok = i.position_map_confirmation.as_ref().is_some_and(|p| {
        p.ready
            && (!directional(&p.side.trim().to_ascii_uppercase())
                || p.side.trim().eq_ignore_ascii_case(primary))
    });
    if directional(primary) {
        if position_ok {
            r.aligned_count += 1;
        } else {
            r.conflict_count += 1;
        }
    }
    for side in [
        direction(&i.dex_side, i.dex_bias),
        direction(&i.live_flow_direction, i.live_flow_bias),
    ] {
        if side == primary {
            r.aligned_count += 1;
            adjusted += 5.0;
        } else if directional(side) && directional(primary) {
            r.conflict_count += 1;
            adjusted -= 6.0;
        }
    }
    if let Some(zero) = i.zero_gamma.filter(|_| directional(primary)) {
        if (primary == "CALL" && spot > zero) || (primary == "PUT" && spot < zero) {
            r.aligned_count += 1;
            adjusted += 3.0;
        } else {
            r.conflict_count += 1;
            adjusted -= 3.0;
        }
    }
    let votes = [i.oi_bias, i.volume_bias, i.dominance_bias].map(bias_side);
    let aligned = votes.iter().filter(|&&s| s == primary).count();
    let opposite = votes
        .iter()
        .filter(|&&s| directional(s) && directional(primary) && s != primary)
        .count();
    if aligned >= 2 {
        r.aligned_count += 1;
        adjusted += 2.0;
    } else if aligned == 0 && opposite >= 2 {
        r.conflict_count += 1;
        adjusted -= 2.0;
    }
    if directional(primary) {
        let call = primary == "CALL";
        let mut targets: Vec<f64> = [
            if call {
                i.upside_target_1
            } else {
                i.downside_target_1
            },
            i.liquidity_magnet,
            if call { i.call_wall } else { i.put_wall },
        ]
        .into_iter()
        .flatten()
        .filter(|p| if call { *p > spot } else { *p < spot })
        .collect();
        targets.sort_by(f64::total_cmp);
        targets.dedup();
        if !call {
            targets.reverse();
        }
        r.target_1 = targets.first().copied();
        let mut secondary: Vec<f64> = targets.into_iter().skip(1).collect();
        if let Some(t) = if call {
            i.upside_target_2
        } else {
            i.downside_target_2
        } {
            if if call { t > spot } else { t < spot } {
                secondary.push(t);
            }
        }
        r.target_2 = if call {
            secondary.into_iter().min_by(f64::total_cmp)
        } else {
            secondary.into_iter().max_by(f64::total_cmp)
        };
        let stops = [
            if call { i.put_wall } else { i.call_wall },
            i.zero_gamma,
            if call {
                i.downside_target_1
            } else {
                i.upside_target_1
            },
        ]
        .into_iter()
        .flatten()
        .filter(|p| if call { *p < spot } else { *p > spot });
        r.suggested_stop = if call {
            stops.max_by(f64::total_cmp)
        } else {
            stops.min_by(f64::total_cmp)
        };
    }
    if r.target_1.is_some() {
        r.aligned_count += 1;
        adjusted += 3.0;
    }
    let vacuum = direction(&i.primary_vacuum_direction, None);
    if vacuum == primary {
        r.aligned_count += 1;
        adjusted += 2.0;
    } else if directional(vacuum) && directional(primary) {
        r.conflict_count += 1;
        adjusted -= 2.0;
    }
    let score = round1(adjusted.clamp(0.0, 100.0));
    for (failed, reason) in [
        (primary == "WAIT", "TIED_SCORES"),
        (base < 67.0, "DIRECTIONAL_SCORE"),
        (score < 72.0, "DECISION_SCORE"),
        (gap < 14.0, "SCORE_GAP"),
        (confidence < 62.0, "CONFIDENCE"),
        (r.aligned_count < 3, "CONFIRMATIONS"),
        (r.conflict_count > 2, "CONFLICTS"),
        (source_conflict, "SOURCE_SIDE_CONFLICT"),
        (directional(primary) && !position_ok, "POSITION_MAP"),
    ] {
        if failed {
            r.blocks.push(reason.into());
        }
    }
    r.decision_score = Some(score);
    r.directional_score = Some(round1(base));
    r.score_gap = Some(round1(gap));
    if directional(primary) && r.blocks.is_empty() {
        r.decision = primary.into();
    }
    r
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eighty_legacy_score_cases_match_without_authorizing_entries() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-decision-score.json"))
                .unwrap();
        for case in fixtures["cases"].as_array().unwrap() {
            let input: Input = serde_json::from_value(case["reading"].clone()).unwrap();
            let result = evaluate(&input, true);
            assert!(!result.entry_authorized);
            let actual = serde_json::to_value(result).unwrap();
            for (key, expected) in case["expected"].as_object().unwrap() {
                assert_eq!(&actual[key], expected, "{} {key}", case["name"]);
            }
            assert_eq!(evaluate(&input, false).decision, "WAIT");
        }
        let bad = Input {
            spot: Some(100.0),
            call_score: Some(f64::NAN),
            put_score: Some(30.0),
            ..Default::default()
        };
        assert_eq!(evaluate(&bad, true).decision, "WAIT");
    }
}
