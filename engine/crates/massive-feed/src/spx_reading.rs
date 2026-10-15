//! Independent, read-only SPXW structural lane. Never supplies equity ticks or orders.
use crate::{
    credentials::Credentials,
    live::{now_ms, SharedState},
    spx_synthetic::Status,
};
use serde::Serialize;
use snipers_core::{
    calendar, horizons,
    options::{self, Observation},
};
use std::time::Duration;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub phase: String,
    pub detail: String,
    pub observed_ms: Option<u64>,
    pub reference_ms: Option<u64>,
    pub spot: Option<f64>,
    pub reference_root: String,
    pub reference_expiration: String,
    pub contracts: usize,
    pub excluded_am_contracts: usize,
    pub horizons: Vec<horizons::Horizon>,
    pub oi_levels: Vec<OiLevel>,
    pub entry_confirmed: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OiLevel {
    pub label: String,
    pub call_strike: Option<f64>,
    pub put_strike: Option<f64>,
    pub call_oi: Option<f64>,
    pub put_oi: Option<f64>,
    pub known: usize,
    pub total: usize,
}
fn oi_levels(observation: &Observation, horizons: &[horizons::Horizon], spot: f64) -> Vec<OiLevel> {
    horizons
        .iter()
        .map(|h| {
            let contracts: Vec<_> = observation
                .contracts
                .values()
                .filter(|c| h.expirations.contains(&c.expiration))
                .collect();
            let maximum = |kind: &str| {
                let mut strikes: std::collections::BTreeMap<u64, f64> =
                    std::collections::BTreeMap::new();
                for c in contracts.iter().filter(|c| c.kind == kind) {
                    if let Some(oi) = c.open_interest.filter(|v| v.is_finite() && *v >= 0.) {
                        *strikes.entry(c.strike.to_bits()).or_default() += oi;
                    }
                }
                strikes
                    .into_iter()
                    .filter(|(_, oi)| *oi > 0.)
                    .map(|(k, v)| (f64::from_bits(k), v))
                    .max_by(|(ka, a), (kb, b)| {
                        a.total_cmp(b)
                            .then_with(|| (kb - spot).abs().total_cmp(&(ka - spot).abs()))
                    })
            };
            let call = maximum("call");
            let put = maximum("put");
            OiLevel {
                label: h.label.clone(),
                call_strike: call.map(|p| p.0),
                put_strike: put.map(|p| p.0),
                call_oi: call.map(|p| p.1),
                put_oi: put.map(|p| p.1),
                known: contracts
                    .iter()
                    .filter(|c| c.open_interest.is_some())
                    .count(),
                total: contracts.len(),
            }
        })
        .collect()
}
fn recent(t: u64, now: u64, age: u64) -> bool {
    t <= now && now - t <= age && calendar::date_at(t) == calendar::date_at(now)
}
impl Reading {
    pub fn current(&self, now: u64, synthetic: &Status) -> Self {
        let mut out = self.clone();
        let usable = synthetic.phase == "ready"
            && synthetic.estimate.as_ref().is_some_and(|e| {
                e.usable_for_analysis
                    && recent(e.oldest_quote_ms, now, 15_000)
                    && e.root == self.reference_root
                    && e.expiration == self.reference_expiration
            });
        if !usable
            || self.observed_ms.is_none_or(|t| !recent(t, now, 300_000))
            || self.reference_ms.is_none_or(|t| !recent(t, now, 300_000))
        {
            out.phase = "waiting".into();
            if self.phase != "waiting" || self.detail.is_empty() {
                out.detail = "Esperando referencia SPX reciente y cadena propia vigente".into();
            }
            out.horizons.clear();
            out.oi_levels.clear();
            out.spot = None;
        }
        out.entry_confirmed = false;
        out
    }
}

pub fn calculate(
    mut observation: Observation,
    synthetic: &Status,
    now: u64,
) -> Result<Reading, &'static str> {
    let e = synthetic
        .estimate
        .as_ref()
        .filter(|e| {
            synthetic.phase == "ready"
                && e.usable_for_analysis
                && e.approximate
                && e.price.is_finite()
                && e.price > 0.
                && matches!(e.root.as_str(), "SPX" | "SPXW")
                && recent(e.oldest_quote_ms, now, 15_000)
        })
        .ok_or("Referencia sintética SPX no vigente")?;
    if observation.symbol != "SPX"
        || observation.contracts.is_empty()
        || !recent(observation.observed_ms, now, 300_000)
    {
        return Err("Cadena SPX ausente o caducada");
    }
    if observation
        .contracts
        .values()
        .any(|c| !matches!(options::underlying(&c.ticker), Some("SPX" | "SPXW")))
    {
        return Err("Cadena SPX contiene otro subyacente");
    }
    let excluded_am_contracts = observation
        .contracts
        .values()
        .filter(|c| options::underlying(&c.ticker) == Some("SPX"))
        .count();
    observation.contracts.retain(|_, c| {
        options::underlying(&c.ticker) == Some("SPXW")
            && calendar::day_bounds(&c.expiration)
                .and_then(|(t, _)| calendar::date_at(t))
                .is_some_and(
                    |d| matches!(calendar::schedule(d), calendar::Day::Open(s) if now < s.close_ms),
                )
    });
    if observation.contracts.is_empty() {
        return Err("Sin contratos SPXW PM vigentes");
    }
    let horizons = horizons::calculate(
        &observation,
        e.price,
        "SPX sintético · paridad CALL/PUT; SPXW PM; rango local",
    )?;
    let oi_levels = oi_levels(&observation, &horizons, e.price);
    Ok(Reading {
        oi_levels,
        phase: "ready".into(),
        detail: "Lectura independiente SPXW PM · OI diario y griegas del proveedor; cobertura local, no cadena total. Referencia aproximada; sin confirmación de entrada.".into(),
        observed_ms: Some(observation.observed_ms), reference_ms: Some(e.oldest_quote_ms), spot: Some(e.price),
        reference_root: e.root.clone(), reference_expiration: e.expiration.clone(),
        contracts: observation.contracts.len(), excluded_am_contracts, horizons, entry_confirmed: false,
    })
}

pub async fn worker(key: &Credentials, state: SharedState) {
    let mut timer = tokio::time::interval(Duration::from_secs(5));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut next = 0;
    loop {
        timer.tick().await;
        if now_ms() < next {
            continue;
        }
        let before = {
            let s = state.lock().unwrap();
            s.spx_engine
                .snapshot(now_ms(), s.options.phase == "streaming")
        };
        let Some(price) = before
            .estimate
            .as_ref()
            .filter(|e| e.usable_for_analysis && before.phase == "ready")
            .map(|e| e.price)
        else {
            continue;
        };
        // Bounded REST refresh, same existing Options socket; never subscribe a second feed.
        let result = crate::option_snapshot::fetch(key, "SPX", price, 4).await;
        let current = {
            let s = state.lock().unwrap();
            s.spx_engine
                .snapshot(now_ms(), s.options.phase == "streaming")
        };
        let result = match result {
            Ok(observation) => {
                tokio::task::spawn_blocking(move || calculate(observation, &current, now_ms()))
                    .await
                    .unwrap_or(Err("Calculador SPX interrumpido"))
            }
            Err(e) => Err(e),
        };
        let mut s = state.lock().unwrap();
        match result {
            Ok(reading) => {
                s.spx_reading = reading;
                next = now_ms() + 60_000;
            }
            Err(e) => {
                s.spx_reading = Reading {
                    phase: "waiting".into(),
                    detail: e.into(),
                    ..Default::default()
                };
                next = now_ms() + 60_000;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inputs() -> (Observation, Status, u64) {
        let now = calendar::legacy_eastern_ms("2026-09-25 10:00:00").unwrap();
        let quotes = [5995., 6000., 6005.]
            .into_iter()
            .flat_map(|strike| {
                [
                    snipers_core::synthetic::Quote {
                        root: "SPXW".into(),
                        expiration: "2026-09-25".into(),
                        side: snipers_core::synthetic::Side::Call,
                        strike,
                        bid: 20. + 6000. - strike,
                        ask: 20.2 + 6000. - strike,
                        quote_ms: now,
                    },
                    snipers_core::synthetic::Quote {
                        root: "SPXW".into(),
                        expiration: "2026-09-25".into(),
                        side: snipers_core::synthetic::Side::Put,
                        strike,
                        bid: 20.,
                        ask: 20.2,
                        quote_ms: now,
                    },
                ]
            })
            .collect::<Vec<_>>();
        let estimate =
            snipers_core::synthetic::spx("SPXW", "2026-09-25", &quotes, now, true).unwrap();
        let mut contracts = std::collections::BTreeMap::new();
        for root in ["SPX", "SPXW"] {
            for kind in ["call", "put"] {
                let cp = if kind == "call" { "C" } else { "P" };
                let row = serde_json::json!({"details":{"ticker":format!("O:{root}260925{cp}06000000"),"expiration_date":"2026-09-25","contract_type":kind,"strike_price":6000},"underlying_asset":{"ticker":"I:SPX"},"open_interest":500,"day":{"volume":20},"greeks":{"gamma":0.01,"delta":if kind=="call"{0.5}else{-0.5}}});
                let mut c = options::normalize(&row, "SPX").unwrap();
                c.premium_multiplier = Some(100.);
                contracts.insert(c.ticker.clone(), c);
            }
        }
        (
            Observation {
                schema: 1,
                symbol: "SPX".into(),
                observed_ms: now,
                observed_date: "2026-09-25".into(),
                oi_reference_date: None,
                contracts,
            },
            Status {
                phase: "ready".into(),
                estimate: Some(estimate),
                ..Default::default()
            },
            now,
        )
    }
    #[test]
    fn own_pm_chain_reaches_levels_and_exposure_without_am_or_entry_confirmation() {
        let (o, s, now) = inputs();
        let r = calculate(o, &s, now).unwrap();
        assert_eq!(r.contracts, 2);
        assert_eq!(r.excluded_am_contracts, 2);
        assert!(!r.entry_confirmed);
        assert_eq!(r.horizons[0].mode, "DAY");
        assert_eq!(r.horizons[0].exposure.gamma_used, 2);
        assert!(r.horizons[0].exposure.gex_complete);
        assert!(r.horizons[0].exposure.zero_gamma.is_none());
        assert_eq!(r.current(now, &s).phase, "ready");
        assert!(r.current(now + 15_001, &s).horizons.is_empty());
        let mut disconnected = s.clone();
        disconnected.phase = "waiting".into();
        assert!(r.current(now, &disconnected).spot.is_none());
    }
    #[test]
    fn wrong_symbol_future_stale_and_mixed_chains_are_rejected() {
        let (o, s, now) = inputs();
        for variant in 0..4 {
            let mut bad = o.clone();
            match variant {
                0 => bad.symbol = "SPY".into(),
                1 => bad.observed_ms = now + 1,
                2 => bad.observed_ms = now - 300_001,
                _ => {
                    bad.contracts.values_mut().next().unwrap().ticker =
                        "O:QQQ260925C06000000".into()
                }
            };
            assert!(calculate(bad, &s, now).is_err());
        }
        let mut bad = s.clone();
        bad.estimate.as_mut().unwrap().oldest_quote_ms = now + 1;
        assert!(calculate(o, &bad, now).is_err());
    }
    #[test]
    fn missing_greeks_stay_partial_and_reference_rotation_hides_previous_reading() {
        let (mut o, mut s, now) = inputs();
        for c in o.contracts.values_mut() {
            c.gamma = None;
            c.delta = None;
        }
        let r = calculate(o, &s, now).unwrap();
        assert!(!r.horizons[0].exposure.gex_complete);
        assert_eq!(r.horizons[0].exposure.gamma_used, 0);
        assert_eq!(r.oi_levels[0].call_strike, Some(6000.));
        assert_eq!(r.oi_levels[0].put_oi, Some(500.));
        s.estimate.as_mut().unwrap().expiration = "2026-09-28".into();
        assert!(r.current(now, &s).horizons.is_empty());
    }
}
