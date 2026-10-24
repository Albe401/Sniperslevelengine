//! Structural snapshot context only. Live-flow and TRF overlays are separate layers.
use crate::levels::Row;
use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    #[serde(skip)]
    pub raw_bias: Option<f64>,
    pub available: bool,
    pub side: String,
    pub reading: String,
    pub strength: String,
    pub reason: String,
    pub bias: Option<f64>,
    pub call_score: Option<f64>,
    pub put_score: Option<f64>,
    pub confidence: Option<f64>,
    pub oi_bias: Option<f64>,
    pub volume_bias: Option<f64>,
    pub dominance_bias: Option<f64>,
    pub gex_bias: Option<f64>,
    pub entry_confirmed: bool,
}
pub fn classify(bias: f64) -> (&'static str, &'static str, &'static str) {
    if bias >= 0.18 {
        ("CALL", "ALCISTA CALL", "STRONG")
    } else if bias >= 0.07 {
        ("CALL", "LIGERO SESGO CALL", "MODERATE")
    } else if bias <= -0.18 {
        ("PUT", "BAJISTA PUT", "STRONG")
    } else if bias <= -0.07 {
        ("PUT", "LIGERO SESGO PUT", "MODERATE")
    } else {
        ("NEUTRAL", "NEUTRAL / MIXTO", "NEUTRAL")
    }
}
pub fn calculate(rows: &[Row], complete: bool, call_gex: f64, put_gex: f64) -> Reading {
    let waiting = |reason: &str| Reading {
        side: "N/A".into(),
        reading: "Esperando datos".into(),
        strength: "WAITING".into(),
        reason: reason.into(),
        ..Default::default()
    };
    if !complete || rows.is_empty() || rows.len() > 20_000 {
        return waiting("Cobertura estructural incompleta");
    }
    let mut totals = [0.0; 6];
    for r in rows {
        let inputs = [r.call_oi, r.put_oi, r.call_volume, r.put_volume];
        if inputs.iter().any(|n| !n.is_finite() || *n < 0.0)
            || !r.dominance.is_finite()
            || r.dominance.abs() > 1.0
        {
            return waiting("Datos estructurales inválidos");
        }
        let activity = inputs.iter().sum::<f64>();
        for (total, n) in totals.iter_mut().zip(inputs) {
            *total += n;
        }
        totals[4] += r.dominance * activity;
        totals[5] += activity;
    }
    if totals.iter().any(|n| !n.is_finite())
        || !call_gex.is_finite()
        || !put_gex.is_finite()
        || call_gex < 0.0
        || put_gex < 0.0
        || !(call_gex + put_gex).is_finite()
        || !(totals[0] + totals[1]).is_finite()
        || !(totals[2] + totals[3]).is_finite()
    {
        return waiting("Exposición inválida o desbordamiento");
    }
    if totals[0]+totals[1]==0.0 && totals[2]+totals[3]==0.0 {
        return waiting("Sin interés abierto ni volumen para una lectura estructural");
    }
    let ratio = |c: f64, p: f64| if c + p > 0.0 { (c - p) / (c + p) } else { 0.0 };
    let oi = ratio(totals[0], totals[1]);
    let volume = ratio(totals[2], totals[3]);
    let dominance = if totals[5] > 0.0 {
        totals[4] / totals[5]
    } else {
        0.0
    };
    let gex = ratio(call_gex, put_gex);
    let bias = (oi * 0.4 + volume * 0.25 + dominance * 0.2 + gex * 0.15).clamp(-1.0, 1.0);
    let (side, reading, strength) = classify(bias);
    let round = |n: f64, d: f64| (n * d).round_ties_even() / d;
    Reading {
        available: true,
        raw_bias: Some(bias),
        side: side.into(),
        reading: reading.into(),
        strength: strength.into(),
        reason: "Contexto estructural del snapshot; sin overlay de flujo ni TRF".into(),
        bias: Some(round(bias, 10000.0)),
        call_score: Some(round(50.0 + bias * 50.0, 10.0)),
        put_score: Some(round(50.0 - bias * 50.0, 10.0)),
        confidence: Some(round(50.0 + bias.abs() * 50.0, 10.0)),
        oi_bias: Some(round(oi, 10000.0)),
        volume_bias: Some(round(volume, 10000.0)),
        dominance_bias: Some(round(dominance, 10000.0)),
        gex_bias: Some(round(gex, 10000.0)),
        entry_confirmed: false,
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alignment {
    pub status: String,
    pub side: String,
    pub call_count: usize,
    pub put_count: usize,
    pub neutral_count: usize,
    pub unavailable_count: usize,
}
pub fn align(sides: [&str; 3]) -> Alignment {
    let normalized = sides.map(|s| s.trim().to_ascii_uppercase());
    let count = |name: &str| normalized.iter().filter(|s| *s == name).count();
    let (c, p, n) = (count("CALL"), count("PUT"), count("NEUTRAL"));
    let u = 3 - c - p - n;
    let (status, side) = if c == 3 {
        ("FULL ALIGNMENT", "CALL")
    } else if p == 3 {
        ("FULL ALIGNMENT", "PUT")
    } else if c > 0 && p > 0 {
        ("MIXED", "NEUTRAL")
    } else if c >= 2 {
        ("PARTIAL ALIGNMENT", "CALL")
    } else if p >= 2 {
        ("PARTIAL ALIGNMENT", "PUT")
    } else if n >= 2 {
        ("NEUTRAL", "NEUTRAL")
    } else if u >= 2 {
        ("INSUFFICIENT DATA", "N/A")
    } else {
        ("NEUTRAL / MIXED", "NEUTRAL")
    };
    Alignment {
        status: status.into(),
        side: side.into(),
        call_count: c,
        put_count: p,
        neutral_count: n,
        unavailable_count: u,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_readings_all_alignment_combinations_and_thresholds_match() {
        let f: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-reading.json")).unwrap();
        for case in f["cases"].as_array().unwrap() {
            let rows: Vec<Row> = case["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| Row {
                    call_oi: r["call_oi"].as_f64().unwrap(),
                    put_oi: r["put_oi"].as_f64().unwrap(),
                    call_volume: r["call_volume"].as_f64().unwrap(),
                    put_volume: r["put_volume"].as_f64().unwrap(),
                    dominance: r["dominance"].as_f64().unwrap(),
                    ..Default::default()
                })
                .collect();
            let r = calculate(
                &rows,
                true,
                case["callGex"].as_f64().unwrap(),
                case["putGex"].as_f64().unwrap(),
            );
            let e = &case["expected"];
            if rows.iter().all(|r|r.call_oi+r.put_oi+r.call_volume+r.put_volume==0.0) {
                assert!(!r.available && r.call_score.is_none() && r.put_score.is_none());
                continue; // Explicit correction of the legacy undefined 0/0 directional ratio.
            }
            assert!(r.available);
            assert_eq!(r.side, e["side"]);
            assert_eq!(r.reading, e["reading"]);
            assert_eq!(r.strength, e["strength"]);
            for (actual, key) in [
                (r.bias, "bias"),
                (r.call_score, "call_score"),
                (r.put_score, "put_score"),
                (r.confidence, "confidence"),
                (r.oi_bias, "oi_bias"),
                (r.volume_bias, "volume_bias"),
                (r.dominance_bias, "dominance_bias"),
                (r.gex_bias, "gex_bias"),
            ] {
                assert!(
                    (actual.unwrap() - e[key].as_f64().unwrap()).abs() < 1e-9,
                    "{} {key}",
                    case["name"]
                );
            }
        }
        for case in f["alignments"].as_array().unwrap() {
            let s = &case["sides"];
            let a = align([
                s[0].as_str().unwrap(),
                s[1].as_str().unwrap(),
                s[2].as_str().unwrap(),
            ]);
            let e = &case["expected"];
            assert_eq!(a.status, e["status"]);
            assert_eq!(a.side, e["side"]);
            for (n, key) in [
                (a.call_count, "call_count"),
                (a.put_count, "put_count"),
                (a.neutral_count, "neutral_count"),
                (a.unavailable_count, "unavailable_count"),
            ] {
                assert_eq!(n as u64, e[key].as_u64().unwrap());
            }
        }
        for case in f["biases"].as_array().unwrap() {
            assert_eq!(
                serde_json::to_value(classify(case["bias"].as_f64().unwrap())).unwrap(),
                case["expected"]
            );
        }
    }
    #[test]
    fn incomplete_invalid_or_overflow_inputs_cannot_certify_direction() {
        for rows in [
            vec![],
            vec![Row {
                call_oi: f64::MAX,
                put_oi: f64::MAX,
                ..Default::default()
            }],
            vec![Row {
                dominance: f64::NAN,
                ..Default::default()
            }],
            vec![Row {
                call_volume: -1.0,
                ..Default::default()
            }],
        ] {
            let r = calculate(&rows, true, 1.0, 1.0);
            assert!(!r.available);
            assert_eq!(r.side, "N/A");
            assert_eq!(r.bias, None);
            assert!(!r.entry_confirmed);
        }
        assert!(!calculate(&[Row::default()], false, 0.0, 0.0).available);
        assert!(!calculate(&[Row::default()], true, f64::MAX, f64::MAX).available);
        assert_eq!(
            align(["call", " CALL ", "unknown"]).status,
            "PARTIAL ALIGNMENT"
        );
    }
}
