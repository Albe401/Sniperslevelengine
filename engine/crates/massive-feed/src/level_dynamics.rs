//! Descriptive, uncalibrated zone model. No order permission and no dealer-position inference.
use crate::live::LiveState;
use serde::Serialize;
use snipers_core::{calendar, flow::Window, options::ContractSnapshot};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub symbol: String,
    pub at: u64,
    pub available: bool,
    pub reason: String,
    pub preferred: Option<String>,
    pub levels: Vec<Level>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]fn barriers_receive_same_zone_model_and_mismatched_snapshots_are_rejected(){
        let(mut s,now)=fixture();let h=&mut s.option_snapshot_status.horizons[0];
        h.structure.routes[0].direction="CALL".into();h.structure.routes[0].barrier=Some(500.);
        let r=Tracker::default().update(&s,now);let wall=r.levels.iter().find(|l|l.name=="IMÁN PRINCIPAL").unwrap();let barrier=r.levels.iter().find(|l|l.name=="BARRERA CALL").unwrap();assert_eq!(wall.strength,barrier.strength);assert_eq!(wall.coverage,barrier.coverage);
        s.option_snapshot_status.horizons[0].observed_ms-=1;assert!(!Tracker::default().update(&s,now).available);
    }
    fn fixture() -> (LiveState, u64) {
        let now = calendar::rfc3339_ms("2026-09-28T15:00:00Z").unwrap();
        let mut s = LiveState::default();
        s.symbol = "QQQ".into();
        s.started_ms = now;
        s.stocks.phase = "streaming".into();
        s.options.phase = "streaming".into();
        s.options.last_event_ms = Some(now);
        let mut obs = snipers_core::options::Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: now,
            observed_date: "2026-09-28".into(),
            oi_reference_date: Some("2026-09-25".into()),
            contracts: Default::default(),
        };
        for strike in [490, 500, 510] {
            for (kind, code, delta) in [("call", "C", 0.5), ("put", "P", -0.5)] {
                let ticker = format!("O:QQQ260928{code}{:08}", strike * 1000);
                let mut c=snipers_core::options::normalize(&json!({"details":{"ticker":ticker,"expiration_date":"2026-09-28","strike_price":strike,"contract_type":kind},"open_interest":1000,"day":{"volume":500},"greeks":{"gamma":0.03,"delta":delta},"implied_volatility":0.2}),"QQQ").unwrap();
                c.premium_multiplier = Some(100.);
                obs.contracts.insert(ticker, c);
            }
        }
        s.option_snapshot_status.horizons =
            snipers_core::horizons::calculate(&obs, 495., "FIXTURE").unwrap();
        s.option_snapshot_status.phase = "ready".into();
        s.option_snapshot_status.observed_ms = Some(now);
        let h = &mut s.option_snapshot_status.horizons[0];
        h.structure.call_wall.as_mut().unwrap().strike = 510.;
        h.structure.put_wall.as_mut().unwrap().strike = 490.;
        h.structure.selected.insert("M1".into(), Some(500.));
        s.option_snapshot = Some(obs);
        (s, now)
    }
    #[test]
    fn no_flow_is_missing_not_zero_or_a_destination() {
        let (s, now) = fixture();
        let r = Tracker::default().update(&s, now);
        assert_eq!(r.levels.len(), 3);
        assert!(r.preferred.is_none());
        assert!(r.levels.iter().all(|l| l.premium.is_none()
            && l.call_buy.is_none()
            && l.pull.is_none()
            && l.change.is_none()));
        assert!(r
            .levels
            .iter()
            .all(|l| l.oi_change.is_none() && l.oi.is_some() && l.strength.is_some()));
    }
    #[test]
    fn persistence_uses_time_not_render_count_and_resets_on_identity_or_gap() {
        let (mut s, now) = fixture();
        let mut tracker = Tracker::default();
        tracker.update(&s, now);
        for _ in 0..200 {
            assert!(tracker
                .update(&s, now)
                .levels
                .iter()
                .all(|l| l.change.is_none()));
        }
        for t in 1..=60 {
            tracker.update(&s, now + t * 1000);
        }
        let r = tracker.update(&s, now + 60_000);
        assert!(r
            .levels
            .iter()
            .all(|l| l.change.is_some() && l.trend == "→ ESTABLE"));
        s.option_snapshot
            .as_mut()
            .unwrap()
            .contracts
            .values_mut()
            .next()
            .unwrap()
            .gamma = None;
        assert!(tracker
            .update(&s, now + 61_000)
            .levels
            .iter()
            .all(|l| l.change.is_none()));
        assert!(tracker
            .update(&s, now + 100_000)
            .levels
            .iter()
            .all(|l| l.change.is_none()));
    }
    #[test]
    fn stale_foreign_closed_and_disconnected_cannot_publish_live_levels() {
        let (mut s, now) = fixture();
        let mut tracker = Tracker::default();
        assert!(tracker.update(&s, now).available);
        s.option_snapshot.as_mut().unwrap().symbol = "SPY".into();
        assert!(!tracker.update(&s, now).available);
        s.option_snapshot.as_mut().unwrap().symbol = "QQQ".into();
        s.options.phase = "connecting".into();
        assert!(!tracker.update(&s, now).available);
        s.options.phase = "streaming".into();
        assert!(!tracker.update(&s, now + 900_001).available);
        assert!(
            !tracker
                .update(&s, calendar::rfc3339_ms("2026-09-28T21:00:00Z").unwrap())
                .available
        );
    }
    #[test]
    fn gamma_missing_is_not_zero_and_scores_require_coverage() {
        let (mut s, now) = fixture();
        for c in s.option_snapshot.as_mut().unwrap().contracts.values_mut() {
            c.gamma = None;
            c.open_interest = None;
            c.volume = None;
        }
        let r = Tracker::default().update(&s, now);
        assert!(r.levels.iter().all(|l| l.strength.is_none()
            && l.coverage == 10.
            && l.category == "PENDIENTE"
            && l.pull.is_none()));
    }
    #[test]
    fn ambiguous_or_mixed_day_universes_never_publish_strength() {
        for case in 0..4 {
            let (mut s, now) = fixture();
            match case {
                0 => s.option_snapshot_status.observed_ms = Some(now - 1),
                1 => s.option_snapshot_status.horizons.push(s.option_snapshot_status.horizons[0].clone()),
                2 => s.option_snapshot_status.horizons[0].expirations = vec!["2026-09-29".into()],
                _ => s.option_snapshot_status.horizons[0].expirations.push("2026-09-29".into()),
            }
            let report = Tracker::default().update(&s, now);
            assert!(!report.available && report.levels.is_empty());
        }
    }
    #[test]
    fn pull_requires_real_classified_zone_flow_and_current_price() {
        let (mut s, start) = fixture();
        let now = start + 60_000;
        let cs: Vec<_> = s
            .option_snapshot
            .as_ref()
            .unwrap()
            .contracts
            .values()
            .cloned()
            .collect();
        s.flow_engine.set_contracts(
            cs.iter()
                .map(|c| {
                    (
                        c.ticker.clone(),
                        snipers_core::flow::Contract {
                            ticker: c.ticker.clone(),
                            kind: c.kind.clone(),
                            strike: c.strike,
                            expiration: c.expiration.clone(),
                            shares_per_contract: Some(100.),
                            premium_multiplier: Some(100.),
                        },
                    )
                })
                .collect(),
        );
        for c in &cs {
            s.flow_engine.trade(&c.ticker, 2., 1., start, start);
            for i in 1..=6 {
                s.flow_engine.quote(&c.ticker, 1., 2., now - 10 + i);
                s.flow_engine
                    .trade(&c.ticker, 2., 1., now - 10 + i, now - 10 + i);
            }
        }
        s.options.last_event_ms = Some(now);
        s.ticks.insert(
            "QQQ".into(),
            crate::live::Tick {
                symbol: "QQQ".into(),
                price: Some(495.),
                trade_ms: Some(now),
                bid: None,
                ask: None,
                quote_ms: None,
                trade_sequence: None,
            },
        );
        for (i, close) in [(2, 494.), (1, 495.)] {
            let t = now - i * 60_000;
            s.minutes.insert(
                t,
                crate::live::Bar {
                    timestamp_ms: t,
                    open: 493.,
                    high: 496.,
                    low: 492.,
                    close,
                    volume: 1000.,
                    vwap: None,
                },
            );
        }
        for r in &mut s.option_snapshot_status.horizons[0].structure.routes {
            r.path_openness = Some(80.);
        }
        let mut tr = Tracker::default();
        let r = tr.update(&s, now);
        assert!(r
            .levels
            .iter()
            .all(|l| l.flow_complete && l.pull.is_some_and(|v| (0.0..=100.).contains(&v))));
        s.ticks.get_mut("QQQ").unwrap().price = Some(500.);
        assert!(tr
            .update(&s, now)
            .levels
            .iter()
            .find(|l| l.name == "IMÁN PRINCIPAL")
            .unwrap()
            .pull
            .is_none());
        s.ticks.get_mut("QQQ").unwrap().trade_ms = Some(now - 15_001);
        assert!(tr.update(&s, now).levels.iter().all(|l| l.pull.is_none()));
        s.flow_engine.reset_coverage();
        assert!(tr
            .update(&s, now)
            .levels
            .iter()
            .all(|l| !l.flow_complete && l.pull.is_none()));
    }
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    pub name: String,
    pub strike: f64,
    pub low: f64,
    pub high: f64,
    pub provisional: bool,
    pub premium: Option<f64>,
    pub call_buy: Option<f64>,
    pub call_sell: Option<f64>,
    pub put_buy: Option<f64>,
    pub put_sell: Option<f64>,
    pub unknown: Option<f64>,
    pub flow_complete: bool,
    pub classified_pct: Option<f64>,
    pub oi: Option<f64>,
    pub oi_change: Option<f64>,
    pub oi_reference: Option<String>,
    pub daily_volume: Option<f64>,
    pub strength: Option<f64>,
    pub category: String,
    pub coverage: f64,
    pub change: Option<f64>,
    pub trend: String,
    pub pull: Option<f64>,
    pub pull_reason: String,
    pub components: Vec<(String, f64, f64)>,
}
#[derive(Clone, Default)]
pub struct Tracker {
    session: String,
    last: u64,
    memory: BTreeMap<String, Memory>,
}
#[derive(Clone)]
struct Memory {
    key: String,
    mask: String,
    since: u64,
    samples: VecDeque<(u64, f64)>,
}
fn fresh(t: u64, now: u64, max: u64) -> bool {
    t > 0 && t <= now && now - t <= max
}
fn sum<'a>(
    mut rows: impl Iterator<Item = &'a ContractSnapshot>,
    field: impl Fn(&ContractSnapshot) -> Option<f64>,
) -> Option<f64> {
    let mut count = 0;
    let total = rows.try_fold(0.0, |s, c| {
        count += 1;
        field(c)
            .filter(|n| n.is_finite() && *n >= 0.)
            .map(|n| s + n)
    });
    total.filter(|n| count > 0 && n.is_finite())
}
fn concentration(local: Option<f64>, total: Option<f64>) -> Option<f64> {
    local
        .zip(total)
        .filter(|(_, t)| *t > 0.)
        .map(|(v, t)| (v / t / 0.15 * 100.).clamp(0., 100.))
}
fn classified(w: &Window) -> Option<(f64, f64)> {
    let bull = w.buckets.get("CALL BUY")?.premium + w.buckets.get("PUT SELL")?.premium;
    let bear = w.buckets.get("PUT BUY")?.premium + w.buckets.get("CALL SELL")?.premium;
    let known = bull + bear;
    (w.premium.is_finite() && w.premium > 0. && known.is_finite() && known > 0.).then_some((
        (known / w.premium * 100.).clamp(0., 100.),
        (bull - bear) / known,
    ))
}
impl Tracker {
    pub fn update(&mut self, s: &LiveState, now: u64) -> Report {
        let mut out = Report {
            symbol: s.symbol.clone(),
            at: now,
            reason: "Esperando estructura DAY vigente".into(),
            ..Default::default()
        };
        let date = calendar::date_at(now);
        let in_session = date.is_some_and(|d| matches!(calendar::schedule(d), calendar::Day::Open(x) if now>=x.open_ms && now<x.close_ms));
        let session = format!("{}:{:?}:{}", s.symbol, date, s.started_ms);
        let connected = s.stocks.phase == "streaming" && s.options.phase == "streaming";
        if self.session != session
            || now < self.last
            || now.saturating_sub(self.last) > 15_000
            || !connected
            || !in_session
        {
            self.memory.clear();
            self.session = session;
        }
        self.last = now;
        if !connected || !in_session {
            out.reason = "Niveles vivos pendientes de sesión y conexión".into();
            return out;
        }
        let Some(obs) = s.option_snapshot.as_ref().filter(|o| {
            o.symbol == s.symbol
                && fresh(o.observed_ms, now, 900_000)
                && Some(o.observed_date.as_str()) == date.map(|d| d.to_string()).as_deref()
        }) else {
            self.memory.clear();
            return out;
        };
        let Some(h) = s
            .option_snapshot_status
            .horizons
            .iter()
            .find(|h| h.mode == "DAY" && h.observed_ms==obs.observed_ms && fresh(h.observed_ms, now, 900_000))
        else {
            self.memory.clear();
            return out;
        };
        if s.option_snapshot_status.observed_ms != Some(obs.observed_ms)
            || s.option_snapshot_status.horizons.iter().filter(|h| h.mode == "DAY").count() != 1
            || h.expirations.len() != 1
            || h.expirations[0] != obs.observed_date
        {
            self.memory.clear();
            return out;
        }
        if s.option_snapshot_status.phase != "ready" {
            self.memory.clear();
            return out;
        }
        let st = &h.structure;
        let (put, magnet, call, routes) = if st.complete {
            (
                st.put_wall.as_ref().map(|w| w.strike),
                st.selected.get("M1").copied().flatten(),
                st.call_wall.as_ref().map(|w| w.strike),
                &st.routes,
            )
        } else if let Some(p) = &st.provisional {
            (
                p.put_wall.as_ref().map(|w| w.strike),
                p.selected.get("M1").copied().flatten(),
                p.call_wall.as_ref().map(|w| w.strike),
                &p.routes,
            )
        } else {
            self.memory.clear();
            return out;
        };
        let contracts: Vec<_> = obs
            .contracts
            .values()
            .filter(|c| h.expirations.contains(&c.expiration))
            .collect();
        let mut strikes: Vec<_> = contracts
            .iter()
            .map(|c| c.strike)
            .filter(|p| p.is_finite() && *p > 0.)
            .collect();
        strikes.sort_by(f64::total_cmp);
        strikes.dedup();
        let gamma = |c: &ContractSnapshot| {
            c.gamma
                .zip(c.open_interest)
                .zip(c.premium_multiplier)
                .filter(|((g, o), m)| *g >= 0. && *o >= 0. && *m > 0.)
                .map(|((g, o), m)| g * o * m)
        };
        let total_oi = sum(contracts.iter().copied(), |c| c.open_interest);
        let total_vol = sum(contracts.iter().copied(), |c| c.volume);
        let total_gex = sum(contracts.iter().copied(), gamma);
        let spot = s
            .ticks
            .get(&s.symbol)
            .filter(|t| t.symbol == s.symbol && t.trade_ms.is_some_and(|t| fresh(t, now, 15_000)))
            .and_then(|t| t.price)
            .filter(|p| p.is_finite() && *p > 0.);
        let flow_fresh = s
            .options
            .last_event_ms
            .is_some_and(|t| fresh(t, now, 15_000));
        let all_ids = contracts.iter().map(|c| c.ticker.clone()).collect();
        let all_flow = s.flow_engine.zone_activity(&all_ids, now);
        let bars: Vec<_> = s
            .minutes
            .values()
            .filter(|b| {
                b.timestamp_ms + 60_000 <= now
                    && fresh(b.timestamp_ms, now, 240_000)
                    && calendar::date_at(b.timestamp_ms) == date
            })
            .collect();
        let price_direction = bars
            .last()
            .zip(bars.iter().rev().nth(1))
            .filter(|(b, a)| b.timestamp_ms == a.timestamp_ms + 60_000)
            .and_then(|(b, a)| {
                (a.close > 0. && b.close.is_finite())
                    .then_some(((b.close / a.close - 1.) / 0.001).clamp(-1., 1.))
            });
        let identity: Vec<_> = contracts
            .iter()
            .map(|c| {
                (
                    &c.ticker,
                    c.open_interest.is_some(),
                    c.gamma.is_some(),
                    c.volume.is_some(),
                    c.premium_multiplier,
                )
            })
            .collect();
        let mut references=vec![("PUT WALL",put),("IMÁN PRINCIPAL",magnet),("CALL WALL",call)];
        for route in routes {let name=match route.direction.as_str(){"CALL"=>"BARRERA CALL","PUT"=>"BARRERA PUT",_=>continue};if !references.iter().any(|(n,_)|*n==name){references.push((name,route.barrier));}}
        for (name, strike) in references {
            let Some(strike) = strike.filter(|n| n.is_finite() && *n > 0.) else {
                continue;
            };
            let Some(i) = strikes.iter().position(|p| *p == strike) else {
                continue;
            };
            let low = strikes[i.saturating_sub(1)];
            let high = strikes[(i + 1).min(strikes.len() - 1)];
            let local: Vec<_> = contracts
                .iter()
                .copied()
                .filter(|c| c.strike >= low && c.strike <= high)
                .collect();
            let ids: BTreeSet<_> = local.iter().map(|c| c.ticker.clone()).collect();
            let w = s.flow_engine.zone_activity(&ids, now);
            let covered = flow_fresh && w.premium_complete;
            let class = flow_fresh.then(|| classified(&w)).flatten();
            let observed = flow_fresh && (w.event_count > 0 || covered);
            let bucket = |label: &str| {
                observed
                    .then(|| w.buckets.get(label).map(|b| b.premium))
                    .flatten()
                    .filter(|v| v.is_finite())
            };
            let oi = sum(local.iter().copied(), |c| c.open_interest);
            let vol = sum(local.iter().copied(), |c| c.volume);
            let gex = sum(local.iter().copied(), gamma);
            let oi_change: Option<f64> = local.iter().try_fold(0., |n, c| {
                s.option_snapshot_status
                    .changes
                    .iter()
                    .find(|x| {
                        x.ticker == c.ticker
                            && x.previous_observed_date
                                .as_ref()
                                .is_some_and(|d| d < &obs.observed_date)
                    })
                    .and_then(|x| x.change)
                    .filter(|d| d.is_finite())
                    .map(|d| n + d)
            });
            let key = format!(
                "{strike}:{low}:{high}:{}:{covered}:{:?}:{:?}",
                st.complete, identity, obs.oi_reference_date
            );
            let memory = self.memory.entry(name.into()).or_insert_with(|| Memory {
                key: key.clone(),
                mask: String::new(),
                since: now,
                samples: VecDeque::new(),
            });
            if memory.key != key {
                *memory = Memory {
                    key,
                    mask: String::new(),
                    since: now,
                    samples: VecDeque::new(),
                };
            }
            let persistence =
                (now.saturating_sub(memory.since) as f64 / 300_000. * 100.).clamp(0., 100.);
            let parts = [
                ("Concentración OI", concentration(oi, total_oi), 25.),
                ("Concentración gamma", concentration(gex, total_gex), 25.),
                ("Volumen del día", concentration(vol, total_vol), 15.),
                (
                    "Prima observada 1m",
                    if covered && all_flow.premium_complete {
                        concentration(Some(w.premium), Some(all_flow.premium))
                    } else {
                        None
                    },
                    20.,
                ),
                (
                    "Cambio OI diario",
                    oi_change
                        .zip(oi)
                        .filter(|(_, o)| *o > 0.)
                        .map(|(d, o)| (50. + 50. * d / o).clamp(0., 100.)),
                    5.,
                ),
                ("Persistencia de referencia", Some(persistence), 10.),
            ];
            let components: Vec<_> = parts
                .into_iter()
                .filter_map(|(n, v, w)| v.filter(|v| v.is_finite()).map(|v| (n.to_owned(), v, w)))
                .collect();
            let coverage: f64 = components.iter().map(|p| p.2).sum();
            let strength = (coverage >= 60.)
                .then(|| components.iter().map(|p| p.1 * p.2).sum::<f64>() / coverage);
            // A coverage/component change establishes a new baseline, never a strength trend.
            let mask = components
                .iter()
                .map(|p| p.0.as_str())
                .collect::<Vec<_>>()
                .join("|");
            if memory.mask != mask {
                memory.samples.clear();
                memory.mask = mask;
            }
            let change = strength.and_then(|v| {
                memory
                    .samples
                    .iter()
                    .rev()
                    .find(|(t, _)| now.saturating_sub(*t) >= 60_000)
                    .map(|(_, old)| v - old)
            });
            if let Some(v) = strength {
                if memory
                    .samples
                    .back()
                    .is_none_or(|(t, _)| now.saturating_sub(*t) >= 1_000)
                {
                    memory.samples.push_back((now, v));
                }
            }
            while memory
                .samples
                .front()
                .is_some_and(|(t, _)| now.saturating_sub(*t) > 120_000)
            {
                memory.samples.pop_front();
            }
            let trend = match change {
                Some(d) if d >= 5. => "↑ GANANDO",
                Some(d) if d <= -5. => "↓ PERDIENDO",
                Some(_) => "→ ESTABLE",
                None => "ESTABLECIENDO BASE",
            };
            let mut pull = None;
            let mut pull_reason =
                "Faltan precio, flujo clasificado o camino verificable".to_string();
            if let (Some(p), Some(strength), Some((classified, bias)), Some(direction)) =
                (spot, strength, class, price_direction)
            {
                let d = if (strike - p).abs() < 0.000_001 {
                    0.
                } else {
                    (strike - p).signum()
                };
                if d == 0. {
                    pull_reason = "Precio en el nivel; no es un destino adelante".into();
                } else if covered && classified >= 50. && w.event_count >= 5 {
                    let route = routes
                        .iter()
                        .find(|r| r.direction == if d > 0. { "CALL" } else { "PUT" });
                    if let Some(route) = route {
                        if let Some(open) = route.path_openness.filter(|n| n.is_finite()) {
                            let barrier = route
                                .barrier
                                .zip(route.barrier_score)
                                .filter(|(b, _)| (*b - p) * d > 0. && (*b - strike) * d < 0.)
                                .map(|(_, score)| score.clamp(0., 100.) / 100.)
                                .unwrap_or(0.);
                            let distance =
                                (100. - ((strike - p).abs() / p / 0.02) * 100.).clamp(0., 100.);
                            let rvol = s.metrics.rvol.filter(|r| {
                                r.is_finite()
                                    && *r >= 0.
                                    && s.metrics.asof_ms.is_some_and(|t| fresh(t, now, 180_000))
                            });
                            let participation = rvol.map(|r| r.min(2.) / 2.).unwrap_or(0.5);
                            pull = Some(
                                (strength * 0.35
                                    + distance * 0.15
                                    + (50. + 50. * bias * d) * 0.25
                                    + (50. + 50. * direction * d) * 0.15
                                    + open.clamp(0., 100.) * 0.10
                                    - barrier * 20.)
                                    * (0.8 + 0.2 * participation),
                            );
                            pull = pull.map(|v| v.clamp(0., 100.));
                            pull_reason = format!(
                                "Índice heurístico · {} · {}",
                                if barrier > 0. {
                                    "barrera intermedia"
                                } else {
                                    "sin barrera mapeada intermedia"
                                },
                                if rvol.is_some() {
                                    "RVOL disponible"
                                } else {
                                    "RVOL no disponible"
                                }
                            );
                        }
                    }
                }
            }
            out.levels.push(Level {
                name: name.into(),
                strike,
                low,
                high,
                provisional: !st.complete,
                premium: observed.then_some(w.premium).filter(|p| p.is_finite()),
                call_buy: bucket("CALL BUY"),
                call_sell: bucket("CALL SELL"),
                put_buy: bucket("PUT BUY"),
                put_sell: bucket("PUT SELL"),
                unknown: bucket("UNKNOWN"),
                flow_complete: covered,
                classified_pct: class.map(|c| c.0),
                oi,
                oi_change,
                oi_reference: obs.oi_reference_date.clone(),
                daily_volume: vol,
                strength,
                category: strength
                    .map(|v| {
                        if v >= 80. {
                            "EXTREME"
                        } else if v >= 60. {
                            "HIGH"
                        } else if v >= 40. {
                            "MEDIUM"
                        } else {
                            "LOW"
                        }
                    })
                    .unwrap_or("PENDIENTE")
                    .into(),
                coverage,
                change,
                trend: trend.into(),
                pull,
                pull_reason,
                components,
            });
        }
        let mut ranked: Vec<_> = out
            .levels
            .iter()
            .filter(|l| !l.provisional && l.pull.is_some() && matches!(l.name.as_str(),"PUT WALL"|"IMÁN PRINCIPAL"|"CALL WALL"))
            .collect();
        ranked.sort_by(|a, b| b.pull.unwrap().total_cmp(&a.pull.unwrap()));
        // Rank only a complete comparison; ties and missing candidates are not winners.
        if ranked.len() == 3 && ranked[0].pull.unwrap() - ranked[1].pull.unwrap() >= 5. {
            out.preferred = Some(ranked[0].name.clone());
        }
        out.available = !out.levels.is_empty();
        out.reason="DAY · zonas ±1 strike listado · prima negociada 1m, no capital neto · zonas pueden solaparse".into();
        out
    }
}
