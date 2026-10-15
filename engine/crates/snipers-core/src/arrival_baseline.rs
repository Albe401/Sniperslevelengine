//! Prospective benchmark only: driftless log Brownian motion, no fitted direction weights.
//! Reflection principle: P(hit distance a in H)=erfc(a/(sigma*sqrt(2H))).
//! Integrate the capture-to-full-minute delay instead of borrowing a future start price.
use crate::{
    calendar::{self, Day},
    level_research::{Case, ObservedBar},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
const MINUTE: u64 = 60_000;
pub const VERSION: &str = "log-brownian-rth-arrival-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Forecast {
    pub version: String,
    pub case: Case,
    pub issued_ms: u64,
    pub probability: f64,
    pub volatility_sensitivity: [f64; 2],
    pub sigma_per_minute: f64,
    pub returns: usize,
    pub volatility_through_ms: u64,
    pub horizon_trading_minutes: u64,
    pub calibrated: bool,
    pub scope: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub version: String,
    pub forecasts: Vec<Forecast>,
    pub reason: String,
    pub calibrated: bool,
}
fn rth(t: u64) -> bool {
    calendar::date_at(t).is_some_and(|d| match calendar::schedule(d) {
        Day::Open(s) => t >= s.open_ms && t + MINUTE <= s.close_ms,
        _ => false,
    })
}
fn minutes(c: &Case) -> Option<u64> {
    if c.start_ms >= c.deadline_ms
        || crate::level_research::deadline(c.captured_ms, &c.level.horizon) != Some(c.deadline_ms)
    {
        return None;
    }
    let mut n = 0;
    let mut date = calendar::date_at(c.start_ms)?;
    let last = calendar::date_at(c.deadline_ms)?;
    while date <= last {
        match calendar::schedule(date) {
            Day::Open(s) => {
                let from = c.start_ms.max(s.open_ms);
                let through = c.deadline_ms.min(s.close_ms);
                n += through.saturating_sub(from) / MINUTE;
            }
            Day::Closed => {}
            Day::Unknown => return None,
        }
        date = date.succ_opt()?;
    }
    (n > 0).then_some(n)
}
// Abramowitz-Stegun 7.1.26, abs error <1.5e-7 for erf on x>=0.
fn erfc(x: f64) -> f64 {
    let t = 1. / (1. + 0.3275911 * x);
    (((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t - 0.284496736) * t + 0.254829592)
        * t
        * (-x * x).exp())
    .clamp(0., 1.)
}
fn conditional(x: f64, lo: f64, hi: f64, sigma: f64, h: f64) -> f64 {
    let distance = if x < lo {
        lo - x
    } else if x > hi {
        x - hi
    } else {
        0.
    };
    erfc(distance / (sigma * (2. * h).sqrt()))
}
pub fn probability(c: &Case, sigma: f64, h: u64) -> Option<f64> {
    if !sigma.is_finite()
        || sigma <= 0.
        || h == 0
        || !c.spot_at_capture.is_finite()
        || c.spot_at_capture <= 0.
        || c.level.zone.low <= 0.
        || c.level.zone.high < c.level.zone.low
        || c.start_ms < c.captured_ms
    {
        return None;
    }
    let x = c.spot_at_capture.ln();
    let lo = c.level.zone.low.ln();
    let hi = c.level.zone.high.ln();
    let delay = (c.start_ms - c.captured_ms) as f64 / MINUTE as f64;
    let p = if delay == 0. {
        conditional(x, lo, hi, sigma, h as f64)
    } else {
        // Fixed quadrature over N(0,1), tail outside +/-6 is <2e-9.
        let mut sum = 0.;
        let mut weight = 0.;
        for i in 0..=240 {
            let z = -6. + i as f64 * 0.05;
            let w = (-0.5 * z * z).exp() * if i == 0 || i == 240 { 0.5 } else { 1. };
            sum += w * conditional(x + sigma * delay.sqrt() * z, lo, hi, sigma, h as f64);
            weight += w;
        }
        sum / weight
    };
    p.is_finite().then_some(p.clamp(0., 1.))
}
pub fn forecast(cases: &[Case], history: &[ObservedBar], now: u64) -> Reading {
    let pending = |reason: &str| Reading {
        version: VERSION.into(),
        forecasts: vec![],
        reason: reason.into(),
        calibrated: false,
    };
    let Some(first) = cases.first() else {
        return pending("Sin objetivos elegibles en sesión RTH");
    };
    if first.captured_ms != now {
        return pending("El pronóstico debe emitirse en el instante de captura");
    }
    let mut by_time: BTreeMap<u64, &ObservedBar> = BTreeMap::new();
    for b in history.iter().filter(|b| {
        b.symbol == first.level.symbol
            && b.timestamp_ms + MINUTE <= now
            && b.available_ms <= now
            && rth(b.timestamp_ms)
    }) {
        if [b.open, b.high, b.low, b.close]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
            || b.high < b.open.max(b.close)
            || b.low > b.open.min(b.close)
            || b.available_ms < b.timestamp_ms + MINUTE
            || b.timestamp_ms % MINUTE != 0
        {
            return pending("Historial OHLC inválido");
        }
        if let Some(old) = by_time.insert(b.timestamp_ms, b) {
            if old.open != b.open || old.high != b.high || old.low != b.low || old.close != b.close
            {
                return pending("Revisiones contradictorias en el historial de volatilidad");
            }
        }
    }
    let Some((&last, _)) = by_time.last_key_value() else {
        return pending("Falta historial de volatilidad");
    };
    if now - (last + MINUTE) > 120_000 {
        return pending("Volatilidad antigua: falta una vela cerrada reciente");
    }
    let earliest = now.saturating_sub(7 * 86_400_000);
    let mut previous: Option<(u64, f64)> = None;
    let mut returns = vec![];
    for (&t, b) in by_time.range(earliest..) {
        if let Some((pt, p)) = previous {
            if t == pt + MINUTE {
                returns.push((b.close / p).ln());
            }
        }
        previous = Some((t, b.close));
    }
    if returns.len() > 120 {
        returns.drain(..returns.len() - 120);
    }
    if returns.len() < 60 {
        return pending("Requiere al menos 60 retornos de minutos RTH reales");
    }
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let sigma = (returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>()
        / (returns.len() - 1) as f64)
        .sqrt();
    if !sigma.is_finite() || sigma < 1e-10 {
        return pending("Volatilidad nula o inválida");
    }
    let mut forecasts = vec![];
    for c in cases {
        if !c.level.eligible
            || c.captured_ms != now
            || c.level.symbol != first.level.symbol
            || c.level.available_ms != now
            || c.level.observed_ms > now
            || c.barrier.is_some()
        {
            continue;
        }
        let Some(h) = minutes(c) else {
            continue;
        };
        let Some(p) = probability(c, sigma, h) else {
            continue;
        };
        let low = probability(c, sigma * 0.75, h).unwrap();
        let high = probability(c, sigma * 1.25, h).unwrap();
        forecasts.push(Forecast{version:VERSION.into(),case:c.clone(),issued_ms:now,probability:p,volatility_sensitivity:[low.min(high),low.max(high)],sigma_per_minute:sigma,returns:returns.len(),volatility_through_ms:last+MINUTE,horizon_trading_minutes:h,calibrated:false,
            scope:"Experimental: deriva logarítmica cero y volatilidad constante; reloj RTH sin riesgo nocturno. Estima toque continuo; evaluación mediante rango OHLC de un minuto. Sensibilidad +/-25% de sigma, no intervalo de confianza. Sin órdenes.".into()});
    }
    // Keep nearest eligible upper/lower target per horizon. Never turn distance into a vote.
    forecasts.sort_by(|a, b| {
        a.case
            .level
            .horizon
            .cmp(&b.case.level.horizon)
            .then(a.case.direction.cmp(&b.case.direction))
            .then(if a.case.direction == "up" {
                a.case.level.zone.low.total_cmp(&b.case.level.zone.low)
            } else {
                b.case.level.zone.high.total_cmp(&a.case.level.zone.high)
            })
    });
    forecasts.dedup_by(|a, b| {
        a.case.level.horizon == b.case.level.horizon && a.case.direction == b.case.direction
    });
    Reading {
        version: VERSION.into(),
        reason: if forecasts.is_empty() {
            "Ningún objetivo compatible"
        } else {
            "Estimación experimental sin calibración empírica"
        }
        .into(),
        forecasts,
        calibrated: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn setup() -> (Vec<Case>, Vec<ObservedBar>, u64) {
        let now = calendar::rfc3339_ms("2026-10-09T16:00:00Z").unwrap();
        let row = json!({"version":"research-capture-v1","id":"test","symbol":"QQQ","capturedMs":now,"availableMs":now,"quality":{"priceObservationEligible":true,"bars":"available"},"evidence":{"tick":{"symbol":"QQQ","price":100.,"tradeMs":now},"optionSnapshotStatus":{"horizons":[{"mode":"DAY","observedMs":now,"structure":{"complete":true,"callWall":{"strike":101.},"putWall":{"strike":100./1.01}}},{"mode":"WEEK","observedMs":now,"structure":{"complete":true,"magnet":102.}}]}}});
        let cases = crate::level_research::catalog(&row).cases;
        let bars = (0..121)
            .map(|i| {
                let t = now - (121 - i) * MINUTE;
                let p = 100. * (if i % 2 == 0 { 0.001_f64 } else { -0.001_f64 }).exp();
                ObservedBar {
                    symbol: "QQQ".into(),
                    timestamp_ms: t,
                    available_ms: t + MINUTE,
                    open: p,
                    high: p,
                    low: p,
                    close: p,
                }
            })
            .collect();
        (cases, bars, now)
    }
    #[test]
    fn reflection_principle_values_and_monotonicity() {
        assert!((erfc(1. / 2_f64.sqrt()) - 0.3173105079).abs() < 2e-7);
        let (c, _, _) = setup();
        let mut c = c[0].clone();
        c.level.zone.low = 101.;
        c.level.zone.high = 101.;
        let a = probability(&c, 0.001, 60).unwrap();
        c.level.zone.low = 102.;
        c.level.zone.high = 102.;
        assert!(probability(&c, 0.001, 60).unwrap() < a);
        assert!(probability(&c, 0.001, 120).unwrap() > probability(&c, 0.001, 60).unwrap());
        assert!(probability(&c, 0.002, 60).unwrap() > probability(&c, 0.001, 60).unwrap());
    }
    #[test]
    fn real_history_produces_frozen_uncalibrated_independent_events() {
        let (c, b, n) = setup();
        let r = forecast(&c, &b, n);
        assert_eq!(r.forecasts.len(), 3);
        assert!(r.forecasts.iter().all(|f| !f.calibrated
            && f.issued_ms == n
            && f.volatility_through_ms <= n
            && f.probability > 0.
            && f.probability < 1.));
        assert!((r.forecasts[0].probability - r.forecasts[1].probability).abs() < 1e-10);
    }
    #[test]
    fn refuses_short_stale_foreign_zero_or_conflicting_history() {
        let (c, b, n) = setup();
        assert!(forecast(&c, &b[..40], n).forecasts.is_empty());
        let foreign: Vec<_> = b
            .iter()
            .cloned()
            .map(|mut b| {
                b.symbol = "SPY".into();
                b
            })
            .collect();
        assert!(forecast(&c, &foreign, n).forecasts.is_empty());
        let flat: Vec<_> = b
            .iter()
            .cloned()
            .map(|mut b| {
                b.open = 100.;
                b.high = 100.;
                b.low = 100.;
                b.close = 100.;
                b
            })
            .collect();
        assert!(forecast(&c, &flat, n).forecasts.is_empty());
        let mut conflict = b.clone();
        let mut revise = b[0].clone();
        revise.high += 1.;
        conflict.push(revise);
        assert!(forecast(&c, &conflict, n).forecasts.is_empty());
    }
    #[test]
    fn future_data_cannot_change_forecast() {
        let (c, mut b, n) = setup();
        let before = forecast(&c, &b, n).forecasts[0].probability;
        b.push(ObservedBar {
            symbol: "QQQ".into(),
            timestamp_ms: n,
            available_ms: n + MINUTE,
            open: 100.,
            high: 1000.,
            low: 1.,
            close: 1000.,
        });
        assert_eq!(forecast(&c, &b, n).forecasts[0].probability, before);
        assert!(forecast(&c, &b, n + 1).forecasts.is_empty());
    }
    #[test]
    fn fractional_minute_delay_is_integrated_without_future_price() {
        let (c, _, _) = setup();
        let mut c = c[0].clone();
        c.captured_ms += 15_000;
        c.start_ms += MINUTE;
        let p = probability(&c, 0.001, 60).unwrap();
        assert!(p.is_finite() && p > 0. && p < 1.);
    }
}
