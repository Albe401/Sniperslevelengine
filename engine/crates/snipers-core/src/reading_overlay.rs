//! Legacy reading adjustments; callers must supply independently validated evidence.
use crate::reading::{self, Reading};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adjusted {
    pub reading: Reading,
    pub flow_applied: bool,
    pub trf_applied: bool,
    pub trf_confidence_points: f64,
    pub trf_score_points: f64,
    pub reason: String,
}
fn round(n: f64) -> f64 {
    format!("{n:.1}").parse().expect("finite score")
}
pub fn with_flow(base: &Reading, horizon: &str, flow_bias: Option<f64>) -> Adjusted {
    let mut r = Adjusted {
        reading: base.clone(),
        flow_applied: false,
        trf_applied: false,
        trf_confidence_points: 0.0,
        trf_score_points: 0.0,
        reason: "Solo estructura; evidencia live pendiente".into(),
    };
    if !base.available || horizon != "DAY" {
        return r;
    }
    let Some((structural, flow)) = base
        .raw_bias
        .zip(flow_bias)
        .filter(|(s, f)| s.is_finite() && s.abs() <= 1.0 && f.is_finite() && f.abs() <= 1.0)
    else {
        return r;
    };
    let bias = (structural * 0.75 + flow * 0.25).clamp(-1.0, 1.0);
    let (side, reading, strength) = reading::classify(bias);
    r.reading.side = side.into();
    r.reading.reading = reading.into();
    r.reading.strength = strength.into();
    r.reading.bias = Some((bias * 10000.0).round_ties_even() / 10000.0);
    r.reading.raw_bias = Some(bias);
    r.reading.call_score = Some(round(50.0 + bias * 50.0));
    r.reading.put_score = Some(round(50.0 - bias * 50.0));
    r.reading.confidence = Some(round(50.0 + bias.abs() * 50.0));
    r.flow_applied = true;
    r.reason = "DAY: estructura 75 % y flujo observado 25 %".into();
    r.reading.entry_confirmed = false;
    r
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trf {
    pub confirmation: String,
    pub alignment: String,
    pub strength: String,
    pub pressure_side: String,
    pub pressure_coverage: f64,
    pub confluences: usize,
}
pub fn apply_trf(r: &mut Adjusted, horizon: &str, t: &Trf, verified: bool) {
    if !verified || !r.reading.available {
        return;
    }
    let max: f64 = match horizon {
        "DAY" | "NEXT_SESSION" => 12.0,
        "WEEK" => 6.0,
        "NEXT4" => 3.0,
        _ => return,
    };
    let Some((confidence, (call, put))) = r
        .reading
        .confidence
        .zip(r.reading.call_score.zip(r.reading.put_score))
    else {
        return;
    };
    if [confidence, call, put, t.pressure_coverage]
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=100.0).contains(v))
    {
        return;
    }
    let mut factor: f64 = if t.confirmation.contains("DIVERGEN") {
        -0.75
    } else if t.confirmation.contains("PARCIAL") {
        0.25
    } else if t.confirmation.contains("CONFIRMADO") {
        0.70
    } else {
        0.0
    };
    factor += if t.alignment.contains("FUERTE") {
        0.15
    } else if t.alignment.contains("MEDIA") {
        0.08
    } else if t.alignment.contains("DEBIL") || t.alignment.contains("DÉBIL") {
        -0.05
    } else if t.alignment.contains("DIVERGEN") {
        -0.15
    } else {
        0.0
    };
    if t.pressure_coverage >= 10.0 && matches!(t.pressure_side.as_str(), "CALL" | "PUT") {
        if r.reading.side == t.pressure_side {
            factor += 0.20;
        } else if matches!(r.reading.side.as_str(), "CALL" | "PUT") {
            factor -= 0.30;
        }
    }
    factor += (0.025 * t.confluences as f64).min(0.10);
    factor = factor.clamp(-1.0, 1.0);
    let strength = &t.strength;
    let multiplier = if strength.contains("MUY FUERTE") || strength.contains("VERY STRONG") {
        1.0
    } else if strength.contains("FUERTE") || strength.contains("STRONG") {
        0.9
    } else if strength.contains("MEDIA") || strength.contains("MEDIUM") {
        0.7
    } else if strength.contains("BAJA") || strength.contains("DEBIL") || strength.contains("WEAK") {
        0.5
    } else {
        0.6
    };
    let points = max * factor * multiplier;
    r.reading.confidence = Some(round((confidence + points).clamp(0.0, 100.0)));
    r.trf_confidence_points = round(points);
    let base = match r.reading.side.as_str() {
        "CALL" => Some(call),
        "PUT" => Some(put),
        _ => None,
    };
    if let Some(base) = base {
        let adjusted = (base + points * 0.5).clamp(50.0, 100.0);
        r.trf_score_points = round(adjusted - base);
        let other = 100.0 - adjusted;
        let (call, put) = if r.reading.side == "CALL" {
            (adjusted, other)
        } else {
            (other, adjusted)
        };
        r.reading.call_score = Some(round(call));
        r.reading.put_score = Some(round(put));
    }
    r.trf_applied = true;
    r.reason = "Ajuste TRF verificado; conserva el lado previo".into();
    r.reading.entry_confirmed = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ninety_six_legacy_overlays_preserve_scores_and_side() {
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-overlay.json")).unwrap();
        for c in data["cases"].as_array().unwrap() {
            let b = &c["base"];
            let base = Reading {
                available: true,
                side: b["side"].as_str().unwrap().into(),
                call_score: b["call_score"].as_f64(),
                put_score: b["put_score"].as_f64(),
                confidence: b["confidence"].as_f64(),
                ..Default::default()
            };
            let h = c["horizon"].as_str().unwrap();
            let ctx: Trf = serde_json::from_value(c["context"].clone()).unwrap();
            let mut r = with_flow(&base, h, None);
            apply_trf(&mut r, h, &ctx, true);
            let expected = &c["expected"];
            for (actual, key) in [
                (r.reading.call_score.unwrap(), "callScore"),
                (r.reading.put_score.unwrap(), "putScore"),
                (r.reading.confidence.unwrap(), "confidence"),
                (r.trf_confidence_points, "points"),
                (r.trf_score_points, "scorePoints"),
            ] {
                assert!(
                    (actual - expected[key].as_f64().unwrap()).abs() < 1e-8,
                    "{key}: {actual} != {}, case {c}",
                    expected[key]
                );
            }
            assert_eq!(r.reading.side, expected["side"].as_str().unwrap());
            assert!(!r.reading.entry_confirmed);
            let mut blocked = with_flow(&base, h, None);
            apply_trf(&mut blocked, h, &ctx, false);
            assert!(!blocked.trf_applied);
            assert_eq!(blocked.reading.call_score, base.call_score);
        }
    }
    #[test]
    fn day_flow_uses_unrounded_base_and_other_horizons_remain_structural() {
        let base = Reading {
            available: true,
            raw_bias: Some(0.123456),
            bias: Some(0.1235),
            side: "CALL".into(),
            ..Default::default()
        };
        let r = with_flow(&base, "DAY", Some(-1.0));
        assert_eq!(r.reading.raw_bias, Some(0.123456 * 0.75 - 0.25));
        assert_eq!(r.reading.side, "PUT");
        assert!(r.flow_applied);
        for horizon in ["NEXT_SESSION", "WEEK", "NEXT4"] {
            assert!(!with_flow(&base, horizon, Some(-1.0)).flow_applied);
        }
        for bias in [f64::NAN, f64::INFINITY, 1.001] {
            assert!(!with_flow(&base, "DAY", Some(bias)).flow_applied);
        }
    }
}
