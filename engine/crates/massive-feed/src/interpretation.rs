//! Bounded context computed from already validated state; no network or order side effects.
use crate::live::LiveState;
use snipers_core::magnet_confirmation::{self, Confirmation, Evidence, OffMagnet, OptionsMagnet};
use std::collections::BTreeMap;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallFlow {
    pub available: bool,
    pub specs: Vec<snipers_core::wall_flow::Spec>,
    pub windows: Vec<snipers_core::wall_flow::Window>,
}
pub fn adjusted_readings(
    state: &LiveState,
    now: u64,
) -> BTreeMap<String, snipers_core::reading_overlay::Adjusted> {
    use snipers_core::reading_overlay::{self, Trf};
    let confirmations = confirmations(state, now);
    let maps = position_maps(state, now);
    let day = snipers_core::calendar::date_at(now).map(|d| d.to_string());
    let flow = day.as_ref().and_then(|d| state.flow.expiry_windows.get(d));
    let flow_bias = flow
        .filter(|f| {
            f.event_count > 0
                && f.premium_complete
                && state.options.phase == "streaming"
                && state
                    .options
                    .last_event_ms
                    .is_some_and(|t| t <= now && now - t <= 15_000)
        })
        .and_then(|f| f.bullish_pct.zip(f.bearish_pct))
        .map(|(b, s)| (b - s) / 100.0);
    let session = &state.off_exchange.analytics.session;
    state
        .option_snapshot_status
        .horizons
        .iter()
        .take(3)
        .map(|h| {
            let mut base = h.structure.reading.clone();
            if !structure_current(state, h, now) {
                base.available = false;
            }
            let mut r = reading_overlay::with_flow(&base, &h.mode, flow_bias);
            if let Some(c) = confirmations.get(&h.mode) {
                let status = match c.status.as_str() {
                    "CONFIRMED_STRONG" | "CONFIRMED" => "CONFIRMADO",
                    "PARTIAL_CONFIRMATION" => "PARCIAL",
                    "DIVERGENCIA" => "DIVERGENCIA",
                    _ => "SIN CONFIRMACION",
                };
                let verified = c.off_magnet.is_some()
                    && matches!(
                        c.status.as_str(),
                        "CONFIRMED_STRONG"
                            | "CONFIRMED"
                            | "PARTIAL_CONFIRMATION"
                            | "WEAK_CONFIRMATION"
                            | "DIVERGENCIA"
                    );
                let spot = maps.get(&format!("{} CALL", h.mode)).and_then(|m| m.price);
                let confluences = c.off_magnet.zip(spot).map_or(0, |(magnet, spot)| {
                    [
                        h.structure.selected.get("M1").copied().flatten(),
                        h.exposure.zero_gamma,
                        h.structure.call_wall.as_ref().map(|w| w.strike),
                        h.structure.put_wall.as_ref().map(|w| w.strike),
                        h.structure.magnet,
                    ]
                    .into_iter()
                    .flatten()
                    .filter(|p| (magnet - p).abs() / spot <= 0.003)
                    .count()
                });
                let context = Trf {
                    confirmation: status.into(),
                    alignment: c.alignment.clone(),
                    strength: match c.off_score {
                        Some(v) if v >= 80.0 => "MUY FUERTE",
                        Some(v) if v >= 65.0 => "FUERTE",
                        Some(v) if v >= 45.0 => "MEDIA",
                        _ => "DÉBIL",
                    }
                    .into(),
                    pressure_side: match session.pressure_pct {
                        Some(p) if p >= 15.0 => "CALL",
                        Some(p) if p <= -15.0 => "PUT",
                        _ => "",
                    }
                    .into(),
                    pressure_coverage: session.pressure_coverage_pct.unwrap_or(0.0),
                    confluences,
                };
                reading_overlay::apply_trf(&mut r, &h.mode, &context, verified);
            }
            (h.mode.clone(), r)
        })
        .collect()
}
pub fn wall_pulls(state: &LiveState, now: u64) -> BTreeMap<String, snipers_core::wall_pull::Read> {
    use snipers_core::{liquidity, wall_pull};
    let maps = position_maps(state, now);
    let flows = wall_flow(state, now);
    let mut out = BTreeMap::new();
    for h in state.option_snapshot_status.horizons.iter().take(3) {
        for side in ["CALL", "PUT"] {
            let key = format!("{} {side}", h.mode);
            let support = if side == "CALL" {
                h.structure.put_wall.as_ref()
            } else {
                h.structure.call_wall.as_ref()
            };
            let target = if side == "CALL" {
                h.structure.call_wall.as_ref()
            } else {
                h.structure.put_wall.as_ref()
            };
            let route = h.structure.routes.iter().find(|r| r.direction == side);
            let live = flows.get(&h.mode);
            let window = |minutes| {
                live.and_then(|f| f.windows.iter().find(|w| w.minutes == minutes))
                    .map(|w| wall_pull::summarize(w, side))
                    .unwrap_or_default()
            };
            let day = snipers_core::calendar::date_at(now).map(|d| d.to_string());
            let flow = day.as_ref().and_then(|d| state.flow.expiry_windows.get(d));
            let input = wall_pull::Input {
                side: side.into(),
                horizon: h.mode.clone(),
                support_score: support.map(|w| w.score),
                support_strength: support.map(|w| w.strength.clone()).unwrap_or_default(),
                pull_score: route.and_then(|r| r.score),
                pull_strength: route
                    .and_then(|r| r.score)
                    .map(|s| liquidity::strength(s).into())
                    .unwrap_or_default(),
                pull_ready: route.is_some_and(|r| r.structural_ready),
                support_context: support.is_some_and(|w| w.zone_available),
                target_context: target.is_some_and(|w| w.zone_available),
                position_ready: structure_current(state, h, now)
                    && maps.get(&key).is_some_and(|m| m.ready),
                flow_available: live.is_some_and(|f| f.available)
                    && flow.is_some_and(|f| f.event_count > 0 && f.premium_complete),
                flow_direction: flow.map(|f| f.direction.clone()).unwrap_or_default(),
                bullish_pct: flow.and_then(|f| f.bullish_pct),
                bearish_pct: flow.and_then(|f| f.bearish_pct),
                live_1m: window(1),
                live_5m: window(5),
            };
            out.insert(key, wall_pull::calculate(&input));
        }
    }
    out
}
pub(crate) fn structure_current(
    state: &LiveState,
    h: &snipers_core::horizons::Horizon,
    now: u64,
) -> bool {
    use snipers_core::calendar::date_at;
    state.option_snapshot_status.phase == "ready"
        && h.structure.complete
        && h.structure.liquidity_complete
        && h.observed_ms <= now
        && now - h.observed_ms <= 900_000
        && date_at(now).is_some()
        && date_at(now) == date_at(h.observed_ms)
        && !h.expirations.is_empty()
        && h.expirations
            .iter()
            .all(|e| snipers_core::calendar::day_bounds(e).is_some_and(|(_, end)| end > now))
        && state
            .option_snapshot
            .as_ref()
            .is_some_and(|o| o.symbol == state.symbol && o.observed_ms == h.observed_ms)
}
pub fn wall_flow(state: &LiveState, now: u64) -> BTreeMap<String, WallFlow> {
    use snipers_core::wall_flow;
    state
        .option_snapshot_status
        .horizons
        .iter()
        .take(3)
        .map(|h| {
            let available = state.options.phase == "streaming"
                && state
                    .options
                    .last_event_ms
                    .is_some_and(|t| t <= now && now - t <= 15_000)
                && structure_current(state, h, now);
            let specs = wall_flow::structural_specs(&h.structure);
            let windows = wall_flow::windows(
                &specs,
                &h.expirations,
                &state.flow.contract_windows,
                available,
            );
            (
                h.mode.clone(),
                WallFlow {
                    available,
                    specs,
                    windows,
                },
            )
        })
        .collect()
}

pub fn position_maps(
    state: &LiveState,
    now: u64,
) -> BTreeMap<String, snipers_core::position_map::Map> {
    use snipers_core::position_map::{self, Input};
    let tick = state.ticks.get(&state.symbol);
    let trade = tick.and_then(|t| t.price.zip(t.trade_ms));
    let quote = tick
        .and_then(|t| t.bid.zip(t.ask).zip(t.quote_ms))
        .filter(|((b, a), _)| b.is_finite() && a.is_finite() && *b > 0.0 && a >= b)
        .map(|((b, a), t)| (b / 2.0 + a / 2.0, t));
    let spot = trade
        .into_iter()
        .chain(quote)
        .filter(|(p, _)| p.is_finite() && *p > 0.0)
        .max_by_key(|(_, t)| *t);
    let mut out = BTreeMap::new();
    for h in state.option_snapshot_status.horizons.iter().take(3) {
        let complete = state.stocks.phase == "streaming"
            && state.option_snapshot_status.phase == "ready"
            && h.structure.complete
            && h.structure.liquidity_complete
            && state
                .option_snapshot
                .as_ref()
                .is_some_and(|o| o.symbol == state.symbol && o.observed_ms == h.observed_ms);
        for side in ["CALL", "PUT"] {
            let route = h.structure.routes.iter().find(|r| r.direction == side);
            let i = Input {
                side: side.into(),
                price: spot.map(|(p, _)| p),
                put_wall: h.structure.put_wall.as_ref().map(|w| w.strike),
                call_wall: h.structure.call_wall.as_ref().map(|w| w.strike),
                pull_target: route.and_then(|r| r.pull_target),
                main_magnet: h.structure.selected.get("M1").copied().flatten(),
                liquidity_magnet: h.structure.magnet,
                vacuum_low: route.and_then(|r| r.vacuum_low),
                vacuum_high: route.and_then(|r| r.vacuum_high),
            };
            out.insert(
                format!("{} {side}", h.mode),
                position_map::guarded(&i, complete, h.observed_ms, spot.map(|(_, t)| t), now),
            );
        }
    }
    out
}

pub fn confirmations(state: &LiveState, now: u64) -> BTreeMap<String, Confirmation> {
    let off = &state.off_exchange.analytics;
    let session = &off.session;
    let stock = state.ticks.get(&state.symbol);
    let trade = stock.and_then(|t| t.price.zip(t.trade_ms));
    let quote = stock
        .and_then(|t| t.bid.zip(t.ask).zip(t.quote_ms))
        .filter(|((bid, ask), _)| bid.is_finite() && ask.is_finite() && *bid > 0.0 && ask >= bid)
        .map(|((bid, ask), time)| (bid / 2.0 + ask / 2.0, time));
    let spot = trade
        .into_iter()
        .chain(quote)
        .filter(|(p, _)| p.is_finite() && *p > 0.0)
        .max_by_key(|(_, time)| *time);
    let day = snipers_core::calendar::date_at(now).map(|d| d.to_string());
    let history_ready = state
        .off_exchange
        .history
        .get(&session.date)
        .is_some_and(|h| {
            h.phase == "ready"
                && state
                    .off_exchange
                    .recovery_since_ms
                    .is_none_or(|needed| h.through_ms.is_some_and(|through| through >= needed))
        });
    state
        .option_snapshot_status
        .horizons
        .iter()
        .take(3)
        .map(|h| {
            // The active legacy UI prefers M1; a liquidity target must not replace it.
            // Use the score of this exact M1, from the same observed structure.
            let options = h
                .structure
                .selected
                .get("M1")
                .copied()
                .flatten()
                .map(|price| OptionsMagnet { price, score: h.structure.main_magnet_score });
            let magnet = session.top_levels.first().map(|level| OffMagnet {
                price: level.level,
                notional: level.notional,
                prints: level.prints,
                total_notional: session.total_notional,
                total_prints: session.print_count,
                concentration_pct: level.concentration_pct,
            });
            let result = magnet_confirmation::guarded(
                options,
                magnet,
                spot.map(|(p, _)| p),
                Evidence {
                    symbol: &state.symbol,
                    off_symbol: &off.symbol,
                    options_ms: Some(h.observed_ms),
                    off_ms: off.last_live_ms,
                    spot_ms: spot.map(|(_, t)| t),
                    options_complete: state.option_snapshot_status.phase == "ready"
                        && h.structure.complete
                        && state.option_snapshot.as_ref().is_some_and(|o| {
                            o.symbol == state.symbol && o.observed_ms == h.observed_ms
                        }),
                    off_complete: session.complete
                        && session.history_complete
                        && history_ready
                        && day.as_deref() == Some(&session.date)
                        && state.stocks.phase == "streaming"
                        && state.off_exchange.queue_drops == 0
                        && state.off_exchange.error.is_none(),
                },
                now,
            );
            (h.mode.clone(), result)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use snipers_core::{calendar, off_exchange::Level, options::Observation};
    #[test]
    fn integration_preserves_m1_and_blocks_unrecovered_or_stale_state() {
        let now = calendar::day_bounds("2026-09-24").unwrap().0 + 10 * 3_600_000;
        let mut observation = Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: now,
            observed_date: "2026-09-24".into(),
            oi_reference_date: None,
            contracts: BTreeMap::new(),
        };
        for (kind, cp, delta) in [("call", "C", 0.5), ("put", "P", -0.5)] {
            let mut c=snipers_core::options::normalize(&serde_json::json!({"details":{"ticker":format!("O:QQQ260924{cp}00500000"),"expiration_date":"2026-09-24","contract_type":kind,"strike_price":500},"open_interest":10,"day":{"volume":20},"greeks":{"delta":delta,"gamma":0.01},"implied_volatility":0.2}),"QQQ").unwrap();
            c.premium_multiplier = Some(100.0);
            observation.contracts.insert(c.ticker.clone(), c);
        }
        let shared = crate::live::new_state();
        let mut state = shared.lock().unwrap();
        state.option_snapshot_status.horizons =
            snipers_core::horizons::calculate(&observation, 500.0, "stock_ws").unwrap();
        state.option_snapshot_status.phase = "ready".into();
        state.option_snapshot = Some(observation);
        for h in &mut state.option_snapshot_status.horizons {
            h.structure.selected.insert("M1".into(), Some(500.0));
            h.structure.magnet = Some(510.0);
        }
        state.stocks.phase = "streaming".into();
        state.ticks.insert(
            "QQQ".into(),
            crate::live::Tick {
                symbol: "QQQ".into(),
                price: Some(500.0),
                bid: None,
                ask: None,
                trade_ms: Some(now),
                trade_sequence: None,
                quote_ms: None,
            },
        );
        state.off_exchange.analytics.symbol = "QQQ".into();
        state.off_exchange.analytics.last_live_ms = Some(now);
        let session = &mut state.off_exchange.analytics.session;
        session.date = "2026-09-24".into();
        session.complete = true;
        session.history_complete = true;
        session.print_count = 1000;
        session.total_notional = 1_000_000.0;
        session.top_levels = vec![Level {
            level: 500.0,
            notional: 200_000.0,
            prints: 20,
            concentration_pct: 20.0,
            ..Default::default()
        }];
        let date = session.date.clone();
        state.off_exchange.history.insert(
            date,
            crate::off_exchange::HistoryStatus {
                phase: "ready".into(),
                through_ms: Some(now),
                ..Default::default()
            },
        );
        let results = confirmations(&state, now);
        assert_eq!(results["DAY"].status, "CONFIRMED_STRONG");
        assert_eq!(results["DAY"].primary_magnet, Some(500.0));
        assert!(!results["DAY"].entry_confirmed);
        let original_score=state.option_snapshot_status.horizons[0].structure.main_magnet_score;
        assert_eq!(results["DAY"].options_score,original_score.map(|v|(v*100.).round_ties_even()/100.));
        state.option_snapshot_status.horizons[0].structure.main_magnet_score=Some(0.5);
        assert_eq!(confirmations(&state,now)["DAY"].options_score,Some(0.5));
        state.option_snapshot_status.horizons[0].structure.main_magnet_score=None;
        assert_eq!(confirmations(&state,now)["DAY"].status,"NO_OPTIONS_SCORE");
        state.option_snapshot_status.horizons[0].structure.main_magnet_score=original_score;

        let adjusted = adjusted_readings(&state, now);
        assert!(adjusted["DAY"].trf_applied);
        assert!(!adjusted["DAY"].flow_applied);
        assert_eq!(
            adjusted["DAY"].reading.side,
            state
                .option_snapshot_status
                .horizons
                .iter()
                .find(|h| h.mode == "DAY")
                .unwrap()
                .structure
                .reading
                .side
        );
        assert_eq!(confirmations(&state, now + 15_001)["DAY"].status, "WAITING");
        state.off_exchange.recovery_since_ms = Some(now + 1);
        assert!(!adjusted_readings(&state, now)["DAY"].trf_applied);
        assert_eq!(confirmations(&state, now)["DAY"].status, "WAITING");
        state.off_exchange.recovery_since_ms = None;
        state.off_exchange.queue_drops = 1;
        assert!(!adjusted_readings(&state, now)["DAY"].trf_applied);
        assert_eq!(confirmations(&state, now)["DAY"].status, "WAITING");
        state.off_exchange.queue_drops = 0;
        state.option_snapshot.as_mut().unwrap().symbol = "SPY".into();
        assert_eq!(confirmations(&state, now)["DAY"].status, "WAITING");
        state.option_snapshot.as_mut().unwrap().symbol = "QQQ".into();
        for h in &mut state.option_snapshot_status.horizons {
            h.structure.call_wall.as_mut().unwrap().strike = 510.0;
            h.structure.put_wall.as_mut().unwrap().strike = 490.0;
            for r in &mut h.structure.routes {
                r.target = Some(900.0); // Breakout target is not the preferred pull target.
                r.pull_target = Some(if r.direction == "CALL" { 505.0 } else { 495.0 });
            }
        }
        let maps = position_maps(&state, now);
        assert_eq!(maps.len(), 6);
        assert!(maps["DAY CALL"].ready && maps["DAY PUT"].ready);
        assert_eq!(maps["DAY CALL"].pull_target, Some(505.0));
        assert_eq!(maps["DAY CALL"].main_options_magnet, Some(500.0));
        assert_eq!(maps["DAY CALL"].liquidity_magnet, Some(510.0));
        assert!(!maps["DAY CALL"].entry_confirmed);
        for h in &mut state.option_snapshot_status.horizons {
            for wall in [&mut h.structure.call_wall, &mut h.structure.put_wall]
                .into_iter()
                .flatten()
            {
                wall.score = 70.0;
                wall.strength = "HIGH".into();
                wall.zone_available = true;
            }
            for route in &mut h.structure.routes {
                route.score = Some(70.0);
                route.structural_ready = true;
            }
        }
        state.options.phase = "streaming".into();
        state.options.last_event_ms = Some(now);
        let h = state
            .option_snapshot_status
            .horizons
            .iter()
            .find(|h| h.mode == "DAY")
            .unwrap();
        state.flow.contract_windows = snipers_core::wall_flow::structural_specs(&h.structure)
            .iter()
            .flat_map(|s| {
                [1, 5].map(|minutes| snipers_core::flow::ContractWindow {
                    ticker: format!("{}-{}", s.kind, s.strike),
                    kind: s.kind.clone(),
                    strike: s.strike,
                    expiration: "2026-09-24".into(),
                    minutes,
                    observed_since_ms: Some(now - 1_800_000),
                    covered: true,
                    contracts: Some(0.0),
                    premium: Some(0.0),
                    trades: Some(0),
                    reason: String::new(),
                })
            })
            .collect();
        let mut directional =
            snipers_core::flow::Engine::default().snapshot(now).windows[3].clone();
        directional.premium_complete = true;
        directional.event_count = 1;
        directional.bullish_pct = Some(60.0);
        directional.bearish_pct = Some(40.0);
        directional.direction = "BULLISH FLOW".into();
        state
            .flow
            .expiry_windows
            .insert("2026-09-24".into(), directional.clone());
        let pulls = wall_pulls(&state, now);
        assert!(adjusted_readings(&state, now)["DAY"].flow_applied);
        assert!(!adjusted_readings(&state, now)["WEEK"].flow_applied);
        assert!(pulls["DAY CALL"].ready);
        assert!(!pulls["DAY PUT"].ready);
        assert!(pulls["WEEK CALL"].ready);
        assert!(!pulls["WEEK CALL"].live_required);
        assert!(pulls.values().all(|p| !p.entry_confirmed));
        let mut unclassified = directional.clone();
        unclassified.bullish_pct = None;
        unclassified.bearish_pct = None;
        unclassified.direction = "UNCLASSIFIED".into();
        state
            .flow
            .expiry_windows
            .insert("2026-09-24".into(), unclassified);
        assert!(!adjusted_readings(&state, now)["DAY"].flow_applied);
        state.flow.expiry_windows.remove("2026-09-24");
        state
            .flow
            .expiry_windows
            .insert("2026-09-25".into(), directional);
        assert!(!wall_pulls(&state, now)["DAY CALL"].ready);
        assert!(!adjusted_readings(&state, now)["DAY"].flow_applied);
        assert!(wall_pulls(&state, now)["WEEK CALL"].ready);
        assert!(wall_pulls(&state, now + 15_001).values().all(|p| !p.ready));
        assert!(!position_maps(&state, now + 15_001)["DAY CALL"].ready);
        state.option_snapshot.as_mut().unwrap().symbol = "SPY".into();
        assert!(!position_maps(&state, now)["DAY CALL"].ready);
        state.option_snapshot.as_mut().unwrap().symbol = "QQQ".into();
        state.option_snapshot_status.phase = "loading".into();
        assert!(!position_maps(&state, now)["DAY CALL"].ready);

        // Exercise the separate structural path with recovered closed 2m bars.
        state.option_snapshot_status.phase = "ready".into();
        state.history.phase = "ready".into();
        state.metrics.session = "2026-09-24".into();
        state.metrics.asof_ms = Some(now - 120_000);
        state.metrics.rvol = Some(1.1);
        state.metrics.vwap = Some(500.0);
        for offset in 3..15 {
            let t = now - offset * 60_000;
            state.minutes.insert(
                t,
                crate::live::Bar {
                    timestamp_ms: t,
                    open: 500.1,
                    high: 500.25,
                    low: 499.99,
                    close: 500.2,
                    volume: 100.0,
                    vwap: Some(500.0),
                },
            );
        }
        for h in &mut state.option_snapshot_status.horizons {
            h.structure.call_wall.as_mut().unwrap().strike = 505.0;
            for route in &mut h.structure.routes {
                if route.direction == "CALL" {
                    route.vacuum_low = Some(500.5);
                    route.vacuum_high = Some(501.0);
                    route.vacuum_status = "VACUUM".into();
                    route.barrier = None;
                    route.barrier_score = None;
                }
            }
        }
        let mut flow = state.flow.expiry_windows["2026-09-25"].clone();
        flow.minutes = 1;
        flow.buckets.get_mut("CALL BUY").unwrap().contracts = 5.0;
        state.flow.windows = vec![flow];
        let liq = snipers_core::option_liquidity::Read {
            ready: true,
            contracts_tracked: state
                .flow
                .contract_windows
                .iter()
                .filter(|w| w.minutes == 1)
                .count(),
            ..Default::default()
        };
        let paths = crate::structural_path::calculate(&state, now, &liq);
        assert!(paths[0].trigger.ready, "{}", paths[0].trigger.reason);
        assert!(paths[0].candidate.ready, "{:?}", paths[0].candidate.blocks);
        assert!(!paths[0].candidate.entry_authorized);
        assert!(!paths[1].candidate.ready);
        assert!(!paths[0].simulation_compatible);
        assert_eq!(paths[0].preparation_reason, "ENTRY_MOVED_FROM_MAGNET");
        state.ticks.get_mut("QQQ").unwrap().price = Some(500.2);
        let prepared = crate::structural_path::calculate(&state, now, &liq);
        assert!(
            prepared[0].simulation_compatible,
            "{}",
            prepared[0].preparation_reason
        );
        assert_eq!(prepared[0].evidence.price, 500.2);
        assert!(serde_json::to_value(&prepared[0])
            .unwrap()
            .get("evidence")
            .is_none());
        assert!(
            !snipers_core::decision_prepare::prepare(&prepared[0].evidence, now + 3_001).is_ok()
        );
        let mut partial = state.minutes.values().next().unwrap().clone();
        partial.timestamp_ms = now - 120_000;
        state.minutes.insert(partial.timestamp_ms, partial.clone());
        // Unsettled latest pair must not hide the previous valid closed pair.
        assert!(
            crate::structural_path::calculate(&state, now, &liq)[0]
                .candidate
                .ready
        );
        // Once its close settles, the missing minute must block confirmation.
        assert!(
            !crate::structural_path::calculate(&state, now + 3000, &liq)[0]
                .candidate
                .ready
        );
        partial.timestamp_ms = now - 60_000;
        state.minutes.insert(partial.timestamp_ms, partial);
        assert!(
            crate::structural_path::calculate(&state, now + 3000, &liq)[0]
                .candidate
                .ready
        );
        assert!(
            crate::structural_path::calculate(&state, now + 15_001, &liq)
                .iter()
                .all(|r| !r.candidate.ready)
        );
        state.flow.contract_windows[0].covered = false;
        assert!(
            !crate::structural_path::calculate(&state, now, &liq)[0]
                .candidate
                .ready
        );
        state.flow.contract_windows[0].covered = true;
        state.option_snapshot.as_mut().unwrap().symbol = "SPY".into();
        assert!(crate::structural_path::calculate(&state, now, &liq)
            .iter()
            .all(|r| !r.candidate.ready));
    }
}
