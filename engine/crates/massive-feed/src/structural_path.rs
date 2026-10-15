//! Read-only active-symbol path assembled from already recovered inputs.
use crate::{interpretation, live::LiveState};
use snipers_core::{
    bars::{self, Minute},
    calendar::{self, Day},
    magnet_trigger,
    structural_path::{self, Input},
};
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub symbol: String,
    pub horizon: String,
    pub magnet: Option<f64>,
    pub target_wall: Option<f64>,
    pub vacuum_low: Option<f64>,
    pub vacuum_high: Option<f64>,
    pub trigger: magnet_trigger::Trigger,
    pub candidate: structural_path::Candidate,
    pub preparation_reason: String,
    pub simulation_compatible: bool,
    /// Backend evidence is never accepted back from the UI as an entry request.
    #[serde(skip)]
    pub evidence: snipers_core::decision_prepare::Evidence,
}
pub fn calculate(
    state: &LiveState,
    now: u64,
    liquidity: &snipers_core::option_liquidity::Read,
) -> Vec<Read> {
    let Some(date) = calendar::date_at(now) else {
        return vec![];
    };
    let Day::Open(session) = calendar::schedule(date) else {
        return vec![];
    };
    let Some(h) = state
        .option_snapshot_status
        .horizons
        .iter()
        .find(|h| matches!(h.mode.as_str(), "DAY" | "NEXT_SESSION"))
    else {
        return vec![];
    };
    let structure_ready = interpretation::structure_current(state, h, now);
    let spot = state
        .ticks
        .get(&state.symbol)
        .filter(|t| t.symbol == state.symbol)
        .and_then(|t| {
            [
                t.price
                    .zip(t.trade_ms)
                    .filter(|(p, _)| p.is_finite() && *p > 0.0),
                t.bid
                    .zip(t.ask)
                    .zip(t.quote_ms)
                    .filter(|((b, a), _)| b.is_finite() && a.is_finite() && *b > 0.0 && a >= b)
                    .map(|((b, a), time)| (b / 2.0 + a / 2.0, time)),
            ]
            .into_iter()
            .flatten()
            .max_by_key(|(_, time)| *time)
        });
    let complete = structure_ready
        && state.stocks.phase == "streaming"
        && state.history.phase == "ready"
        && spot.is_some_and(|(_, t)| {
            t <= now && now - t <= 15_000 && calendar::date_at(t) == Some(date)
        });
    let input: Vec<_> = state
        .minutes
        .range(now.saturating_sub(14 * 60_000)..now)
        .filter(|(_, b)| b.timestamp_ms.saturating_add(62_000) <= now)
        .map(|(_, b)| Minute {
            t: b.timestamp_ms,
            o: b.open,
            h: b.high,
            l: b.low,
            c: b.close,
            v: b.volume,
            vw: b.vwap,
        })
        .collect();
    let bars: Vec<_> = bars::session_bars(&input, session.open_ms, session.close_ms, 2)
        .into_iter()
        .filter(|b| b.timestamp.saturating_add(122_000) <= now)
        .collect();
    let maps = interpretation::position_maps(state, now);
    let readings = interpretation::adjusted_readings(state, now);
    let side = readings
        .get(&h.mode)
        .filter(|r| r.reading.available)
        .map(|r| r.reading.side.as_str());
    let position = side.and_then(|s| maps.get(&format!("{} {s}", h.mode)));
    let flow = state.flow.windows.iter().find(|w| w.minutes == 1);
    let coverage: Vec<_> = state
        .flow
        .contract_windows
        .iter()
        .filter(|w| w.minutes == 1)
        .collect();
    let flow_transport = state.options.phase == "streaming"
        && state
            .options
            .last_event_ms
            .is_some_and(|t| t <= now && now - t <= 15_000)
        && liquidity.contracts_tracked > 0
        && coverage.len() == liquidity.contracts_tracked
        && coverage.iter().all(|w| w.covered)
        && flow.is_some_and(|f| f.premium_complete && f.event_count > 0);
    let flow_side = flow.map_or("", |f| match f.direction.as_str() {
        "BULLISH FLOW" => "CALL",
        "BEARISH FLOW" => "PUT",
        _ => match f.bullish_pct.zip(f.bearish_pct) {
            Some((b, s)) if b >= s + 5.0 => "CALL",
            Some((b, s)) if s >= b + 5.0 => "PUT",
            _ => "",
        },
    });
    let rvol_ready = complete
        && state.metrics.session == date.to_string()
        && state
            .metrics
            .asof_ms
            .is_some_and(|t| t <= now && now - t <= 180_000)
        && state
            .metrics
            .rvol
            .is_some_and(|r| r.is_finite() && r >= 1.0);
    let magnet = h.structure.selected.get("M1").copied().flatten();
    ["CALL", "PUT"]
        .into_iter()
        .map(|side| {
            let route = h.structure.routes.iter().find(|r| r.direction == side);
            let wall = if side == "CALL" {
                h.structure.call_wall.as_ref()
            } else {
                h.structure.put_wall.as_ref()
            };
            let trigger = magnet_trigger::evaluate(
                side,
                magnet.unwrap_or(0.0),
                &bars,
                state.metrics.vwap,
                now,
                complete,
            );
            let input = Input {
                side: side.into(),
                magnet,
                wall: wall.map(|w| w.strike),
                wall_score: wall.map(|w| w.score),
                wall_strength: wall.map(|w| w.strength.clone()).unwrap_or_default(),
                vacuum_low: route.and_then(|r| r.vacuum_low),
                vacuum_high: route.and_then(|r| r.vacuum_high),
                vacuum_confirmed: route.is_some_and(|r| r.vacuum_status == "VACUUM"),
                trigger_ready: trigger.ready,
                entry: trigger.reference_price,
                tolerance: trigger.magnet_tolerance,
                volume_ratio: trigger.volume_ratio,
                flow_transport_ready: flow_transport,
                flow_side: flow_side.into(),
                buy_contracts: flow
                    .and_then(|f| f.buckets.get(&format!("{side} BUY")))
                    .map_or(0.0, |b| b.contracts),
                rvol_ready,
                option_liquidity_ready: liquidity.ready,
                position_ready: position.is_some_and(|p| p.ready),
                position_side: position.map(|p| p.side.clone()).unwrap_or_default(),
                barrier: route.and_then(|r| r.barrier),
                barrier_score: route.and_then(|r| r.barrier_score),
                barrier_strength: String::new(),
            };
            let candidate = structural_path::evaluate(&input, complete);
            let evidence = snipers_core::decision_prepare::Evidence {
                symbol: state.symbol.clone(),
                source_symbol: state
                    .option_snapshot
                    .as_ref()
                    .map(|s| s.symbol.clone())
                    .unwrap_or_default(),
                horizon: h.mode.clone(),
                published_ms: now,
                verified: complete,
                bars: bars.clone(),
                vwap: state.metrics.vwap,
                path: input.clone(),
                price: spot.map_or(0.0, |(p, _)| p),
                price_ms: spot.map_or(0, |(_, t)| t),
            };
            let preparation = snipers_core::decision_prepare::prepare(&evidence, now);
            Read {
                symbol: state.symbol.clone(),
                horizon: h.mode.clone(),
                magnet,
                target_wall: input.wall,
                vacuum_low: input.vacuum_low,
                vacuum_high: input.vacuum_high,
                trigger,
                candidate,
                simulation_compatible: preparation.is_ok(),
                preparation_reason: preparation
                    .err()
                    .unwrap_or("PENDING_SESSION_ADMISSION")
                    .into(),
                evidence,
            }
        })
        .collect()
}
