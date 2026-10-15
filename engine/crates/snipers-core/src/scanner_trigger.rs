//! Two-minute VWAP geometry. A trigger is analysis, never entry permission.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub direction: String,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub close: Option<f64>,
    pub vwap: Option<f64>,
    pub volume: Option<f64>,
    pub volume_ratio: Option<f64>,
    pub baseline_bars: usize,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub ready: bool,
    pub reason: String,
    pub trigger_type: Option<String>,
    pub reference_price: Option<f64>,
    pub invalidation: Option<f64>,
    pub volume_ratio: Option<f64>,
    pub baseline_bars: usize,
    pub entry_confirmed: bool,
}
pub fn evaluate(i: &Input) -> Read {
    let mut out = Read {
        volume_ratio: i.volume_ratio.filter(|v| v.is_finite() && *v >= 0.0),
        baseline_bars: i.baseline_bars,
        ..Default::default()
    };
    let positive = |v: Option<f64>| v.filter(|v| v.is_finite() && *v > 0.0);
    let values = (
        positive(i.open),
        positive(i.high),
        positive(i.low),
        positive(i.close),
        positive(i.vwap),
        i.volume.filter(|v| v.is_finite() && *v >= 0.0),
    );
    let (Some(o), Some(h), Some(l), Some(c), Some(vwap), Some(volume)) = values else {
        out.reason = "VELA 2M / VWAP INCOMPLETOS".into();
        return out;
    };
    if l > o.min(c).min(h) || h < o.max(c).max(l) {
        out.reason = "OHLC 2M INVALIDO".into();
        return out;
    }
    let Some(ratio) = out.volume_ratio.filter(|_| i.baseline_bars >= 3) else {
        out.reason = "ESPERANDO BASE DE VOLUMEN 2M".into();
        return out;
    };
    if volume <= 0.0 || ratio < 1.0 {
        out.reason = format!("VOLUMEN 2M {ratio:.2}x POR DEBAJO DE 1.00x");
        return out;
    }
    let bullish = i.direction == "ALCISTA";
    let kind = match i.direction.as_str() {
        "ALCISTA" if c > o && o <= vwap && c > vwap => Some("RECLAIM VWAP 2M"),
        "ALCISTA" if c > o && o > vwap && l <= vwap && c > vwap => Some("RECHAZO VWAP 2M"),
        "BAJISTA" if c < o && o >= vwap && c < vwap => Some("PERDIDA VWAP 2M"),
        "BAJISTA" if c < o && o < vwap && h >= vwap && c < vwap => Some("RECHAZO VWAP 2M"),
        "ALCISTA" | "BAJISTA" => None,
        _ => {
            out.reason = "DIRECCION NO VERIFICADA".into();
            return out;
        }
    };
    let Some(kind) = kind else {
        out.reason = "SIN RECLAIM / RECHAZO VWAP EN VELA 2M".into();
        return out;
    };
    out.ready = true;
    out.reason = format!("{kind} · VOL 2M {ratio:.2}x");
    out.trigger_type = Some(kind.into());
    out.reference_price = Some(c);
    out.invalidation = Some(if bullish { l.min(vwap) } else { h.max(vwap) });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_trigger_fixtures_and_invalid_numbers() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/legacy-scanner-trigger.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let input: Input = serde_json::from_value(case["input"].clone()).unwrap();
            let actual = evaluate(&input);
            let expected = &case["expected"];
            assert_eq!(
                actual.ready,
                expected["ready"].as_bool().unwrap(),
                "{input:?}"
            );
            assert_eq!(
                actual.trigger_type.as_deref(),
                expected["trigger_type"].as_str()
            );
            assert_eq!(actual.invalidation, expected["invalidation"].as_f64());
            assert!(!actual.entry_confirmed);
            for invalid in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
                let mut bad = input.clone();
                bad.close = Some(invalid);
                assert!(!evaluate(&bad).ready);
            }
        }
    }
}
