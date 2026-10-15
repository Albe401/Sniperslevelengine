//! Option flow is inferred from contemporaneous quotes, never exchange trade intent.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const QUOTE_MAX_AGE_MS: u64 = 2_000;
const RETENTION_MS: u64 = 30 * 60_000;
const MAX_EVENTS: usize = 100_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    pub ticker: String,
    pub kind: String,
    pub strike: f64,
    pub expiration: String,
    pub shares_per_contract: Option<f64>,
    /// Distinct from deliverable share count; None for unverified nonstandard contracts.
    pub premium_multiplier: Option<f64>,
}

#[derive(Clone)]
struct Quote {
    bid: f64,
    ask: f64,
    timestamp_ms: u64,
    bid_size: Option<f64>,
    ask_size: Option<f64>,
}
#[derive(Clone, Serialize)]
pub struct FlowEvent {
    pub timestamp_ms: u64,
    pub ticker: String,
    pub kind: String,
    pub price: f64,
    pub size: f64,
    pub premium: Option<f64>,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub side: String,
    pub label: String,
    pub confidence: String,
    pub quote_state: String,
}
#[derive(Clone, Default, Serialize)]
pub struct Bucket {
    pub contracts: f64,
    pub premium: f64,
    pub trades: usize,
}
#[derive(Clone, Serialize)]
pub struct Window {
    pub minutes: u64,
    pub event_count: usize,
    pub contracts: f64,
    /// Sum of verified premiums only; premium_complete must also be checked.
    pub premium: f64,
    pub premium_complete: bool,
    pub bullish_premium: f64,
    pub bearish_premium: f64,
    pub bullish_pct: Option<f64>,
    pub bearish_pct: Option<f64>,
    pub direction: String,
    pub buckets: BTreeMap<String, Bucket>,
    /// Inference coverage on retained contracts, not reception coverage of the market.
    pub classification_quality: ClassificationQuality,
}
#[derive(Clone, Default, Serialize)]
pub struct ClassificationQuality {
    pub classified_contracts: f64,
    pub unknown_contracts: f64,
    pub classified_pct: Option<f64>,
    pub unknown_reasons: BTreeMap<String, Bucket>,
    pub confidence_contracts: BTreeMap<String, f64>,
    pub basis: &'static str,
    pub reception_coverage_pct: Option<f64>,
}
#[derive(Clone, Default, Serialize)]
pub struct Snapshot {
    pub asof_ms: u64,
    /// Current Eastern-date expiration only; never falls back to next expiry.
    pub intraday_windows: Vec<Window>,
    pub expiry_windows: BTreeMap<String, Window>,
    pub contract_windows: Vec<ContractWindow>,
    pub bid_ask: Vec<BidAskWindow>,
    pub windows: Vec<Window>,
    pub recent: Vec<FlowEvent>,
    pub capacity_drops: u64,
    pub rejected_trades: u64,
    pub quote_max_age_ms: u64,
}
#[derive(Clone, Default)]
pub struct Engine {
    stream_since: BTreeMap<String, u64>,
    coverage_blocked: bool,
    contracts: BTreeMap<String, Contract>,
    quotes: BTreeMap<String, Quote>,
    events: BTreeMap<(u64, u64), FlowEvent>,
    sequence: u64,
    capacity_drops: u64,
    rejected_trades: u64,
    incomplete_through_ms: u64,
}
fn positive(n: f64) -> bool {
    n.is_finite() && n > 0.0
}
impl Engine {
    /// A reconnect starts a new observation interval; retained trades are context only.
    pub fn reset_coverage(&mut self) {
        self.stream_since.clear();
        self.quotes.clear();
        self.coverage_blocked = false;
    }
    pub fn invalidate_coverage(&mut self) {
        self.coverage_blocked = true;
    }
    pub fn set_contracts(&mut self, contracts: BTreeMap<String, Contract>) {
        // Current-universe analytics must not retain premiums from retired or
        // redefined contracts. The transport journal preserves the raw history.
        self.events.retain(|_, event| {
            contracts.contains_key(&event.ticker)
                && self.contracts.get(&event.ticker) == contracts.get(&event.ticker)
        });
        self.stream_since.retain(|ticker, _| {
            self.contracts.get(ticker) == contracts.get(ticker) && contracts.contains_key(ticker)
        });
        self.quotes.retain(|symbol, _| {
            contracts.contains_key(symbol) && self.contracts.get(symbol) == contracts.get(symbol)
        });
        self.contracts = contracts;
    }
    pub fn quote(&mut self, ticker: &str, bid: f64, ask: f64, timestamp_ms: u64) {
        self.quote_with_depth(ticker, bid, ask, timestamp_ms, None, None);
    }
    pub fn quote_with_depth(
        &mut self,
        ticker: &str,
        bid: f64,
        ask: f64,
        timestamp_ms: u64,
        bid_size: Option<f64>,
        ask_size: Option<f64>,
    ) {
        if !self.contracts.contains_key(ticker) {
            return;
        }
        if self
            .quotes
            .get(ticker)
            .is_some_and(|q| q.timestamp_ms > timestamp_ms)
        {
            return;
        }
        // Retain invalid quote state, so a crossed update cannot leave an older valid NBBO active.
        self.quotes.insert(
            ticker.into(),
            Quote {
                bid,
                ask,
                timestamp_ms,
                bid_size,
                ask_size,
            },
        );
    }
    pub fn liquidity(&self, now: u64, connected: bool) -> crate::option_liquidity::Read {
        let rows: Vec<_> = self
            .contracts
            .keys()
            .map(|ticker| {
                let quote = self.quotes.get(ticker);
                crate::option_liquidity::Quote {
                    ticker: ticker.clone(),
                    bid: quote.map(|q| q.bid),
                    ask: quote.map(|q| q.ask),
                    bid_size: quote.and_then(|q| q.bid_size),
                    ask_size: quote.and_then(|q| q.ask_size),
                    timestamp_ms: quote.map(|q| q.timestamp_ms),
                }
            })
            .collect();
        crate::option_liquidity::evaluate(&rows, now, connected)
    }
    pub fn trade(&mut self, ticker: &str, price: f64, size: f64, timestamp_ms: u64, now_ms: u64) {
        if !positive(price)
            || !positive(size)
            // Retain small clock leads; every published window excludes events after now.
            || timestamp_ms > now_ms.saturating_add(1_000)
            || now_ms.saturating_sub(timestamp_ms) > RETENTION_MS
        {
            self.rejected_trades += 1;
            self.coverage_blocked = true;
            return;
        }
        let Some(meta) = self.contracts.get(ticker) else {
            self.rejected_trades += 1;
            self.coverage_blocked = true;
            return;
        };
        let mut side = "UNKNOWN";
        let mut confidence = "LOW";
        let mut bid = None;
        let mut ask = None;
        let quote_state = match self.quotes.get(ticker) {
            None => "missing",
            Some(q) if q.timestamp_ms > timestamp_ms => "future",
            Some(q) if timestamp_ms - q.timestamp_ms > QUOTE_MAX_AGE_MS => "stale",
            Some(q) if !positive(q.bid) || !positive(q.ask) || q.ask < q.bid => "invalid",
            Some(q) => {
                bid = Some(q.bid);
                ask = Some(q.ask);
                let tolerance = 0.01_f64.max((q.ask - q.bid) * 0.05);
                if price >= q.ask - tolerance {
                    side = "BUY";
                    confidence = "HIGH";
                } else if price <= q.bid + tolerance {
                    side = "SELL";
                    confidence = "HIGH";
                } else if price > (q.bid + q.ask) / 2.0 {
                    side = "BUY";
                    confidence = "MEDIUM";
                } else if price < (q.bid + q.ask) / 2.0 {
                    side = "SELL";
                    confidence = "MEDIUM";
                }
                "valid"
            }
        };
        let premium = meta
            .premium_multiplier
            .filter(|m| positive(*m))
            .map(|m| price * size * m)
            .filter(|v| v.is_finite());
        let event = FlowEvent {
            timestamp_ms,
            ticker: ticker.into(),
            kind: meta.kind.clone(),
            price,
            size,
            premium,
            bid,
            ask,
            side: side.into(),
            label: format!("{} {side}", meta.kind.to_uppercase()),
            confidence: confidence.into(),
            quote_state: quote_state.into(),
        };
        self.sequence += 1;
        self.stream_since
            .entry(ticker.into())
            .and_modify(|since| {
                if *since > now_ms
                    || crate::calendar::date_at(*since) != crate::calendar::date_at(now_ms)
                {
                    *since = now_ms;
                }
            })
            .or_insert(now_ms);
        self.events.insert((timestamp_ms, self.sequence), event);
        self.purge(now_ms);
        while self.events.len() > MAX_EVENTS {
            let ((time, _), _) = self.events.pop_first().unwrap();
            self.incomplete_through_ms = self
                .incomplete_through_ms
                .max(time.saturating_add(RETENTION_MS));
            self.capacity_drops += 1;
        }
    }
    fn purge(&mut self, now: u64) {
        let cutoff = now.saturating_sub(RETENTION_MS);
        while self
            .events
            .first_key_value()
            .is_some_and(|((t, _), _)| *t < cutoff)
        {
            self.events.pop_first();
        }
    }
    pub fn snapshot_expiry(&mut self,now:u64,expiration:&str)->Snapshot {
        let mut snapshot=self.snapshot(now);
        snapshot.intraday_windows=if self.contracts.values().any(|c|c.expiration==expiration){[1,5,15,30].into_iter().map(|m|self.window_filtered(m,now,Some(expiration))).collect()}else{vec![]};
        snapshot
    }
    pub fn snapshot(&mut self, now: u64) -> Snapshot {
        self.purge(now);
        Snapshot {
            asof_ms: now,
            bid_ask: self.bid_ask_windows(now),
            intraday_windows: crate::calendar::date_at(now)
                .map(|d| d.to_string())
                .filter(|date| self.contracts.values().any(|c| c.expiration == *date))
                .map(|date| {
                    [1, 5, 15, 30]
                        .into_iter()
                        .map(|m| self.window_filtered(m, now, Some(&date)))
                        .collect()
                })
                .unwrap_or_default(),
            expiry_windows: self
                .contracts
                .values()
                .map(|c| c.expiration.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .map(|expiry| {
                    let window = self.window_filtered(30, now, Some(&expiry));
                    (expiry, window)
                })
                .collect(),
            contract_windows: [1, 5]
                .into_iter()
                .flat_map(|m| self.contract_windows(m, now))
                .collect(),
            windows: [1, 5, 15, 30]
                .into_iter()
                .map(|minutes| self.window(minutes, now))
                .collect(),
            recent: self
                .events
                .range(..=(now, u64::MAX))
                .rev()
                .take(50)
                .map(|(_, e)| e.clone())
                .collect(),
            capacity_drops: self.capacity_drops,
            rejected_trades: self.rejected_trades,
            quote_max_age_ms: QUOTE_MAX_AGE_MS,
        }
    }
    /// Today's expiry only; adding NEXT4 structural contracts must not change the
    /// local DAY comparison. Coverage is rebuilt after rotation/disconnection.
    pub fn local_volume(
        &self,
        symbol: &str,
        now: u64,
        verified: bool,
    ) -> crate::local_volume::Read {
        let expiry = crate::calendar::date_at(now)
            .map(|d| d.to_string())
            .unwrap_or_default();
        let windows: serde_json::Map<String, serde_json::Value> = [1, 5, 15]
            .into_iter()
            .map(|minutes| {
                (
                    format!("{minutes}m"),
                    serde_json::to_value(self.window_filtered(minutes, now, Some(&expiry)))
                        .unwrap_or_default(),
                )
            })
            .collect();
        let last = self
            .events
            .values()
            .rev()
            .find(|e| {
                e.timestamp_ms <= now
                    && self
                        .contracts
                        .get(&e.ticker)
                        .is_some_and(|c| c.expiration == expiry)
            })
            .map(|e| e.timestamp_ms);
        let input = serde_json::json!({"symbol":symbol,"connected":verified,"authenticated":verified,"subscribed":verified,"windows":windows,"last_trade":{"timestamp":last.map(|ms|ms as f64/1000.)}});
        let mut read = crate::local_volume::evaluate(symbol, "DAY", &input, now);
        if read.usable {
            let rows: Vec<_> = self
                .contract_windows(5, now)
                .into_iter()
                .filter(|w| w.expiration == expiry)
                .collect();
            if rows.is_empty()
                || rows.iter().any(|w| !w.covered)
                || last.is_none_or(|t| t > now || now - t > 15000)
            {
                read.usable = false;
                read.state = "ESPERANDO".into();
                read.rvol_local = None;
                read.reason = "VENTANA PROPIA INCOMPLETA O PRECIO SIN VIGENCIA".into();
            }
        }
        read
    }
    fn bid_ask_windows(&self, now: u64) -> Vec<BidAskWindow> {
        let mut rows: BTreeMap<_, _> = self.contract_windows(5, now).into_iter().map(|w| {
            let row = BidAskWindow { ticker:w.ticker.clone(), kind:w.kind, strike:w.strike,
                expiration:w.expiration, covered:w.covered, bid:w.covered.then_some(0.),
                ask:w.covered.then_some(0.), unknown:w.covered.then_some(0.) };
            (w.ticker, row)
        }).collect();
        for (_, e) in self.events.range((now.saturating_sub(300_000), 0)..=(now, u64::MAX)) {
            if let Some(r) = rows.get_mut(&e.ticker).filter(|r| r.covered) {
                let quote = e.bid.zip(e.ask).filter(|(bid,ask)| ask > bid);
                let target = if e.quote_state == "valid" && quote.is_some_and(|(_,ask)| e.price >= ask) { &mut r.ask }
                    else if e.quote_state == "valid" && quote.is_some_and(|(bid,_)| e.price <= bid) { &mut r.bid }
                    else { &mut r.unknown };
                *target = target.map(|n| n + e.size);
            }
        }
        rows.into_values().collect()
    }
    fn contract_windows(&self, minutes: u64, now: u64) -> Vec<ContractWindow> {
        let cutoff = now.saturating_sub(minutes * 60_000);
        let mut rows: BTreeMap<_, _> = self
            .contracts
            .iter()
            .map(|(ticker, c)| {
                let since = self.stream_since.get(ticker).copied();
                let covered = !self.coverage_blocked
                    && self.incomplete_through_ms < now
                    && since.is_some_and(|s| {
                        s <= cutoff
                            && crate::calendar::date_at(s).is_some()
                            && crate::calendar::date_at(s) == crate::calendar::date_at(now)
                    });
                (
                    ticker.clone(),
                    ContractWindow {
                        ticker: ticker.clone(),
                        kind: c.kind.clone(),
                        strike: c.strike,
                        expiration: c.expiration.clone(),
                        minutes,
                        observed_since_ms: since,
                        covered,
                        contracts: covered.then_some(0.0),
                        premium: covered.then_some(0.0),
                        trades: covered.then_some(0),
                        reason: if covered {
                            "Ventana observada completa"
                        } else {
                            "Suscripción nueva, interrumpida o datos rechazados"
                        }
                        .into(),
                    },
                )
            })
            .collect();
        for (_, e) in self.events.range((cutoff, 0)..=(now, u64::MAX)) {
            if let Some(r) = rows.get_mut(&e.ticker).filter(|r| r.covered) {
                r.contracts = r.contracts.map(|n| n + e.size);
                r.premium = r.premium.zip(e.premium).map(|(n, p)| n + p);
                r.trades = r.trades.map(|n| n + 1);
            }
        }
        rows.into_values()
            .map(|mut r| {
                if r.contracts.is_some_and(|n| !n.is_finite())
                    || r.premium.is_some_and(|n| !n.is_finite())
                {
                    r.covered = false;
                    r.contracts = None;
                    r.premium = None;
                    r.trades = None;
                    r.reason = "Desbordamiento; ventana no certificada".into();
                }
                r
            })
            .collect()
    }
    fn window(&self, minutes: u64, now: u64) -> Window {
        self.window_filtered(minutes, now, None)
    }
    fn window_filtered(&self, minutes: u64, now: u64, expiration: Option<&str>) -> Window {
        self.window_scoped(minutes, now, expiration, None)
    }
    /// Full retained event window for an explicit zone, never the bounded recent-print list.
    pub fn zone_activity(&self, tickers: &std::collections::BTreeSet<String>, now: u64) -> Window {
        self.window_scoped(1, now, None, Some(tickers))
    }
    fn window_scoped(
        &self,
        minutes: u64,
        now: u64,
        expiration: Option<&str>,
        tickers: Option<&std::collections::BTreeSet<String>>,
    ) -> Window {
        let cutoff = now.saturating_sub(minutes * 60_000);
        let mut buckets: BTreeMap<String, Bucket> =
            ["CALL BUY", "CALL SELL", "PUT BUY", "PUT SELL", "UNKNOWN"]
                .into_iter()
                .map(|s| (s.into(), Bucket::default()))
                .collect();
        let mut count = 0;
        let mut quality=ClassificationQuality{basis:"quote_inference_on_observed_contracts",..Default::default()};
        let mut complete = self.incomplete_through_ms < now;
        if let Some(tickers) = tickers {
            complete &= !tickers.is_empty()
                && !self.coverage_blocked
                && tickers.iter().all(|t| {
                    self.contracts.contains_key(t)
                        && self.stream_since.get(t).is_some_and(|s| {
                            *s <= cutoff
                                && crate::calendar::date_at(*s).is_some()
                                && crate::calendar::date_at(*s) == crate::calendar::date_at(now)
                        })
                });
        }
        if let Some(expiry) = expiration {
            complete &= !self.coverage_blocked
                && self
                    .contracts
                    .values()
                    .filter(|c| c.expiration == expiry)
                    .all(|c| {
                        self.stream_since.get(&c.ticker).is_some_and(|s| {
                            *s <= cutoff
                                && crate::calendar::date_at(*s).is_some()
                                && crate::calendar::date_at(*s) == crate::calendar::date_at(now)
                        })
                    });
        }
        for (_, event) in self.events.range((cutoff, 0)..=(now, u64::MAX)) {
            if tickers.is_some_and(|t| !t.contains(&event.ticker)) {
                continue;
            }
            if expiration.is_some_and(|expiry| {
                !self
                    .contracts
                    .get(&event.ticker)
                    .is_some_and(|c| c.expiration == expiry)
            }) {
                continue;
            }
            let label = if buckets.contains_key(&event.label) {
                event.label.as_str()
            } else {
                "UNKNOWN"
            };
            let bucket = buckets.get_mut(label).unwrap();
            bucket.contracts += event.size;
            bucket.premium += event.premium.unwrap_or(0.0);
            bucket.trades += 1;
            if label=="UNKNOWN" {
                quality.unknown_contracts+=event.size;
                let reason=if event.quote_state=="valid" {"midpoint_or_ambiguous"}else{event.quote_state.as_str()};
                let row=quality.unknown_reasons.entry(reason.into()).or_default();
                row.contracts+=event.size;row.premium+=event.premium.unwrap_or(0.);row.trades+=1;
            }else{quality.classified_contracts+=event.size;}
            *quality.confidence_contracts.entry(event.confidence.clone()).or_default()+=event.size;
            complete &= event.premium.is_some();
            count += 1;
        }
        let bullish = buckets["CALL BUY"].premium + buckets["PUT SELL"].premium;
        let bearish = buckets["PUT BUY"].premium + buckets["CALL SELL"].premium;
        complete &= buckets
            .values()
            .all(|b| b.contracts.is_finite() && b.premium.is_finite())
            && (bullish + bearish).is_finite();
        let pct = if bullish + bearish > 0.0 {
            bullish / (bullish + bearish) * 100.0
        } else {
            50.0
        };
        let direction = if !complete {
            "INCOMPLETE"
        } else if count == 0 {
            "NO DATA"
        } else if bullish + bearish <= 0.0 {
            "UNCLASSIFIED"
        } else if pct >= 55.0 {
            "BULLISH FLOW"
        } else if 100.0 - pct >= 55.0 {
            "BEARISH FLOW"
        } else {
            "BALANCED FLOW"
        };
        let observed=quality.classified_contracts+quality.unknown_contracts;
        quality.classified_pct=if observed>0.&&observed.is_finite(){Some(100.*quality.classified_contracts/observed)}else{None};
        Window {
            minutes,
            event_count: count,
            contracts: buckets.values().map(|b| b.contracts).sum(),
            premium: buckets.values().map(|b| b.premium).sum(),
            premium_complete: complete,
            bullish_premium: bullish,
            bearish_premium: bearish,
            bullish_pct: (complete && bullish + bearish > 0.0).then_some(pct),
            bearish_pct: (complete && bullish + bearish > 0.0).then_some(100.0 - pct),
            direction: direction.into(),
            buckets,
            classification_quality:quality,
        }
    }
}

/// Executed contracts over five minutes; midpoint inference stays unclassified.
#[derive(Clone, Debug, Serialize)]
pub struct BidAskWindow {
    pub ticker: String,
    pub kind: String,
    pub strike: f64,
    pub expiration: String,
    pub covered: bool,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub unknown: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractWindow {
    pub ticker: String,
    pub kind: String,
    pub strike: f64,
    pub expiration: String,
    pub minutes: u64,
    pub observed_since_ms: Option<u64>,
    pub covered: bool,
    pub contracts: Option<f64>,
    pub premium: Option<f64>,
    pub trades: Option<usize>,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn classification_diagnostics_use_all_contracts_and_preserve_unknown(){
        let now=crate::calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();
        let mut e=Engine::default();e.set_contracts(BTreeMap::from([("A".into(),contract("A","call",Some(100.)))]));
        e.trade("A",1.5,5.,now,now);
        e.quote("A",1.,2.,now);e.trade("A",2.,10.,now,now);e.trade("A",1.,20.,now,now);e.trade("A",1.5,2.,now,now);
        e.trade("A",2.,3.,now+2001,now+2001);
        let r=e.snapshot(now+2001).windows.remove(0);let q=&r.classification_quality;
        assert_eq!(r.contracts,40.);assert_eq!(q.classified_contracts,30.);assert_eq!(q.unknown_contracts,10.);assert_eq!(q.classified_pct,Some(75.));
        assert_eq!(q.unknown_reasons["missing"].contracts,5.);assert_eq!(q.unknown_reasons["stale"].contracts,3.);assert_eq!(q.unknown_reasons["midpoint_or_ambiguous"].contracts,2.);
        assert_eq!(q.reception_coverage_pct,None);
        assert_eq!(e.snapshot(now-1).windows[0].classification_quality.classified_pct,None);
    }
    #[test]
    fn strike_bid_ask_keeps_midpoint_unknown_and_revokes_reconnect() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut e = Engine::default();
        e.set_contracts(BTreeMap::from([("A".into(),contract("A","call",Some(100.)))]));
        e.quote("A",1.,2.,now-360000);
        e.trade("A",2.,1.,now-360000,now-360000);
        for (price,size) in [(2.,20.),(1.,15.),(1.75,3.),(1.995,2.)] {
            e.quote("A",1.,2.,now);
            e.trade("A",price,size,now,now);
        }
        let r=e.snapshot(now).bid_ask.remove(0);
        assert_eq!((r.ask,r.bid,r.unknown),(Some(20.),Some(15.),Some(5.)));
        e.reset_coverage();
        let r=e.snapshot(now).bid_ask.remove(0);
        assert!(!r.covered);assert_eq!(r.ask,None);
    }
    #[test]
    fn zone_activity_uses_full_window_and_revokes_incomplete_universe() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-28 10:00:00").unwrap();
        let mut e = Engine::default();
        let mut c = contract("ZONE", "call", Some(100.));
        c.expiration = "2026-09-28".into();
        e.set_contracts([("ZONE".into(), c)].into());
        e.trade("ZONE", 2., 1., now, now);
        for t in 1..=500 {
            e.quote("ZONE", 1., 2., now + 60_000 + t);
            e.trade("ZONE", 2., 1., now + 60_000 + t, now + 60_000 + t);
        }
        let ids = ["ZONE".to_owned()].into();
        let w = e.zone_activity(&ids, now + 60_500);
        assert!(w.premium_complete);
        assert_eq!(w.event_count, 500);
        assert_eq!(w.premium, 100_000.);
        assert_eq!(w.buckets["CALL BUY"].trades, 500);
        assert!(
            !e.zone_activity(&["ZONE".into(), "MISSING".into()].into(), now + 60_500)
                .premium_complete
        );
        e.reset_coverage();
        assert!(!e.zone_activity(&ids, now + 60_500).premium_complete);
    }
    #[test]
    fn small_clock_lead_is_retained_but_not_published_until_due() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut e = Engine::default();
        let mut c = contract("TODAY", "call", Some(100.));
        c.expiration = "2026-09-24".into();
        e.set_contracts(BTreeMap::from([("TODAY".into(), c)]));
        e.quote("TODAY", 1., 2., now + 500);
        e.trade("TODAY", 2., 10., now + 600, now);
        let early = e.snapshot(now);
        assert_eq!(early.rejected_trades, 0);
        assert!(early.recent.is_empty());
        assert_eq!(early.windows[0].event_count, 0);
        assert!(!e.local_volume("QQQ", now, true).usable);
        let due = e.snapshot(now + 600);
        assert_eq!(due.windows[0].event_count, 1);
        assert_eq!(due.windows[0].buckets["CALL BUY"].premium, 2000.);
        assert_eq!(due.recent[0].timestamp_ms, now + 600);
        assert!(!e.coverage_blocked);
        e.trade("TODAY", 2., 10., now + 1001, now);
        assert_eq!(e.rejected_trades, 1);
        assert!(e.coverage_blocked);
    }

    #[test]
    fn local_volume_separates_expiries_and_loses_usability_after_disconnect_or_staleness() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut e = Engine::default();
        let mut today = contract("TODAY", "call", Some(100.));
        today.expiration = "2026-09-24".into();
        let mut next = today.clone();
        next.ticker = "NEXT".into();
        next.expiration = "2026-09-25".into();
        e.set_contracts(BTreeMap::from([
            ("TODAY".into(), today),
            ("NEXT".into(), next),
        ]));
        for age in [360000, 240000, 180000, 120000, 61000, 0] {
            let t = now - age;
            e.quote("TODAY", 1., 2., t);
            e.trade("TODAY", 2., 100., t, t);
        }
        e.quote("NEXT", 1., 2., now);
        e.trade("NEXT", 2., 20000., now, now);
        let read = e.local_volume("QQQ", now, true);
        assert!(read.usable);
        assert_eq!(read.rvol_local, Some(1.));
        assert_eq!(read.contracts_1m, Some(100.));
        assert_eq!(read.call_contracts_1m, Some(100.));
        assert!(!e.local_volume("QQQ", now, false).usable);
        assert!(!e.local_volume("QQQ", now + 16000, true).usable);
        e.reset_coverage();
        assert!(!e.local_volume("QQQ", now, true).usable);
    }
    #[test]
    fn rotation_cannot_carry_retired_or_redefined_premiums_into_new_universe() {
        let mut engine = Engine::default();
        let a = contract("A", "call", Some(100.0));
        let b = contract("B", "put", Some(100.0));
        engine.set_contracts([("A".into(), a.clone()), ("B".into(), b.clone())].into());
        for ticker in ["A", "B"] {
            engine.quote(ticker, 1.0, 1.1, 100_000);
            engine.trade(ticker, 1.1, 1.0, 100_000, 100_000);
        }
        assert_eq!(engine.snapshot(100_000).windows[0].event_count, 2);
        engine.set_contracts([("A".into(), a.clone())].into());
        let remaining = engine.snapshot(100_001);
        assert_eq!(remaining.windows[0].event_count, 1);
        assert_eq!(remaining.recent[0].ticker, "A");
        engine.set_contracts([("A".into(), a.clone()), ("B".into(), b)].into());
        assert_eq!(engine.snapshot(100_002).windows[0].event_count, 1);
        let mut changed = a;
        changed.premium_multiplier = Some(10.0);
        engine.set_contracts([("A".into(), changed)].into());
        assert_eq!(engine.snapshot(100_003).windows[0].event_count, 0);
    }
    #[test]
    fn expiry_direction_cannot_mix_dates_or_bypass_continuity() {
        let now = crate::calendar::day_bounds("2026-09-24").unwrap().0 + 36_000_000;
        let mut e = Engine::default();
        e.set_contracts(
            [("TODAY", "2026-09-24"), ("NEXT", "2026-09-25")]
                .into_iter()
                .map(|(ticker, expiry)| {
                    (
                        ticker.into(),
                        Contract {
                            ticker: ticker.into(),
                            kind: "call".into(),
                            strike: 500.0,
                            expiration: expiry.into(),
                            shares_per_contract: Some(100.0),
                            premium_multiplier: Some(100.0),
                        },
                    )
                })
                .collect(),
        );
        e.quote("TODAY", 1.0, 2.0, now);
        e.trade("TODAY", 2.0, 1.0, now, now);
        e.quote("NEXT", 1.0, 2.0, now);
        e.trade("NEXT", 1.0, 1000.0, now, now);
        assert!(!e.snapshot(now + 1_799_999).expiry_windows["2026-09-24"].premium_complete);
        let s = e.snapshot(now + 1_800_000);
        assert_eq!(s.expiry_windows["2026-09-24"].direction, "BULLISH FLOW");
        assert_eq!(s.expiry_windows["2026-09-25"].direction, "BEARISH FLOW");
        assert_eq!(s.expiry_windows["2026-09-24"].event_count, 1);
        assert_eq!(s.intraday_windows.len(), 4);
        let day30 = s.intraday_windows.iter().find(|w| w.minutes == 30).unwrap();
        assert_eq!(day30.event_count, 1);
        assert_eq!(day30.contracts, 1.0);
        assert_eq!(day30.direction, "BULLISH FLOW");
        assert_eq!(
            s.windows
                .iter()
                .find(|w| w.minutes == 30)
                .unwrap()
                .direction,
            "BEARISH FLOW"
        );
        e.reset_coverage();
        assert!(e
            .snapshot(now + 1_800_000)
            .intraday_windows
            .iter()
            .all(|w| !w.premium_complete));
        assert!(!e.snapshot(now + 1_800_000).expiry_windows["2026-09-24"].premium_complete);
        e.trade("TODAY", 2.0, 1.0, now + 1_800_001, now + 1_800_001);
        e.invalidate_coverage();
        assert!(!e.snapshot(now + 3_600_001).expiry_windows["2026-09-24"].premium_complete);
    }
    #[test]
    fn intraday_windows_do_not_substitute_other_expirations() {
        let now = crate::calendar::day_bounds("2026-09-24").unwrap().0 + 36_000_000;
        let mut e = Engine::default();
        let mut c = contract("NEXT", "call", Some(100.0));
        c.expiration = "2026-09-25".into();
        e.set_contracts([("NEXT".into(), c)].into());
        e.quote("NEXT", 1.0, 2.0, now);
        e.trade("NEXT", 2.0, 100.0, now, now);
        assert!(e.snapshot(now + 60_000).intraday_windows.is_empty());
        let tomorrow = crate::calendar::day_bounds("2026-09-25").unwrap().0 + 36_000_000;
        let s = e.snapshot(tomorrow);
        assert_eq!(s.intraday_windows.len(), 4);
        assert!(s
            .intraday_windows
            .iter()
            .all(|w| w.event_count == 0 && !w.premium_complete));
    }
    #[test]
    fn rollover_restarts_observation_then_recovers_after_a_full_new_window() {
        let now = crate::calendar::day_bounds("2026-09-24").unwrap().0 + 36_000_000;
        let mut e = Engine::default();
        e.set_contracts(BTreeMap::from([(
            "A".into(),
            contract("A", "call", Some(100.0)),
        )]));
        e.trade("A", 1.0, 1.0, now, now);
        assert!(e.snapshot(now + 60_000).contract_windows[0].covered);
        let next = now + 86_400_000;
        assert!(!e.snapshot(next).contract_windows[0].covered);
        e.trade("A", 1.0, 1.0, next, next);
        assert!(!e.snapshot(next + 59_999).contract_windows[0].covered);
        assert!(e.snapshot(next + 60_000).contract_windows[0].covered);
    }
    #[test]
    fn per_contract_coverage_requires_continuity_and_keeps_unknown_distinct_from_zero() {
        let now = crate::calendar::day_bounds("2026-09-24").unwrap().0 + 36_000_000;
        let mut e = Engine::default();
        let meta = contract("A", "call", Some(100.0));
        e.set_contracts(BTreeMap::from([("A".into(), meta.clone())]));
        assert_eq!(e.snapshot(now).contract_windows[0].contracts, None);
        e.trade("A", 2.0, 3.0, now, now);
        assert!(!e.snapshot(now + 59_999).contract_windows[0].covered);
        let s = e.snapshot(now + 60_000);
        assert_eq!(s.contract_windows[0].contracts, Some(3.0));
        assert_eq!(s.contract_windows[0].premium, Some(600.0));
        assert!(!s.contract_windows[1].covered);
        assert_eq!(
            e.snapshot(now + 60_001).contract_windows[0].contracts,
            Some(0.0)
        );
        e.set_contracts(BTreeMap::from([
            ("A".into(), meta.clone()),
            ("B".into(), contract("B", "put", Some(100.0))),
        ]));
        let s = e.snapshot(now + 61_000);
        assert!(s.contract_windows[0].covered);
        assert!(!s.contract_windows[1].covered);
        e.reset_coverage();
        assert_eq!(e.snapshot(now + 62_000).contract_windows[0].contracts, None);
        e.trade("A", 2.0, 1.0, now + 62_000, now + 62_000);
        assert!(e.snapshot(now + 122_000).contract_windows[0].covered);
        let mut changed = meta;
        changed.premium_multiplier = None;
        e.set_contracts(BTreeMap::from([("A".into(), changed)]));
        assert!(!e.snapshot(now + 123_000).contract_windows[0].covered);
        e.trade("A", 2.0, 1.0, now + 123_000, now + 123_000);
        let s = e.snapshot(now + 183_000);
        assert!(s.contract_windows[0].covered);
        assert_eq!(s.contract_windows[0].premium, None);
        e.trade("A", f64::NAN, 1.0, now + 183_000, now + 183_000);
        assert!(!e.snapshot(now + 184_000).contract_windows[0].covered);
    }
    fn contract(ticker: &str, kind: &str, multiplier: Option<f64>) -> Contract {
        Contract {
            ticker: ticker.into(),
            kind: kind.into(),
            strike: 500.0,
            expiration: "2026-09-25".into(),
            shares_per_contract: Some(100.0),
            premium_multiplier: multiplier,
        }
    }
    #[test]
    fn liquidity_retains_missing_members_and_resets_quotes_on_reconnect_and_rotation() {
        let mut e = Engine::default();
        let contracts: BTreeMap<_, _> = ["A", "B", "C"]
            .into_iter()
            .map(|t| (t.into(), contract(t, "call", Some(100.0))))
            .collect();
        e.set_contracts(contracts.clone());
        for t in ["A", "B"] {
            e.quote_with_depth(t, 1.0, 1.1, 100_000, Some(1.0), Some(1.0));
        }
        assert!(!e.liquidity(100_000, true).ready); // Two of three is below 70%.
        e.quote_with_depth("C", 1.0, 1.1, 100_000, Some(1.0), Some(1.0));
        assert!(e.liquidity(112_000, true).ready);
        assert!(!e.liquidity(112_001, true).ready);
        e.quote_with_depth("C", 1.1, 1.0, 100_001, Some(1.0), Some(1.0));
        assert!(!e.liquidity(100_001, true).ready); // Crossed update replaces older valid NBBO.
        e.quote_with_depth("C", 1.0, 1.1, 100_002, Some(1.0), Some(1.0));
        assert!(e.liquidity(100_002, true).ready);
        e.reset_coverage();
        assert_eq!(e.liquidity(100_002, true).quoted, 0);
        e.set_contracts(BTreeMap::from([(
            "D".into(),
            contract("D", "put", Some(100.0)),
        )]));
        assert_eq!(e.liquidity(100_002, true).contracts_tracked, 1);
        assert_eq!(e.liquidity(100_002, true).quoted, 0);
    }
    #[test]
    fn audited_python_fixtures_match_classification_premiums_and_windows() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-flow.json")).unwrap();
        for case in fixtures["cases"].as_array().unwrap() {
            let now = case["nowMs"].as_u64().unwrap();
            let mut engine = Engine::default();
            let mut contracts = BTreeMap::new();
            for trade in case["trades"].as_array().unwrap() {
                let ticker = trade["ticker"].as_str().unwrap();
                contracts.insert(
                    ticker.into(),
                    contract(ticker, trade["kind"].as_str().unwrap(), Some(100.0)),
                );
            }
            engine.set_contracts(contracts);
            for trade in case["trades"].as_array().unwrap() {
                let ticker = trade["ticker"].as_str().unwrap();
                if let Some(bid) = trade["bid"].as_f64() {
                    engine.quote(
                        ticker,
                        bid,
                        trade["ask"].as_f64().unwrap(),
                        trade["quoteMs"].as_u64().unwrap(),
                    );
                }
                let timestamp = trade["timestampMs"].as_u64().unwrap();
                engine.trade(
                    ticker,
                    trade["price"].as_f64().unwrap(),
                    trade["size"].as_f64().unwrap(),
                    timestamp,
                    now,
                );
                if now - timestamp <= RETENTION_MS {
                    let event = engine.events.get(&(timestamp, engine.sequence)).unwrap();
                    assert_eq!(
                        event.side,
                        trade["expected"]["side"].as_str().unwrap(),
                        "{}",
                        case["name"]
                    );
                    assert_eq!(
                        event.confidence,
                        trade["expected"]["confidence"].as_str().unwrap()
                    );
                    assert!(
                        (event.premium.unwrap() - trade["expected"]["premium"].as_f64().unwrap())
                            .abs()
                            < 1e-8
                    );
                }
            }
            for window in engine.snapshot(now).windows {
                let expected = &case["windows"][format!("{}m", window.minutes)];
                assert_eq!(
                    window.event_count as u64,
                    expected["event_count"].as_u64().unwrap(),
                    "{}",
                    case["name"]
                );
                for (actual, key) in [
                    (window.premium, "premium"),
                    (window.contracts, "contracts"),
                    (window.bullish_premium, "bullish_premium"),
                    (window.bearish_premium, "bearish_premium"),
                ] {
                    assert!(
                        (actual - expected[key].as_f64().unwrap()).abs() < 1e-8,
                        "{} {key}",
                        case["name"]
                    );
                }
                // Legacy represents absent directional evidence as 50/50. Preserve
                // numeric parity only where classified premium actually exists.
                if window.bullish_premium + window.bearish_premium > 0.0 {
                    assert!(
                        (window.bullish_pct.unwrap() - expected["bullish_pct"].as_f64().unwrap())
                            .abs()
                            < 1e-8
                    );
                    assert_eq!(window.direction, expected["direction"].as_str().unwrap());
                } else {
                    assert_eq!(window.bullish_pct, None);
                    assert_eq!(window.bearish_pct, None);
                    assert_eq!(
                        window.direction,
                        if window.event_count == 0 {
                            "NO DATA"
                        } else {
                            "UNCLASSIFIED"
                        }
                    );
                }
            }
        }
    }
    #[test]
    fn stale_future_and_crossed_quotes_cannot_infer_trade_side() {
        let mut engine = Engine::default();
        engine.set_contracts(BTreeMap::from([(
            "x".into(),
            contract("x", "call", Some(100.0)),
        )]));
        engine.quote("x", 1.0, 1.2, 1000);
        engine.trade("x", 1.2, 1.0, 3001, 3001);
        engine.quote("x", 1.0, 1.2, 4000);
        engine.trade("x", 1.2, 1.0, 3999, 4000);
        engine.quote("x", 1.3, 1.1, 5000);
        engine.trade("x", 1.2, 1.0, 5000, 5000);
        let snapshot = engine.snapshot(5000);
        assert_eq!(snapshot.windows[0].direction, "UNCLASSIFIED");
        assert_eq!(snapshot.windows[0].bullish_pct, None);
        assert!(snapshot.windows[0].premium_complete);
        assert!(snapshot.recent.iter().all(|e| e.side == "UNKNOWN"));
        assert_eq!(
            snapshot
                .recent
                .iter()
                .map(|e| e.quote_state.as_str())
                .collect::<Vec<_>>(),
            vec!["invalid", "future", "stale"]
        );
    }
    #[test]
    fn unknown_multiplier_and_out_of_order_purge_are_explicit() {
        let mut engine = Engine::default();
        engine.set_contracts(BTreeMap::from([("x".into(), contract("x", "call", None))]));
        engine.quote("x", 1.0, 1.2, 1_800_000);
        engine.trade("x", 1.2, 1.0, 1_800_000, 1_800_000);
        engine.trade("x", 1.2, 1.0, 1000, 1_800_000);
        let s = engine.snapshot(1_801_001);
        assert_eq!(s.recent.len(), 1);
        assert!(!s.windows[3].premium_complete);
        assert_eq!(s.windows[3].bullish_pct, None);
        assert_eq!(s.recent[0].premium, None);
        assert!(engine.snapshot(3_600_001).recent.is_empty());
    }
}
