//! SPX parity uses its own quote lane on the shared Options socket.
use crate::{
    credentials::Credentials,
    history::checked_url,
    live::{now_ms, SharedState},
    Event,
};
use serde::Serialize;
use serde_json::Value;
use snipers_core::{
    calendar,
    flow::Contract,
    options,
    synthetic::{self, Quote, Side},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq)]
struct Metadata {
    ticker: String,
    root: String,
    expiration: String,
    strike: f64,
    side: Side,
}
#[derive(Clone, Default)]
pub struct Engine {
    desired: BTreeMap<String, Metadata>,
    subscribed: BTreeSet<String>,
    quotes: BTreeMap<String, Quote>,
    discovered_ms: u64,
    root: String,
    expiration: String,
    detail: String,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: String,
    pub detail: String,
    pub contracts: usize,
    pub estimate: Option<synthetic::Estimate>,
    pub entry_confirmed: bool,
}
struct Plan {
    contracts: BTreeMap<String, Metadata>,
    root: String,
    expiration: String,
}
fn fresh(time: u64, now: u64, max: u64) -> bool {
    time <= now
        && now - time <= max
        && calendar::date_at(now).is_some()
        && calendar::date_at(time) == calendar::date_at(now)
}
impl Engine {
    fn replace(&mut self, plan: Plan, now: u64) {
        self.quotes
            .retain(|ticker, _| self.desired.get(ticker) == plan.contracts.get(ticker));
        self.desired = plan.contracts;
        self.root = plan.root;
        self.expiration = plan.expiration;
        self.discovered_ms = now;
        self.detail = "Esperando cotizaciones WebSocket de los pares SPX".into();
    }
    fn fail(&mut self, detail: &str) {
        self.desired.clear();
        self.subscribed.clear();
        self.quotes.clear();
        self.detail = detail.into();
    }
    pub fn reset(&mut self) {
        self.quotes.clear();
    }
    pub fn contracts(&self, now: u64, budget: usize) -> BTreeMap<String, Contract> {
        if !fresh(self.discovered_ms, now, 900_000) || self.desired.len() > budget {
            return BTreeMap::new();
        }
        self.desired
            .values()
            .map(|m| {
                (
                    m.ticker.clone(),
                    Contract {
                        ticker: m.ticker.clone(),
                        kind: if m.side == Side::Call { "call" } else { "put" }.into(),
                        strike: m.strike,
                        expiration: m.expiration.clone(),
                        shares_per_contract: None,
                        premium_multiplier: None,
                    },
                )
            })
            .collect()
    }
    pub fn install(&mut self, contracts: &BTreeMap<String, Contract>) {
        self.subscribed = self
            .desired
            .iter()
            .filter(|(id, m)| {
                contracts.get(*id).is_some_and(|c| {
                    c.strike == m.strike
                        && c.expiration == m.expiration
                        && c.kind == if m.side == Side::Call { "call" } else { "put" }
                })
            })
            .map(|(id, _)| id.clone())
            .collect();
        self.quotes.retain(|id, _| self.subscribed.contains(id));
    }
    pub fn accepts(&self, ticker: &str) -> bool {
        self.subscribed.contains(ticker) && self.desired.contains_key(ticker)
    }
    pub fn receive(&mut self, event: &Event, now: u64) -> bool {
        let Event::Quote { sym, bp, ap, t, .. } = event else {
            return false;
        };
        if !self.accepts(sym) {
            return false;
        }
        if self.quotes.get(sym).is_some_and(|q| q.quote_ms > *t) {
            return false;
        }
        if !bp.is_finite() || !ap.is_finite() || *bp < 0.0 || ap < bp || !fresh(*t, now, 15_000) {
            self.quotes.remove(sym);
            return false;
        }
        let m = &self.desired[sym];
        self.quotes.insert(
            sym.clone(),
            Quote {
                root: m.root.clone(),
                expiration: m.expiration.clone(),
                side: m.side,
                strike: m.strike,
                bid: *bp,
                ask: *ap,
                quote_ms: *t,
            },
        );
        true
    }
    pub fn snapshot(&self, now: u64, connected: bool) -> Status {
        let mut out = Status {
            phase: "waiting".into(),
            detail: self.detail.clone(),
            contracts: self.desired.len(),
            ..Default::default()
        };
        if !connected {
            out.detail = "Esperando conexión Options".into();
            return out;
        }
        if self.desired.is_empty() {
            if out.detail.is_empty() {
                out.detail = "Esperando descubrimiento de pares SPX".into();
            }
            return out;
        }
        if !fresh(self.discovered_ms, now, 900_000) {
            out.phase = "stale".into();
            out.detail = "Selección SPX caducada".into();
            return out;
        }
        if self.subscribed.len() != self.desired.len() {
            out.detail = "Esperando suscripción completa o capacidad de canales".into();
            return out;
        }
        let quotes: Vec<_> = self
            .quotes
            .values()
            .filter(|q| fresh(q.quote_ms, now, 15_000))
            .cloned()
            .collect();
        match synthetic::spx(&self.root, &self.expiration, &quotes, now, true) {
            Ok(estimate) => {
                out.phase = if estimate.usable_for_analysis {
                    "ready"
                } else {
                    "insufficient"
                }
                .into();
                out.detail = "Estimación por paridad CALL/PUT · no es un tick del índice".into();
                out.estimate = Some(estimate);
            }
            Err(_) => {
                out.detail =
                    "Esperando al menos tres pares contemporáneos y con spread válido".into();
            }
        }
        out
    }
}
fn metadata(row: &Value) -> Result<Metadata, &'static str> {
    let d = &row["details"];
    let ticker = d["ticker"].as_str().ok_or("Contrato SPX sin ticker")?;
    let root = options::underlying(ticker)
        .filter(|r| matches!(*r, "SPX" | "SPXW"))
        .ok_or("Contrato ajeno a SPX")?;
    if row["underlying_asset"]["ticker"]
        .as_str()
        .is_some_and(|s| !matches!(s, "SPX" | "I:SPX"))
    {
        return Err("Subyacente SPX inconsistente");
    }
    let expiration = d["expiration_date"]
        .as_str()
        .filter(|d| calendar::day_bounds(d).is_some())
        .ok_or("Vencimiento SPX inválido")?;
    let strike = d["strike_price"]
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or("Strike SPX inválido")?;
    let side = match d["contract_type"].as_str() {
        Some("call") => Side::Call,
        Some("put") => Side::Put,
        _ => return Err("Tipo SPX inválido"),
    };
    let tail = &ticker[ticker.len() - 15..];
    let encoded_strike = tail[7..]
        .parse::<u64>()
        .map_err(|_| "Strike SPX no verificable")? as f64
        / 1000.;
    if tail[..6] != expiration[2..].replace('-', "")
        || encoded_strike != strike
        || tail.as_bytes()[6] != if side == Side::Call { b'C' } else { b'P' }
    {
        return Err("Metadata SPX no coincide con el contrato");
    }
    Ok(Metadata {
        ticker: ticker.into(),
        root: root.into(),
        expiration: expiration.into(),
        strike,
        side,
    })
}
fn plan(rows: &[Value], now: u64) -> Result<Plan, &'static str> {
    if rows.len() > 5000 {
        return Err("Cadena SPX excede el límite");
    }
    let mut groups: BTreeMap<(String, String), Vec<(Metadata, Quote)>> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        let m = metadata(row)?;
        if !seen.insert(m.ticker.clone()) {
            return Err("Contrato SPX duplicado");
        }
        let q = &row["last_quote"];
        if q["timeframe"] != "REAL-TIME" {
            continue;
        }
        let (Some(bid), Some(ask), Some(ns)) = (
            q["bid"].as_f64(),
            q["ask"].as_f64(),
            q["last_updated"].as_u64(),
        ) else {
            continue;
        };
        if !bid.is_finite()
            || !ask.is_finite()
            || bid < 0.
            || ask < bid
            || !fresh(ns / 1_000_000, now, 15_000)
        {
            continue;
        }
        let quote = Quote {
            root: m.root.clone(),
            expiration: m.expiration.clone(),
            side: m.side,
            strike: m.strike,
            bid,
            ask,
            quote_ms: ns / 1_000_000,
        };
        groups
            .entry((m.expiration.clone(), m.root.clone()))
            .or_default()
            .push((m, quote));
    }
    for ((expiration, root), rows) in groups {
        let quotes: Vec<_> = rows.iter().map(|(_, q)| q.clone()).collect();
        let Ok(estimate) = synthetic::spx(&root, &expiration, &quotes, now, true) else {
            continue;
        };
        if !estimate.usable_for_analysis {
            continue;
        }
        let contracts = rows
            .into_iter()
            .filter(|(m, _)| estimate.pairs.iter().any(|p| p.strike == m.strike))
            .map(|(m, _)| (m.ticker.clone(), m))
            .collect();
        return Ok(Plan {
            contracts,
            root,
            expiration,
        });
    }
    Err("SPX_NO_VALID_PAIRS")
}
async fn discover(key: &Credentials) -> Result<Plan, &'static str> {
    let today = calendar::date_at(now_ms()).ok_or("Fecha SPX no disponible")?;
    let mut last = today;
    for _ in 0..10 {
        last = last.succ_opt().ok_or("Fecha SPX inválida")?;
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Cliente SPX no disponible")?;
    let reference = "/v3/reference/options/contracts";
    let mut floor = today.to_string();
    let mut exclusive = false;
    for _ in 0..3 {
        let url = crate::option_snapshot::url(
            reference,
            &[
                ("underlying_ticker", "SPX".into()),
                (
                    if exclusive {
                        "expiration_date.gt"
                    } else {
                        "expiration_date.gte"
                    },
                    floor.clone(),
                ),
                ("expiration_date.lte", last.to_string()),
                ("sort", "expiration_date".into()),
                ("order", "asc".into()),
                ("limit", "1".into()),
            ],
        )?;
        let data = crate::option_snapshot::page(&client, key, &url).await?;
        let Some(expiry) = data["results"]
            .as_array()
            .and_then(|r| r.first())
            .and_then(|r| r["expiration_date"].as_str())
        else {
            return Err("No hay vencimientos SPX próximos");
        };
        if calendar::day_bounds(expiry).is_none()
            || expiry < floor.as_str()
            || (exclusive && expiry == floor)
            || expiry > last.to_string().as_str()
        {
            return Err("Vencimiento SPX fuera de la consulta");
        }
        let path = "/v3/snapshot/options/SPX";
        let mut next = crate::option_snapshot::url(
            path,
            &[("expiration_date", expiry.into()), ("limit", "250".into())],
        )?;
        let mut seen = BTreeSet::new();
        let mut rows = vec![];
        for index in 0..20 {
            if !seen.insert(next.to_string()) {
                return Err("Cursor SPX repetido");
            }
            let data = crate::option_snapshot::page(&client, key, &next).await?;
            let batch = data["results"].as_array().ok_or("Cadena SPX incompleta")?;
            if rows.len() + batch.len() > 5000 {
                return Err("Cadena SPX excede el límite");
            }
            if batch
                .iter()
                .any(|r| r["details"]["expiration_date"] != expiry)
            {
                return Err("Vencimiento SPX mezclado");
            }
            rows.extend(batch.iter().cloned());
            match data["next_url"].as_str() {
                Some(url) if index < 19 => next = checked_url(url, path)?,
                Some(_) => return Err("Paginación SPX incompleta"),
                None => break,
            }
        }
        match plan(&rows, now_ms()) {
            Ok(p) => return Ok(p),
            Err("SPX_NO_VALID_PAIRS") => {}
            Err(e) => return Err(e),
        }
        floor = expiry.into();
        exclusive = true;
    }
    Err("No hay pares SPX recientes para estimar el índice")
}
pub async fn worker(key: &Credentials, state: SharedState) {
    let mut timer = tokio::time::interval(Duration::from_secs(60));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut next_ms = 0;
    loop {
        timer.tick().await;
        let now = now_ms();
        if now < next_ms || state.lock().unwrap().options.phase != "streaming" {
            continue;
        }
        let result = discover(key).await;
        let mut s = state.lock().unwrap();
        match result {
            Ok(plan) => {
                s.spx_engine.replace(plan, now_ms());
                next_ms = now_ms() + 300_000;
            }
            Err(e) => {
                s.spx_engine.fail(e);
                next_ms = now_ms() + 60_000;
            }
        }
        s.scanner_options_notify.notify_one();
    }
}

#[cfg(test)]
fn fixture_rows(now: u64, root: &str) -> Vec<Value> {
    let mut rows = vec![];
    for strike in [5995., 6000., 6005.] {
        for (kind, code, sign) in [("call", "C", 1.), ("put", "P", -1.)] {
            let mid = 20. + sign * (6000. - strike) / 2.;
            rows.push(serde_json::json!({"details":{"ticker":format!("O:{root}260925{code}{:08}",(strike*1000.) as u64),"expiration_date":"2026-09-25","contract_type":kind,"strike_price":strike},"underlying_asset":{"ticker":"I:SPX"},"last_quote":{"bid":mid-0.1,"ask":mid+0.1,"last_updated":now*1_000_000,"timeframe":"REAL-TIME"}}));
        }
    }
    rows
}
#[cfg(test)]
pub(crate) fn fixture_engine(now: u64) -> Engine {
    let mut engine = Engine::default();
    engine.replace(plan(&fixture_rows(now, "SPXW"), now).unwrap(), now);
    let contracts = engine.contracts(now, 30);
    engine.install(&contracts);
    engine
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_keeps_roots_separate_and_requires_current_realtime_metadata() {
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let rows = fixture_rows(now, "SPXW");
        let selected = plan(&rows, now).unwrap();
        assert_eq!(selected.contracts.len(), 6);
        assert_eq!(selected.root, "SPXW");
        for variant in 0..5 {
            let mut bad = rows.clone();
            match variant {
                0 => {
                    for row in &mut bad {
                        row["last_quote"]["timeframe"] = "DELAYED".into();
                    }
                }
                1 => {
                    for row in &mut bad {
                        row["last_quote"]["last_updated"] =
                            (now.saturating_sub(15_001) * 1_000_000).into();
                    }
                }
                2 => bad[0]["details"]["strike_price"] = 6001.into(),
                3 => bad.push(bad[0].clone()),
                _ => bad[0]["underlying_asset"]["ticker"] = "QQQ".into(),
            }
            assert!(plan(&bad, now).is_err(), "variant {variant}");
        }
        let mut mixed: Vec<_> = fixture_rows(now, "SPX")
            .into_iter()
            .filter(|r| r["details"]["contract_type"] == "call")
            .collect();
        mixed.extend(
            rows.into_iter()
                .filter(|r| r["details"]["contract_type"] == "put"),
        );
        assert!(plan(&mixed, now).is_err());
    }
    #[test]
    fn websocket_quotes_drive_estimate_and_disconnect_rotation_or_staleness_revoke_it() {
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut engine = fixture_engine(now);
        assert!(engine.contracts(now, 5).is_empty());
        assert!(engine.snapshot(now, true).estimate.is_none());
        for row in fixture_rows(now, "SPXW") {
            let q = &row["last_quote"];
            let event=crate::decode_batch(&serde_json::json!([{"ev":"Q","sym":row["details"]["ticker"],"bp":q["bid"],"ap":q["ask"],"bs":1,"as":1,"t":now}]).to_string()).unwrap().remove(0);
            assert!(engine.receive(&event, now));
        }
        let ready = engine.snapshot(now, true);
        assert_eq!(ready.phase, "ready");
        assert_eq!(ready.estimate.unwrap().price, 6000.);
        assert!(!ready.entry_confirmed);
        assert!(engine.snapshot(now, false).estimate.is_none());
        assert!(engine.snapshot(now + 15_001, true).estimate.is_none());
        let bad=crate::decode_batch(&serde_json::json!([{"ev":"Q","sym":"O:SPXW260925C05995000","bp":1,"ap":2,"bs":1,"as":1,"t":now+1}]).to_string()).unwrap().remove(0);
        assert!(!engine.receive(&bad, now));
        assert_ne!(engine.snapshot(now, true).phase, "ready");
        engine.reset();
        assert!(engine.snapshot(now, true).estimate.is_none());
        engine.install(&BTreeMap::new());
        assert!(!engine.accepts("O:SPXW260925C06000000"));
    }
}
