//! Closed-2m confirmation around the main options magnet, distinct from VWAP.
use crate::{
    bars::SessionBar,
    calendar::{self, Day},
};
use serde::Serialize;
#[derive(Clone, Default, Serialize)]
pub struct Trigger {
    pub ready: bool,
    pub reason: String,
    pub touched_recently: bool,
    pub volume_ratio: Option<f64>,
    pub distance_from_magnet_pct: Option<f64>,
    pub vwap_supportive: bool,
    pub magnet_tolerance: Option<f64>,
    pub reference_price: Option<f64>,
    pub bar_end_ms: Option<u64>,
    pub entry_authorized: bool,
}
pub fn evaluate(
    side: &str,
    magnet: f64,
    bars: &[SessionBar],
    vwap: Option<f64>,
    now: u64,
    verified: bool,
) -> Trigger {
    let mut r = Trigger::default();
    let reject = |mut r: Trigger, reason: &str| {
        r.reason = reason.into();
        r
    };
    if !verified || !matches!(side, "CALL" | "PUT") || !magnet.is_finite() || magnet <= 0.0 {
        return reject(r, "SIDE / MAIN MAGNET / FUENTE INVALIDO");
    }
    let Some(last) = bars.last() else {
        return reject(r, "ESPERANDO VELA 2M CERRADA");
    };
    let Some(date) = calendar::date_at(now) else {
        return reject(r, "CALENDARIO NO DISPONIBLE");
    };
    let Day::Open(session) = calendar::schedule(date) else {
        return reject(r, "SESION CERRADA");
    };
    if now < session.open_ms || now >= session.close_ms {
        return reject(r, "SESION CERRADA");
    }
    let Some(end) = last.timestamp.checked_add(120_000) else {
        return reject(r, "HORA DE VELA INVALIDA");
    };
    if end > now || now - end > 180_000 || calendar::date_at(end) != Some(date) {
        return reject(r, "VELA 2M CERRADA NO ES RECIENTE");
    }
    // Only the latest six bars influence a five-bar baseline / four-bar touch.
    // Require complete consecutive RTH bars; a gap cannot fabricate a reclaim.
    let bars = &bars[bars.len().saturating_sub(6)..];
    for (index, b) in bars.iter().enumerate() {
        if b.minutes_received != 2
            || b.timestamp < session.open_ms
            || b.timestamp >= session.close_ms
            || !(b.timestamp - session.open_ms).is_multiple_of(120_000)
            || index > 0 && b.timestamp.checked_sub(bars[index - 1].timestamp) != Some(120_000)
            || ![b.open, b.high, b.low, b.close]
                .into_iter()
                .all(|v| v.is_finite() && v > 0.0)
            || !b.volume.is_finite()
            || b.volume < 0.0
            || b.high < b.open.max(b.close)
            || b.low > b.open.min(b.close)
        {
            return reject(r, "VELAS 2M INCOMPLETAS O SIN CONTINUIDAD");
        }
    }
    if vwap.is_some_and(|v| !v.is_finite() || v <= 0.0) {
        return reject(r, "VWAP INVALIDO");
    }
    let recent = &bars[bars.len().saturating_sub(4)..];
    let tolerance = magnet * 0.001;
    r.touched_recently = recent
        .iter()
        .any(|b| b.low <= magnet + tolerance && b.high >= magnet - tolerance);
    let previous = recent.iter().rev().nth(1).map(|b| b.close);
    let volumes: Vec<_> = bars[..bars.len() - 1]
        .iter()
        .filter(|b| b.volume > 0.0)
        .map(|b| b.volume)
        .collect();
    let baseline = if volumes.len() >= 2 {
        Some(volumes.iter().sum::<f64>() / volumes.len() as f64)
    } else {
        None
    };
    let ratio = baseline
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|v| last.volume / v)
        .filter(|v| v.is_finite());
    let volume_ready = ratio.is_some_and(|v| v >= 1.0);
    let distance = (last.close - magnet).abs() / magnet * 100.0;
    if !distance.is_finite() {
        return reject(r, "DISTANCIA INVALIDA");
    }
    let not_chasing = distance <= 0.35;
    let call = side == "CALL";
    let directional = if call {
        last.close > last.open || previous.is_some_and(|p| last.close > p)
    } else {
        last.close < last.open || previous.is_some_and(|p| last.close < p)
    };
    let action = if call {
        r.touched_recently && last.close > magnet && last.low >= magnet - tolerance
    } else {
        r.touched_recently
            && last.close < magnet
            && (previous.is_some_and(|p| p >= magnet - tolerance)
                || last.high >= magnet - tolerance
                || previous.is_some_and(|p| p < magnet))
    };
    r.vwap_supportive = vwap.is_none_or(|v| {
        if call {
            last.close >= v
        } else {
            last.close <= v
        }
    });
    r.volume_ratio = ratio.map(|v| format!("{v:.3}").parse().unwrap_or(v));
    r.distance_from_magnet_pct = Some(format!("{distance:.4}").parse().unwrap_or(distance));
    r.magnet_tolerance = Some(format!("{tolerance:.4}").parse().unwrap_or(tolerance));
    r.reference_price = Some(last.close);
    r.bar_end_ms = Some(end);
    r.ready = action && directional && volume_ready && not_chasing;
    r.reason = if r.ready {
        if call {
            "MAIN MAGNET HOLD / RECLAIM 2M"
        } else {
            "MAIN MAGNET BREAK / FAILED RECLAIM 2M"
        }
        .into()
    } else {
        [
            (
                !r.touched_recently,
                "MAIN MAGNET NO FUE PROBADO RECIENTEMENTE",
            ),
            (!action, "ACCION DEL PRECIO EN MAIN MAGNET NO CONFIRMADA"),
            (!directional, "VELA 2M NO CONFIRMA DIRECCION"),
            (!volume_ready, "VOLUMEN 2M INSUFICIENTE"),
            (!not_chasing, "PRECIO DEMASIADO ALEJADO DEL MAIN MAGNET"),
        ]
        .into_iter()
        .filter_map(|(blocked, reason)| blocked.then_some(reason))
        .collect::<Vec<_>>()
        .join(" | ")
    };
    r
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn main_magnet_legacy_parity_and_gaps_future_partial_source_guards() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-magnet-trigger.json"))
                .unwrap();
        let now = fixtures["nowMs"].as_u64().unwrap();
        for case in fixtures["cases"].as_array().unwrap() {
            let bars: Vec<SessionBar> = serde_json::from_value(case["bars"].clone()).unwrap();
            let side = case["side"].as_str().unwrap();
            let r = evaluate(side, 100.0, &bars, Some(100.0), now, true);
            assert!(!r.entry_authorized);
            let actual = serde_json::to_value(r).unwrap();
            for (key, value) in case["expected"].as_object().unwrap() {
                assert_eq!(&actual[key], value, "{side} {key}");
            }
            assert!(!evaluate(side, 100.0, &bars, None, now, false).ready);
            assert!(!evaluate(side, 100.0, &bars, None, now - 1, true).ready);
            assert!(!evaluate(side, 100.0, &bars, None, now + 180_001, true).ready);
            let mut partial = bars.clone();
            partial.last_mut().unwrap().minutes_received = 1;
            assert!(!evaluate(side, 100.0, &partial, None, now, true).ready);
            if bars.len() > 2 {
                let mut gap = bars.clone();
                gap.remove(1);
                assert!(!evaluate(side, 100.0, &gap, None, now, true).ready);
            }
        }
    }
}
