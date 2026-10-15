//! Internal, descriptive support. No directional score or execution permission.
use crate::{
    calendar::{self, Day},
    options::Observation,
    theta_vega::{self, Read},
};
use serde::{Deserialize, Serialize};
const YEAR: f64 = 365. * 86400.;
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub symbol: String,
    pub at: u64,
    pub available: bool,
    pub comparable: usize,
    pub contracts: Vec<Support>,
    pub interpretation: String,
    pub model: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Support {
    pub contract: Read,
    pub charm_delta_day: Option<f64>,
    pub vanna_delta_iv_point: Option<f64>,
    pub iv_change_points: Option<f64>,
    pub vega_effect_unit: Option<f64>,
    pub elapsed_seconds: Option<f64>,
    pub previous_ms: Option<u64>,
}
// Normal CDF (Abramowitz/Stegun approximation); derivatives are checked independently.
fn cdf(x: f64) -> f64 {
    let z = x.abs();
    let t = 1. / (1. + 0.2316419 * z);
    let p = (-z * z / 2.).exp() / std::f64::consts::TAU.sqrt();
    let v = 1.
        - p * t
            * (0.319381530
                + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    if x >= 0. {
        v
    } else {
        1. - v
    }
}
fn delta(s: f64, k: f64, v: f64, t: f64, r: f64, q: f64, call: bool) -> f64 {
    let d = ((s / k).ln() + (r - q + v * v / 2.) * t) / (v * t.sqrt());
    (-q * t).exp() * (cdf(d) - if call { 0. } else { 1. })
}
/// BSM European approximation. Vanna per +1 IV point; Charm per elapsed calendar day.
pub fn model(s: f64, k: f64, v: f64, t: f64, r: f64, q: f64, call: bool) -> Option<(f64, f64)> {
    if [s, k, v, t, r, q].iter().any(|n| !n.is_finite())
        || s <= 0.
        || k <= 0.
        || v <= 0.
        || t <= 900. / YEAR
    {
        return None;
    }
    let d1 = ((s / k).ln() + (r - q + v * v / 2.) * t) / (v * t.sqrt());
    let d2 = d1 - v * t.sqrt();
    let density = (-q * t - d1 * d1 / 2.).exp() / std::f64::consts::TAU.sqrt();
    let vanna = -density * d2 / v / 100.;
    let charm = (q * delta(s, k, v, t, r, q, call)
        - density * (2. * (r - q) * t - d2 * v * t.sqrt()) / (2. * t * v * t.sqrt()))
        / 365.;
    (charm.is_finite() && vanna.is_finite()).then_some((charm, vanna))
}
#[derive(Clone, Default)]
pub struct Tracker {
    session: String,
    last_ms: u64,
    previous: Vec<Read>,
    current: Vec<Read>,
}
// Bounded per-expiry sample for Snipers internal support; no additional provider calls.
fn support_reads(o: &Observation, today: &str, now: u64) -> Vec<Read> {
    let expiries: std::collections::BTreeSet<_> = o.contracts.values()
        .filter(|c| c.expiration.as_str() >= today && matches!(c.kind.as_str(), "call" | "put"))
        .map(|c| c.expiration.clone()).collect();
    let mut out = Vec::new();
    for expiry in expiries.into_iter().take(4) {
        let mut rows: Vec<_> = o.contracts.values().filter(|c| c.expiration == expiry)
            .filter(|c| matches!(c.kind.as_str(), "call" | "put") && c.underlying_price.is_some_and(|p| p.is_finite() && p > 0.) && c.strike.is_finite() && c.strike > 0.)
            .collect();
        rows.sort_by(|a,b| (a.strike-a.underlying_price.unwrap()).abs().total_cmp(&(b.strike-b.underlying_price.unwrap()).abs()).then_with(||a.ticker.cmp(&b.ticker)));
        out.extend(rows.into_iter().take(64).map(|c| theta_vega::read(c,o.observed_ms,now)).filter(|c| c.recent));
    }
    out
}
impl Tracker {
    pub fn update(
        &mut self,
        o: Option<&Observation>,
        symbol: &str,
        connected: bool,
        now: u64,
    ) -> Context {
        let mut out=Context{symbol:symbol.into(),at:now,model:"BSM europeo aproximado; r=4%, q=0.5% supuestos del modelo GEX, no tasas observadas; cierre convencional 16 ET; Charm por día calendario; Vanna por punto IV. No representa inventario ni flujo institucional.".into(),interpretation:"Apoyo de griegas pendiente: sin observaciones intradía comparables.".into(),..Default::default()};
        let regular=calendar::date_at(now).is_some_and(|d|matches!(calendar::schedule(d),Day::Open(day) if now>=day.open_ms&&now<day.close_ms));
        let session = format!("{symbol}:{:?}", calendar::date_at(now));
        if !connected || !regular {
            *self = Self::default();
            return out;
        }
        if self.session != session
            || self.last_ms > now
            || now.saturating_sub(self.last_ms) > 900_000
        {
            *self = Self {
                session: session.clone(),
                ..Default::default()
            };
        }
        // Retain a bounded historical baseline between 5-minute snapshots. It is not
        // published as current when its quote ages out after 60 seconds.
        let Some(o) = o.filter(|o| {
            o.symbol == symbol
                && o.observed_ms <= now
                && now - o.observed_ms <= 900_000
                && calendar::date_at(o.observed_ms) == calendar::date_at(now)
        }) else {
            return out;
        };
        if self.session != session || o.observed_ms < self.last_ms {
            *self = Self {
                session,
                ..Default::default()
            };
        }
        let today = calendar::date_at(now).unwrap().to_string();
        let rows = support_reads(o, &today, now);
        if rows.is_empty() {
            return out;
        }
        if o.observed_ms > self.last_ms {
            self.previous = std::mem::take(&mut self.current);
            self.current = rows.clone();
            self.last_ms = o.observed_ms;
        }
        let m = crate::exposure::Model::default();
        for c in rows {
            let raw = o.contracts.get(&c.ticker).unwrap();
            let estimated = raw
                .underlying_price
                .zip(raw.iv)
                .zip(crate::horizons::model_years(&c.expiration, o.observed_ms))
                .filter(|_| {
                    raw.underlying_ms
                        .is_some_and(|t| t <= now && now - t <= 60000)
                })
                .and_then(|((spot, iv), years)| {
                    model(
                        spot,
                        c.strike,
                        iv,
                        years,
                        m.risk_free_rate,
                        m.dividend_yield,
                        c.kind == "call",
                    )
                });
            let previous = self.previous.iter().find(|p| {
                p.ticker == c.ticker
                    && p.observed_ms < c.observed_ms
                    && c.observed_ms - p.observed_ms <= 900_000
                    && p.quote_ms.zip(c.quote_ms).is_some_and(|(a, b)| b > a)
            });
            let change = previous
                .and_then(|p| p.iv.zip(c.iv))
                .map(|(a, b)| (b - a) * 100.)
                .filter(|x| x.is_finite());
            let effect = previous
                .and_then(|p| p.vega)
                .zip(change)
                .map(|(v, d)| v * d)
                .filter(|x| x.is_finite());
            if effect.is_some() {
                out.comparable += 1;
            }
            out.contracts.push(Support {
                charm_delta_day: estimated.map(|x| x.0),
                vanna_delta_iv_point: estimated.map(|x| x.1),
                iv_change_points: change,
                vega_effect_unit: effect,
                elapsed_seconds: previous.map(|p| (c.observed_ms - p.observed_ms) as f64 / 1000.),
                previous_ms: previous.map(|p| p.observed_ms),
                contract: c,
            });
        }
        out.available = !out.contracts.is_empty();
        let falling = out
            .contracts
            .iter()
            .filter(|r| r.vega_effect_unit.is_some_and(|v| v < 0.))
            .count();
        let rising = out
            .contracts
            .iter()
            .filter(|r| r.vega_effect_unit.is_some_and(|v| v > 0.))
            .count();
        out.interpretation=format!("Contratos cercanos al precio en hasta cuatro expiraciones (máximo 64 por expiración): {}. Comparables: {}. IV bajando con presión local sobre prima: {}; IV subiendo con apoyo local: {}. Theta mide sensibilidad temporal; Charm/Vanna estiman cambios de Delta, no dirección del activo ni coberturas reales. Sin confirmación de entrada ni atribución exacta de P/L.",out.contracts.len(),out.comparable,falling,rising);
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn observation(now: u64) -> Observation {
        let ticker = "O:QQQ260925C00745000";
        let c=crate::options::normalize(&serde_json::json!({"details":{"ticker":ticker,"expiration_date":"2026-09-25","strike_price":745,"contract_type":"call"},"greeks":{"theta":-0.1,"vega":0.2},"implied_volatility":0.2,"underlying_asset":{"price":745,"last_updated":now*1_000_000},"last_quote":{"last_updated":now*1_000_000,"timeframe":"REAL-TIME"}}),"QQQ").unwrap();
        Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: now,
            observed_date: "2026-09-25".into(),
            oi_reference_date: None,
            contracts: [(ticker.into(), c)].into(),
        }
    }
    #[test]
    fn four_expirations_and_per_expiry_capacity_are_explicit() {
        let now=calendar::rfc3339_ms("2026-09-25T15:00:00Z").unwrap();
        let mut o=observation(now);let base=o.contracts.values().next().unwrap().clone();o.contracts.clear();
        for expiry in ["2026-09-24","2026-09-25","2026-09-28","2026-09-29","2026-09-30","2026-10-01"] {
            for i in 0..70 {let mut c=base.clone();c.expiration=expiry.into();c.ticker=format!("{expiry}-{i}");c.strike=740.+i as f64*0.1;o.contracts.insert(c.ticker.clone(),c);}
        }
        let rows=support_reads(&o,"2026-09-25",now);assert_eq!(rows.len(),256);
        assert!(rows.iter().all(|r|r.expiration!="2026-09-24"&&r.expiration!="2026-10-01"));
        for expiry in ["2026-09-25","2026-09-28","2026-09-29","2026-09-30"] {assert_eq!(rows.iter().filter(|r|r.expiration==expiry).count(),64);}
    }
    #[test]
    fn next_expiry_compares_its_own_contract_not_zero_dte() {
        let now=calendar::rfc3339_ms("2026-09-25T15:00:00Z").unwrap();let mut o=observation(now);
        let c=o.contracts.values_mut().next().unwrap();c.expiration="2026-09-28".into();
        let mut tr=Tracker::default();let first=tr.update(Some(&o),"QQQ",true,now);assert!(first.available);assert_eq!(first.contracts[0].contract.expiration,"2026-09-28");
        o.observed_ms=now+300000;let c=o.contracts.values_mut().next().unwrap();c.iv=Some(0.19);c.quote_ms=Some(now+300000);c.underlying_ms=Some(now+300000);
        let next=tr.update(Some(&o),"QQQ",true,now+300000);assert_eq!(next.comparable,1);assert!(next.contracts[0].vanna_delta_iv_point.is_some());assert_eq!(next.contracts[0].previous_ms,Some(now));
    }
    #[test]
    fn five_minute_cadence_keeps_baseline_but_never_publishes_stale_as_current() {
        let now = calendar::rfc3339_ms("2026-09-25T15:00:00Z").unwrap();
        let first = observation(now);
        let mut tr = Tracker::default();
        assert!(tr.update(Some(&first), "QQQ", true, now).available);
        let aged = tr.update(Some(&first), "QQQ", true, now + 61000);
        assert!(!aged.available);
        assert!(aged.contracts.is_empty());
        let mut second = observation(now + 300000);
        second.contracts.values_mut().next().unwrap().iv = Some(0.19);
        let compared = tr.update(Some(&second), "QQQ", true, now + 300000);
        assert_eq!(compared.comparable, 1);
        assert_eq!(compared.contracts[0].previous_ms, Some(now));
        assert_eq!(compared.contracts[0].elapsed_seconds, Some(300.));
        tr.update(Some(&second), "QQQ", false, now + 300001);
        assert_eq!(
            tr.update(Some(&observation(now + 600000)), "QQQ", true, now + 600000)
                .comparable,
            0
        );
    }
    #[test]
    fn closed_session_future_stale_and_long_gap_do_not_create_comparisons() {
        let now = calendar::rfc3339_ms("2026-09-25T15:00:00Z").unwrap();
        let first = observation(now);
        let mut tr = Tracker::default();
        tr.update(Some(&first), "QQQ", true, now);
        assert!(
            !tr.update(Some(&observation(now + 300000)), "QQQ", true, now + 1000)
                .available
        );
        assert_eq!(
            tr.update(Some(&observation(now + 900001)), "QQQ", true, now + 900001)
                .comparable,
            0
        );
        assert!(
            !tr.update(Some(&first), "QQQ", true, now + 86400000)
                .available
        );
    }
    #[test]
    fn derivatives_match_finite_differences() {
        for call in [true, false] {
            for s in [95., 100., 105.] {
                let (s, k, v, t, r, q) = (s, 100., 0.25, 0.03, 0.04, 0.005);
                let (c, w) = model(s, k, v, t, r, q, call).unwrap();
                let hv = 1e-5;
                let ht = 1e-6;
                let wn = (delta(s, k, v + hv, t, r, q, call) - delta(s, k, v - hv, t, r, q, call))
                    / (2. * hv)
                    / 100.;
                let cn = (delta(s, k, v, t - ht, r, q, call) - delta(s, k, v, t + ht, r, q, call))
                    / (2. * ht)
                    / 365.;
                assert!((w - wn).abs() < 1e-7);
                assert!((c - cn).abs() < 1e-7);
            }
        }
    }
    #[test]
    fn invalid_and_near_expiry_are_pending() {
        assert!(model(100., 100., 0., 0.01, 0.04, 0., true).is_none());
        assert!(model(100., 100., 0.2, 500. / YEAR, 0.04, 0., true).is_none());
        assert!(model(f64::NAN, 100., 0.2, 0.01, 0.04, 0., true).is_none());
    }
    #[test]
    fn snapshots_compare_only_same_contract_and_new_quote() {
        let now = calendar::rfc3339_ms("2026-09-25T15:00:00Z").unwrap();
        let mut c=crate::options::normalize(&serde_json::json!({"details":{"ticker":"O:QQQ260925C00745000","expiration_date":"2026-09-25","strike_price":745,"contract_type":"call"},"greeks":{"theta":-0.1,"vega":0.2},"implied_volatility":0.2,"underlying_asset":{"price":745,"last_updated":now*1_000_000},"last_quote":{"last_updated":now*1_000_000,"timeframe":"REAL-TIME"}}),"QQQ").unwrap();
        c.premium_multiplier = Some(100.);
        let mut o = Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: now,
            observed_date: "2026-09-25".into(),
            oi_reference_date: None,
            contracts: [(c.ticker.clone(), c)].into(),
        };
        let mut tr = Tracker::default();
        assert_eq!(tr.update(Some(&o), "QQQ", true, now).comparable, 0);
        o.observed_ms += 30000;
        o.contracts.values_mut().next().unwrap().iv = Some(0.19);
        assert_eq!(tr.update(Some(&o), "QQQ", true, now + 30000).comparable, 0);
        o.observed_ms += 30000;
        o.contracts.values_mut().next().unwrap().quote_ms = Some(now + 60000);
        o.contracts.values_mut().next().unwrap().iv = Some(0.18);
        let r = tr.update(Some(&o), "QQQ", true, now + 60000);
        assert_eq!(r.comparable, 1);
        assert!((r.contracts[0].vega_effect_unit.unwrap() + 0.2).abs() < 1e-9);
        assert_eq!(tr.update(Some(&o), "QQQ", true, now + 60000).comparable, 1);
        assert!(!tr.update(Some(&o), "SPY", true, now + 60000).available);
        assert!(!tr.update(Some(&o), "QQQ", false, now + 60000).available);
    }
}
