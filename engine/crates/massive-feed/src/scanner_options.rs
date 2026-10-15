//! Bounded structural snapshots for the fixed Top5. No extra WebSocket is opened.
use crate::{
    credentials::Credentials,
    live::{now_ms, SharedState},
};
use serde::Serialize;
use snipers_core::{
    calendar::{self, Day},
    horizons::Horizon,
    options::Observation,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub daily_range:Option<snipers_core::daily_range::Range>,
    pub direction_oi: serde_json::Value,
    pub symbol: String,
    pub phase: String,
    pub detail: String,
    pub observed_ms: Option<u64>,
    pub reference_ms: Option<u64>,
    pub contracts: usize,
    pub horizons: Vec<Horizon>,
    pub entry_confirmed: bool,
}
#[derive(Clone, Default)]
struct Member {
    last_event_ms: Option<u64>,
    desired: BTreeMap<String, snipers_core::flow::Contract>,
    subscribed: BTreeMap<String, snipers_core::flow::Contract>,
    engine: snipers_core::flow::Engine,
    flow: snipers_core::flow::Snapshot,
    flow_second: Option<u64>,
    context: Context,
    reference: Option<(f64, u64)>,
    next_ms: u64,
}
#[derive(Clone, Default)]
pub struct Pool {
    pub capacity: usize,
    pub preparation: bool,
    pub background: bool,
    pub priority: Vec<String>,
    session: String,
    generation: u64,
    members: BTreeMap<String, Member>,
}
#[derive(Clone)]
struct Request {
    symbol: String,
    generation: u64,
    session: String,
    spot: f64,
    reference_ms: u64,
}
fn fresh(t: u64, now: u64, max: u64) -> bool {
    t <= now
        && now - t <= max
        && calendar::date_at(now).is_some()
        && calendar::date_at(t) == calendar::date_at(now)
}
impl Pool {
    pub fn etf_data(&mut self,symbol:&str,now:u64,connected:bool)->serde_json::Value {
        let Some(m)=self.members.get_mut(symbol) else{return serde_json::Value::Null};
        let subscribed=m.subscribed.len();
        serde_json::json!({"symbol":symbol,"at":now,"connected":connected,"tracked":m.desired.len(),"subscribed":subscribed,"lastEventMs":m.last_event_ms,"flow":m.engine.snapshot(now)})
    }
    pub fn symbols(&self)->Vec<String>{self.members.keys().cloned().collect()}
    /// Whole member plans only: a quote budget cannot silently certify a partial plan.
    pub fn contracts(
        &self,
        active: &str,
        now: u64,
        mut budget: usize,
    ) -> BTreeMap<String, snipers_core::flow::Contract> {
        let mut all = BTreeMap::new();
        let mut members:Vec<_>=self.members.iter().collect();
        members.sort_by_key(|(symbol,_)|self.priority.iter().position(|s|s==*symbol).unwrap_or(usize::MAX));
        for (symbol, m) in members {
            if symbol == active && !self.background
                || m.context.phase != "ready"
                || !m
                    .context
                    .observed_ms
                    .is_some_and(|t| fresh(t, now, 900_000))
                || m.desired.len() > budget
            {
                continue;
            }
            budget -= m.desired.len();
            all.extend(m.desired.clone());
        }
        all
    }
    pub fn install(
        &mut self,
        contracts: &BTreeMap<String, snipers_core::flow::Contract>,
        active: &str,
    ) {
        for (symbol, m) in &mut self.members {
            let selected = if symbol == active && !self.background {
                BTreeMap::new()
            } else {
                contracts
                    .iter()
                    .filter(|(ticker, _)| {
                        snipers_core::options::underlying(ticker) == Some(symbol.as_str()) && m.desired.contains_key(*ticker)
                    })
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            };
            if m.subscribed != selected {
                m.engine.set_contracts(selected.clone());
                m.subscribed = selected;
                m.flow_second = None;
            }
        }
    }
    pub fn reset_coverage(&mut self) {
        for m in self.members.values_mut() {
            m.last_event_ms=None;
            m.engine.reset_coverage();
            m.flow_second = None;
        }
    }
    pub fn invalidate_coverage(&mut self) {
        for m in self.members.values_mut() {
            m.engine.invalidate_coverage();
            m.flow_second = None;
        }
    }
    pub fn accepts(&self, ticker: &str) -> bool {
        snipers_core::options::underlying(ticker)
            .and_then(|s| self.members.get(s))
            .is_some_and(|m| m.subscribed.contains_key(ticker))
    }
    /// The shared transport deduplicates before calling this method.
    pub fn receive(&mut self,event:&crate::Event,now:u64)->bool {
        let accepted=self.receive_inner(event,now);
        if accepted {if let crate::Event::Trade{sym,t,..}|crate::Event::Quote{sym,t,..}=event {
            if let Some(m)=snipers_core::options::underlying(sym).and_then(|root|self.members.get_mut(root)) {m.last_event_ms=Some(m.last_event_ms.unwrap_or(0).max(*t));}
        }}
        accepted
    }
    pub fn bot_data(&mut self,symbol:&str,now:u64)->serde_json::Value {
        let Some(m)=self.members.get_mut(symbol) else {return serde_json::Value::Null};
        let complete=!m.desired.is_empty() && m.subscribed==m.desired;
        let expiry=m.desired.values().map(|c|c.expiration.clone()).min().unwrap_or_default();
        serde_json::json!({"flowExpiry":expiry,"flow":m.engine.snapshot_expiry(now,&expiry),"lastEventMs":if complete {m.last_event_ms}else{None},"context":m.context,"flowStatus":{"symbol":symbol,"at":now,"plannedContracts":m.desired.len(),"subscribedContracts":m.subscribed.len(),"completeSubscription":complete,"lastAcceptedEventMs":m.last_event_ms,"eventAgeMs":m.last_event_ms.filter(|t|*t<=now).map(|t|now-t),"reason":if !complete {"incomplete-subscription"}else if m.last_event_ms.is_none(){"waiting-own-events"}else if m.last_event_ms.is_some_and(|t|t>now){"waiting-clock"}else if m.last_event_ms.is_some_and(|t|now-t>15000){"stale-own-events"}else{"current"}}})
    }
    fn receive_inner(&mut self, event: &crate::Event, now: u64) -> bool {
        let ticker = match event {
            crate::Event::Trade { sym, .. } | crate::Event::Quote { sym, .. } => sym,
            _ => return false,
        };
        if !self.accepts(ticker) {
            return false;
        }
        let m = self
            .members
            .get_mut(snipers_core::options::underlying(ticker).unwrap())
            .unwrap();
        match event {
            crate::Event::Trade { p, s, t, .. } => {
                if !p.is_finite() || *p <= 0.0 || !s.is_finite() || *s <= 0.0 || *t > now {
                    m.engine.invalidate_coverage();
                    m.flow_second = None;
                    return false;
                }
                m.engine.trade(ticker, *p, *s, *t, now);
                true
            }
            crate::Event::Quote {
                bp,
                ap,
                bs,
                ask_size,
                t,
                ..
            } => {
                if *t > now {
                    m.engine.quote(ticker, 0.0, 0.0, now);
                    m.engine.invalidate_coverage();
                    m.flow_second = None;
                    return false;
                }
                m.engine
                    .quote_with_depth(ticker, *bp, *ap, *t, Some(*bs), Some(*ask_size));
                bp.is_finite() && ap.is_finite() && *bp > 0.0 && ap >= bp && *t <= now
            }
            _ => false,
        }
    }
    pub fn flow_read(
        &mut self,
        symbol: &str,
        direction: &str,
        now: u64,
        connected: bool,
    ) -> snipers_core::scanner_confirmation::FlowRead {
        let Some(m) = self.members.get_mut(symbol) else {
            return snipers_core::scanner_confirmation::flow(symbol, direction, None, now);
        };
        if m.flow_second != Some(now / 1000) {
            m.flow = m.engine.snapshot(now);
            m.flow_second = Some(now / 1000);
        }
        let windows: Vec<_> = m
            .flow
            .contract_windows
            .iter()
            .filter(|w| w.minutes == 1)
            .collect();
        let covered =
            !windows.is_empty() && m.subscribed == m.desired && windows.iter().all(|w| w.covered);
        let evidence = m
            .flow
            .windows
            .iter()
            .find(|w| w.minutes == 1)
            .map(|window| snipers_core::scanner_confirmation::FlowEvidence {
                symbol,
                calculated_ms: m.flow_second.unwrap_or(0) * 1000,
                verified: connected,
                coverage_complete: covered,
                window,
            });
        snipers_core::scanner_confirmation::flow(symbol, direction, evidence, now)
    }
    pub fn contract_windows(
        &self,
        symbol: &str,
        now: u64,
    ) -> &[snipers_core::flow::ContractWindow] {
        self.members
            .get(symbol)
            .filter(|m| {
                m.flow_second == Some(now / 1000)
                    && !m.desired.is_empty()
                    && m.desired == m.subscribed
            })
            .map_or(&[], |m| m.flow.contract_windows.as_slice())
    }
    pub fn liquidity(
        &self,
        symbol: &str,
        now: u64,
        connected: bool,
    ) -> snipers_core::option_liquidity::Read {
        self.members
            .get(symbol)
            .map(|m| {
                m.engine
                    .liquidity(now, connected && m.subscribed == m.desired)
            })
            .unwrap_or_default()
    }
    /// Invalid membership is an empty pool, never a partially accepted Top5.
    pub fn reconcile(&mut self, symbols: &[String], now: u64) -> bool {
        let session = calendar::date_at(now)
            .map(|d| d.to_string())
            .unwrap_or_default();
        let set: BTreeSet<_> = symbols.iter().cloned().collect();
        let valid = symbols.len() <= if self.background {self.capacity.clamp(10,20)}else{5}
            && set.len() == symbols.len()
            && set
                .iter()
                .all(|s| snipers_core::universe::stock_asset(s).is_some());
        let next = if valid { set } else { BTreeSet::new() };
        let current: BTreeSet<_> = self.members.keys().cloned().collect();
        let changed = self.session != session || current != next;
        if changed {
            self.generation = self.generation.wrapping_add(1);
            if self.session != session {
                self.members.clear();
            }
            self.session = session;
            self.members.retain(|symbol, _| next.contains(symbol));
            for symbol in next {
                self.members
                    .entry(symbol.clone())
                    .or_insert_with(|| Member {
                        context: Context {
                            symbol,
                            phase: "waiting".into(),
                            detail: "Esperando referencia RTH del activo".into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    });
            }
            // A request from the previous membership cannot keep a member loading.
            for member in self
                .members
                .values_mut()
                .filter(|m| m.context.phase == "loading")
            {
                member.context.phase = "waiting".into();
                member.next_ms = 0;
            }
        }
        changed
    }
    pub fn reference(&mut self, symbol: &str, price: f64, time: u64, now: u64) {
        if price.is_finite() && price > 0.0 && fresh(time, now, 120_000) {
            if let Some(m) = self
                .members
                .get_mut(symbol)
                .filter(|m| m.reference.is_none_or(|(_, old)| time >= old))
            {
                m.reference = Some((price, time));
            }
        }
    }
    fn next(&mut self, now: u64, active: &str) -> Option<Request> {
        let Day::Open(day) = calendar::schedule(calendar::date_at(now)?) else {
            return None;
        };
        let start=if self.preparation {day.open_ms.saturating_sub(19_800_000)} else {day.open_ms};
        if self.session != day.date || now < start || now >= day.close_ms {
            return None;
        }
        for (symbol, m) in &mut self.members {
            if symbol == active && !self.background || now < m.next_ms || m.context.phase == "loading" {
                continue;
            }
            let Some((spot, time)) = m.reference.filter(|(_, t)| fresh(*t, now, 120_000)) else {
                continue;
            };
            m.context.phase = "loading".into();
            m.context.detail = "Recuperando cadena propia del activo".into();
            m.context.horizons.clear();
            return Some(Request {
                symbol: symbol.clone(),
                generation: self.generation,
                session: self.session.clone(),
                spot,
                reference_ms: time,
            });
        }
        None
    }
    fn finish(
        &mut self,
        r: &Request,
        result: Result<(Observation, Vec<Horizon>), String>,
        now: u64,
    ) {
        if r.generation != self.generation || r.session != self.session {
            return;
        }
        let Some(m) = self.members.get_mut(&r.symbol) else {
            return;
        };
        m.next_ms = now.saturating_add(60_000);
        m.context.horizons.clear();
        m.context.observed_ms = None;
        m.context.daily_range = None;
        m.context.contracts = 0;
        let result = result.and_then(|(observation, horizons)| {
            if observation.schema != 1
                || observation.symbol != r.symbol
                || observation.observed_date != r.session
                || !fresh(observation.observed_ms, now, 900_000)
                || !fresh(r.reference_ms, now, 120_000)
                || observation.contracts.is_empty()
                || observation.contracts.len() > 20_000
                || observation.contracts.iter().any(|(key, c)| {
                    key != &c.ticker
                        || snipers_core::options::underlying(key) != Some(r.symbol.as_str())
                })
                || horizons.is_empty()
                || horizons
                    .iter()
                    .any(|h| h.observed_ms != observation.observed_ms)
            {
                Err("Cadena incompleta, referencia caducada o identidad distinta".into())
            } else {
                Ok((observation, horizons))
            }
        });
        match result {
            Ok((observation, horizons)) => {
                m.context.daily_range=snipers_core::daily_range::from_observation(&observation,r.spot).ok();
                m.context.direction_oi=crate::direction_context::oi(&observation);
                m.desired = contract_plan(&observation, &horizons, r.spot);
                m.context.phase = "ready".into();
                m.context.detail =
                    "Estructura REST · OI del último día negociado; no es flujo live".into();
                m.context.observed_ms = Some(observation.observed_ms);
                m.context.reference_ms = Some(r.reference_ms);
                m.context.contracts = observation.contracts.len();
                m.context.horizons = horizons;
                m.next_ms = now.saturating_add(300_000);
            }
            Err(error) => {
                m.desired.clear();
                m.context.phase = "error".into();
                m.context.detail = error;
            }
        }
    }
    pub fn snapshot(&self, now: u64, connected: bool) -> BTreeMap<String, Context> {
        self.members
            .iter()
            .map(|(symbol, m)| {
                let mut context = m.context.clone();
                if !connected {
                    context.phase = "disconnected".into();
                    context.detail = "Conexión Stocks no vigente".into();
                    context.horizons.clear();
                } else if context.phase == "ready"
                    && !context.observed_ms.is_some_and(|t| fresh(t, now, 900_000))
                {
                    context.phase = "stale".into();
                    context.detail = "Snapshot de opciones caducado".into();
                    context.horizons.clear();
                }
                (symbol.clone(), context)
            })
            .collect()
    }
}

fn contract_plan(
    observation: &Observation,
    horizons: &[Horizon],
    spot: f64,
) -> BTreeMap<String, snipers_core::flow::Contract> {
    let mut chosen = BTreeSet::new();
    let first = observation
        .contracts
        .values()
        .map(|c| c.expiration.as_str())
        .min();
    for kind in ["call", "put"] {
        let mut rows: Vec<_> = observation
            .contracts
            .values()
            .filter(|c| c.kind == kind && Some(c.expiration.as_str()) == first)
            .collect();
        rows.sort_by(|a, b| {
            (a.strike - spot)
                .abs()
                .total_cmp(&(b.strike - spot).abs())
                .then(a.ticker.cmp(&b.ticker))
        });
        chosen.extend(rows.into_iter().take(6).map(|c| c.ticker.clone()));
    }
    for h in horizons {
        for spec in snipers_core::wall_flow::structural_specs(&h.structure) {
            for c in observation.contracts.values().filter(|c| {
                c.kind == spec.kind
                    && c.strike.to_bits() == spec.strike.to_bits()
                    && h.expirations.contains(&c.expiration)
            }) {
                chosen.insert(c.ticker.clone());
            }
        }
    }
    // Never truncate structural targets and present them as fully subscribed.
    if chosen.len() > 128 {
        return BTreeMap::new();
    }
    chosen
        .into_iter()
        .filter_map(|ticker| {
            let c = observation.contracts.get(&ticker)?;
            Some((
                ticker.clone(),
                snipers_core::flow::Contract {
                    ticker,
                    kind: c.kind.clone(),
                    strike: c.strike,
                    expiration: c.expiration.clone(),
                    shares_per_contract: c.shares_per_contract,
                    premium_multiplier: c.premium_multiplier,
                },
            ))
        })
        .collect()
}

pub async fn worker(key: &Credentials, state: SharedState) {
    let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        timer.tick().await;
        let request = {
            let mut s = state.lock().unwrap();
            if s.stocks.phase != "streaming" {
                continue;
            }
            let active = s.symbol.clone();
            s.scanner_options
                .next(now_ms(), &active)
                .map(|r| (r, s.oi_directory.clone()))
        };
        let Some((request, directory)) = request else {
            continue;
        };
        let fetched = crate::option_snapshot::fetch(key, &request.symbol, request.spot, 4).await;
        let result = match fetched {
            Ok(observation) => {
                let spot = request.spot;
                tokio::task::spawn_blocking(move || {
                    let directory = directory.ok_or("Historial OI no configurado".to_owned())?;
                    let previous = crate::oi_store::previous(
                        &directory,
                        &observation.symbol,
                        &observation.observed_date,
                    )
                    .map_err(str::to_owned)?;
                    crate::oi_store::save(&directory, &observation).map_err(str::to_owned)?;
                    let horizons = snipers_core::horizons::calculate_with_history(
                        &observation,
                        spot,
                        "STOCKS_LAST_TRADE_OR_RTH_CLOSE",
                        previous.as_ref(),
                    )
                    .map_err(str::to_owned)?;
                    Ok((observation, horizons))
                })
                .await
                .unwrap_or_else(|_| Err("No se pudo calcular el contexto de opciones".into()))
            }
            Err(error) => Err(error.into()),
        };
        let mut s = state.lock().unwrap();
        s.scanner_options.finish(&request, result, now_ms());
        s.scanner_options_notify.notify_one();
    }
}

#[cfg(test)]
pub(crate) fn fixture_pool(now: u64) -> Pool {
    let mut pool = Pool::default();
    pool.reconcile(&["SPY".into()], now);
    pool.reference("SPY", 500.0, now, now);
    let request = pool.next(now, "QQQ").unwrap();
    pool.finish(&request, Ok(tests::fixture(now)), now);
    pool
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_small_clock_lead_requires_event_replay_for_background_flow() {
        let now=calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut pool=super::fixture_pool(now);pool.background=true;
        let plan=pool.contracts("SPY",now,1000);pool.install(&plan,"SPY");
        let ticker=plan.keys().next().unwrap().clone();
        let event=crate::Event::Quote{sym:ticker,bp:1.,ap:1.01,bs:10.,ask_size:10.,t:now+650};
        assert!(!pool.receive(&event,now));
        assert!(pool.bot_data("SPY",now+1000)["lastEventMs"].is_null(),"Snapshot reads do not replay a discarded quote");
        assert!(pool.receive(&event,now+650),"The same quote is valid once its original timestamp arrives");
        assert_eq!(pool.bot_data("SPY",now+1000)["lastEventMs"],now+650);
    }
    #[test] fn preparation_allows_bounded_premarket_only_when_enabled(){
        let now=calendar::legacy_eastern_ms("2026-09-24 08:00:00").unwrap();
        let mut pool=Pool::default();pool.background=true;
        pool.reconcile(&["SPY".into()],now);pool.reference("SPY",500.,now,now);
        assert!(pool.next(now,"QQQ").is_none());pool.preparation=true;
        let request=pool.next(now,"QQQ").unwrap();pool.finish(&request,Ok(fixture(now)),now);
        assert_eq!(pool.bot_data("SPY",now)["context"]["directionOi"]["call"].as_f64(),Some(100.));
        assert!(pool.next(now+1000,"QQQ").is_none());
        let mut early=Pool::default();early.background=true;early.preparation=true;
        let before=calendar::legacy_eastern_ms("2026-09-24 03:59:00").unwrap();
        early.reconcile(&["SPY".into()],before);early.reference("SPY",500.,before,before);assert!(early.next(before,"QQQ").is_none());
    }

    #[test] fn background_mode_keeps_selected_asset_in_own_pool_and_requires_own_events(){
        let now=snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut pool=super::fixture_pool(now);pool.background=true;
        let plan=pool.contracts("SPY",now,1000);assert!(!plan.is_empty());pool.install(&plan,"SPY");
        assert!(pool.bot_data("SPY",now)["lastEventMs"].is_null());
        let ticker=plan.keys().next().unwrap().clone();
        assert!(pool.receive(&crate::Event::Quote{sym:ticker,bp:1.,ap:1.01,bs:10.,ask_size:10.,t:now},now));
        assert_eq!(pool.bot_data("SPY",now)["lastEventMs"],now);
        assert!(pool.bot_data("QQQ",now).is_null());pool.reset_coverage();assert!(pool.bot_data("SPY",now)["lastEventMs"].is_null());
    }
    pub(super) fn fixture(now: u64) -> (Observation, Vec<Horizon>) {
        let mut contract=snipers_core::options::normalize(&serde_json::json!({"details":{"ticker":"O:SPY260924C00500000","strike_price":500,"expiration_date":"2026-09-24","contract_type":"call","shares_per_contract":100},"open_interest":100,"day":{"volume":50},"greeks":{"delta":0.5,"gamma":0.01},"implied_volatility":0.2}),"SPY").unwrap();
        contract.premium_multiplier = Some(100.0);
        let observation = Observation {
            schema: 1,
            symbol: "SPY".into(),
            observed_ms: now,
            observed_date: "2026-09-24".into(),
            oi_reference_date: None,
            contracts: [(contract.ticker.clone(), contract)].into(),
        };
        let horizons = snipers_core::horizons::calculate(&observation, 500.0, "TEST").unwrap();
        (observation, horizons)
    }
    #[test]
    fn quote_budget_rotation_and_reconnect_do_not_certify_missing_members() {
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut pool = fixture_pool(now);
        assert!(pool.contracts("QQQ", now, 0).is_empty());
        assert!(pool.contracts("SPY", now, 1000).is_empty());
        assert!(pool.contracts("QQQ", now + 900_001, 1000).is_empty());
        let contracts = pool.contracts("QQQ", now, 1000);
        assert_eq!(contracts.len(), 1);
        pool.install(&contracts, "QQQ");
        let ticker = "O:SPY260924C00500000";
        assert!(pool.accepts(ticker));
        assert!(!pool.accepts("O:QQQ260924C00500000"));
        let event = |ev: &str, t: u64| {
            serde_json::from_value::<crate::Event>(if ev == "Q" {
                serde_json::json!({"ev":"Q","sym":ticker,"bp":1.0,"ap":1.1,"bs":10,"as":10,"t":t})
            } else {
                serde_json::json!({"ev":"T","sym":ticker,"p":1.1,"s":1,"t":t})
            })
            .unwrap()
        };
        for t in [now, now + 60_000] {
            assert!(pool.receive(&event("Q", t), t));
            assert!(pool.receive(&event("T", t), t));
        }
        assert_eq!(
            pool.flow_read("SPY", "ALCISTA", now + 60_000, true).aligned,
            Some(true)
        );
        assert!(!pool.flow_read("SPY", "ALCISTA", now + 60_000, false).ready);
        pool.reset_coverage();
        assert!(!pool.flow_read("SPY", "ALCISTA", now + 60_000, true).ready);
        assert!(!pool.receive(&event("Q", now + 60_001), now + 60_000));
        pool.install(&BTreeMap::new(), "QQQ");
        assert!(!pool.accepts(ticker));
        pool.reconcile(&[], now);
        pool.reconcile(&["SPY".into()], now);
        assert!(pool.contracts("QQQ", now, 1000).is_empty());
    }
    #[test]
    fn complete_snapshots_publish_atomically_but_foreign_stale_and_removed_results_do_not() {
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut pool = Pool::default();
        pool.reconcile(&["SPY".into()], now);
        pool.reference("SPY", 500.0, now, now);
        let request = pool.next(now, "QQQ").unwrap();
        pool.finish(&request, Ok(fixture(now)), now);
        let snapshot = pool.snapshot(now, true);
        assert_eq!(snapshot["SPY"].phase, "ready");
        assert_eq!(snapshot["SPY"].contracts, 1);
        assert_eq!(snapshot["SPY"].horizons.len(), 3);
        assert!(!snapshot["SPY"].entry_confirmed);
        assert!(pool.next(now + 299_999, "QQQ").is_none());
        assert!(pool.snapshot(now, false)["SPY"].horizons.is_empty());
        assert!(pool.snapshot(now + 900_001, true)["SPY"]
            .horizons
            .is_empty());
        let (mut foreign, h) = fixture(now);
        foreign.symbol = "QQQ".into();
        pool.finish(&request, Ok((foreign, h)), now);
        assert_eq!(pool.snapshot(now, true)["SPY"].phase, "error");
        assert!(pool.snapshot(now, true)["SPY"].horizons.is_empty());
        pool.finish(&request, Ok(fixture(now)), now + 120_001);
        assert_eq!(pool.snapshot(now + 120_001, true)["SPY"].phase, "error");
        pool.reconcile(&[], now);
        pool.finish(&request, Ok(fixture(now)), now);
        assert!(pool.snapshot(now, true).is_empty());
    }
    #[test]
    fn membership_bounds_generation_and_reference_age_control_requests() {
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut pool = Pool::default();
        pool.reconcile(&["QQQ".into(), "SPY".into()], now);
        assert!(pool.next(now, "QQQ").is_none());
        pool.reference("SPY", 500.0, now + 1, now);
        assert!(pool.next(now, "QQQ").is_none());
        pool.reference("SPY", 500.0, now, now);
        let request = pool.next(now, "QQQ").unwrap();
        assert_eq!(request.symbol, "SPY");
        assert!(pool.next(now, "QQQ").is_none());
        pool.reconcile(&["QQQ".into()], now);
        pool.finish(&request, Err("obsolete".into()), now);
        assert!(!pool.snapshot(now, true).contains_key("SPY"));
        pool.reconcile(&["SPY".into(), "SPY".into()], now);
        assert!(pool.snapshot(now, true).is_empty());
        pool.reconcile(&["SPX".into()], now);
        assert!(pool.snapshot(now, true).is_empty());
    }
    #[test]
    fn failure_has_bounded_retry_and_session_restart_drops_context() {
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut pool = Pool::default();
        pool.reconcile(&["SPY".into()], now);
        pool.reference("SPY", 500.0, now, now);
        let request = pool.next(now, "QQQ").unwrap();
        pool.finish(&request, Err("test failure".into()), now);
        assert_eq!(pool.snapshot(now, true)["SPY"].phase, "error");
        assert!(pool.next(now + 59_999, "QQQ").is_none());
        assert!(pool.next(now + 60_000, "QQQ").is_some());
        assert_eq!(pool.snapshot(now, false)["SPY"].phase, "disconnected");
        pool.reconcile(&["SPY".into()], now + 86_400_000);
        assert!(pool.next(now + 86_400_000, "QQQ").is_none());
        assert_eq!(
            pool.snapshot(now + 86_400_000, true)["SPY"].phase,
            "waiting"
        );
    }
}
