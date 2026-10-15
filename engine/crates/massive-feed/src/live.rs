//! Read-only streaming monitor. No signals, paper orders, or synthetic fallback.
use crate::{credentials::Credentials, endpoint, subscription, Event, Feed, Market};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedState {
    pub connection_faults: Vec<ConnectionFault>,
    pub quality: crate::telemetry::Quality,
    pub phase: String,
    pub detail: String,
    pub events: u64,
    pub malformed: u64,
    pub reconnects: u64,
    pub last_event_ms: Option<u64>,
    pub last_received_ms: Option<u64>,
    pub channels: usize,
}

/// Bounded diagnostics. Never retain URLs, credentials, payloads or server reason text.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionFault {
    pub at: u64,
    pub stage: String,
    pub kind: String,
    pub code: Option<i32>,
}

fn connection_fault(
    state: &SharedState,
    market: Market,
    stage: &str,
    kind: &str,
    code: Option<i32>,
) -> Failure {
    let mut s = state.lock().unwrap();
    let faults = &mut feed(&mut s, market).connection_faults;
    if faults.len() >= 20 {
        faults.remove(0);
    }
    faults.push(ConnectionFault {
        at: now_ms(),
        stage: stage.into(),
        kind: kind.into(),
        code,
    });
    Failure::Transport
}

fn socket_fault(
    state: &SharedState,
    market: Market,
    stage: &str,
    error: tokio_tungstenite::tungstenite::Error,
) -> Failure {
    use tokio_tungstenite::tungstenite::Error;
    let (kind, code) = match error {
        Error::Io(e) => ("io", e.raw_os_error()),
        Error::Tls(_) => ("tls", None),
        Error::Http(r) => ("http", Some(i32::from(r.status().as_u16()))),
        Error::Protocol(
            tokio_tungstenite::tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
        ) => ("reset_without_close_handshake", None),
        Error::Protocol(_) => ("websocket_protocol", None),
        Error::ConnectionClosed => ("connection_closed", None),
        Error::AlreadyClosed => ("already_closed", None),
        _ => ("websocket_error", None),
    };
    connection_fault(state, market, stage, kind, code)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tick {
    pub symbol: String,
    pub price: Option<f64>,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub trade_ms: Option<u64>,
    #[serde(skip)]
    pub trade_sequence: Option<u64>,
    pub quote_ms: Option<u64>,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bar {
    pub timestamp_ms: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub vwap: Option<f64>,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveState {
    #[serde(skip)]
    pub snapshot_lanes: BTreeMap<String, SharedState>,
    pub auction_map: Option<serde_json::Value>,
    #[serde(skip)]
    pub auction_engine: crate::auction_live::Engine,
    pub option_timing:crate::option_time_queue::Stats,
    #[serde(skip)]
    pub price_alert_engine: crate::price_alerts::Engine,
    pub price_alerts: Vec<crate::price_alerts::Alert>,
    #[serde(skip)]
    pub direction_enabled: bool,
    /// Passive research demand, independent of broker arming and selected screen.
    #[serde(skip)]
    pub research_symbols: Vec<String>,
    #[serde(skip)]
    pub qqq_components: Vec<String>,
    #[serde(skip)]
    pub qqq_ui_enabled: bool,
    #[serde(skip)]
    pub qqq_bot_demand: bool,
    pub qqq_report: serde_json::Value,
    pub swing_report: serde_json::Value,
    pub neuro_context: crate::neuro_context::Context,
    #[serde(skip)]
    pub neuro_contexts: BTreeMap<String, crate::neuro_context::Context>,
    #[serde(skip)]
    pub bot_enabled: bool,
    pub bot_qqq_only: bool,
    #[serde(skip)]
    pub bot_targets: Vec<String>,
    #[serde(skip)]
    pub bot_cvd_engines:BTreeMap<String,snipers_core::cvd::Engine>,
    #[serde(skip)]
    pub bot_held: Vec<String>,
    #[serde(skip)]
    pub bot_ticks: BTreeMap<String, Tick>,
    pub level_dynamics: crate::level_dynamics::Report,
    #[serde(skip)]
    pub level_tracker: crate::level_dynamics::Tracker,
    #[serde(skip)]
    pub greek_tracker: snipers_core::greek_support::Tracker,
    pub greek_support: snipers_core::greek_support::Context,
    pub level_deltas: Vec<serde_json::Value>,
    pub direction_oi: serde_json::Value,
    pub treasury: crate::treasury::Report,
    pub theta_vega: Vec<snipers_core::theta_vega::Read>,
    pub range_context: crate::range_context::Context,
    pub cme: crate::cme_runtime::Status,
    #[serde(skip)]
    pub spx_engine: crate::spx_synthetic::Engine,
    pub spx_synthetic: crate::spx_synthetic::Status,
    pub spx_reading: crate::spx_reading::Reading,
    #[serde(skip)]
    pub scanner_memory: snipers_core::scanner_memory::Memory,
    #[serde(skip)]
    option_fault_baseline: (u64, u64, u64, u64),
    #[serde(skip)]
    pub scanner_options: crate::scanner_options::Pool,
    #[serde(skip)]
    pub scanner_options_notify: Arc<tokio::sync::Notify>,
    #[serde(skip)]
    pub held_symbol: Option<String>,
    #[serde(skip)]
    pub legacy_symbols: Vec<String>,
    #[serde(skip)]
    pub legacy_ticks: std::collections::BTreeMap<String, Tick>,
    pub local_volume: snipers_core::local_volume::Read,
    #[serde(skip)]
    pub held_tick: Option<Tick>,
    pub decision_review: serde_json::Value,
    pub structural_paths: Vec<crate::structural_path::Read>,
    pub option_liquidity: snipers_core::option_liquidity::Read,
    pub scanner: crate::scanner::Report,
    #[serde(skip)]
    pub scanner_store: crate::scanner::Store,
    #[serde(skip)]
    pub scanner_notify: Arc<tokio::sync::Notify>,
    pub symbol: String,
    pub stocks: FeedState,
    pub options: FeedState,
    pub ticks: BTreeMap<String, Tick>,
    /// Provider's authoritative per-second aggregates, replacement keyed by start.
    pub bars: BTreeMap<u64, Bar>,
    pub minutes: BTreeMap<u64, Bar>,
    pub chart_history: crate::chart_history::ChartHistory,
    pub daily_atr: crate::daily_atr::Read,
    pub gap_report: snipers_core::gap_engine::Report,
    #[serde(skip)]
    pub gap_reports: BTreeMap<String,snipers_core::gap_engine::Report>,
    pub history: crate::history::HistoryState,
    pub metrics: crate::metrics::SessionMetrics,
    pub selection: SelectionState,
    pub journal_records: usize,
    pub overwritten_records: u64,
    pub flow: snipers_core::flow::Snapshot,
    pub cvd: snipers_core::cvd::Read,
    pub etf_cvd: BTreeMap<String,snipers_core::cvd::Read>,
    pub etf_flow: BTreeMap<String,serde_json::Value>,
    pub etf_blocks: BTreeMap<String,snipers_core::block_pressure::Report>,
    #[serde(skip)]
    pub(crate) etf_block_engines:BTreeMap<String,snipers_core::off_exchange_tape::Engine>,
    #[serde(skip)]
    pub(crate) etf_block_quotes:BTreeMap<String,snipers_core::off_exchange::Quote>,
    #[serde(skip)]
    pub etf_scanner_enabled: bool,
    #[serde(skip)]
    pub(crate) etf_cvd_engines: BTreeMap<String,snipers_core::cvd::Engine>,
    #[serde(skip)]
    pub(crate) cvd_engine: snipers_core::cvd::Engine,
    pub off_exchange: crate::off_exchange::State,
    pub magnet_confirmations: BTreeMap<String, snipers_core::magnet_confirmation::Confirmation>,
    pub position_maps: BTreeMap<String, snipers_core::position_map::Map>,
    pub wall_flow: BTreeMap<String, crate::interpretation::WallFlow>,
    pub wall_pull: BTreeMap<String, snipers_core::wall_pull::Read>,
    pub adjusted_readings: BTreeMap<String, snipers_core::reading_overlay::Adjusted>,
    pub adjusted_alignment: Option<snipers_core::reading::Alignment>,
    #[serde(skip)]
    pub(crate) off_exchange_tx: Option<tokio::sync::mpsc::Sender<crate::off_exchange::Input>>,
    #[serde(skip)]
    pub off_exchange_notify: Arc<tokio::sync::Notify>,
    pub option_snapshot_status: crate::option_snapshot::SnapshotState,
    #[serde(skip)]
    pub option_snapshot: Option<snipers_core::options::Observation>,
    #[serde(skip)]
    pub oi_directory: Option<std::path::PathBuf>,
    #[serde(skip)]
    pub snapshot_notify: Arc<tokio::sync::Notify>,
    #[serde(skip)]
    pub(crate) flow_engine: snipers_core::flow::Engine,
    #[serde(skip)]
    flow_second: Option<u64>,
    #[serde(skip)]
    pub telemetry: crate::telemetry::Telemetry,
    #[serde(skip)]
    pub background_capture: Option<crate::background_capture::Capture>,
    #[serde(skip)]
    pub(crate) metrics_key: Option<(u64, u64)>,
    #[serde(skip)]
    pub(crate) minute_versions: BTreeMap<u64, u64>,
    #[serde(skip)]
    pub(crate) minute_revision: u64,
    #[serde(skip)]
    pub(crate) history_notify: Arc<tokio::sync::Notify>,
    pub published_ms: u64,
    pub started_ms: u64,
}

impl LiveState {
    pub fn set_etf_scanner_enabled(&mut self,enabled:bool){
        if self.etf_scanner_enabled!=enabled {
            self.etf_cvd_engines.clear();self.etf_block_engines.clear();self.etf_block_quotes.clear();
            self.etf_cvd.clear();self.etf_flow.clear();self.etf_blocks.clear();
        }
        self.etf_scanner_enabled=enabled;self.scanner_options_notify.notify_one();
    }
}
pub type SharedState = Arc<Mutex<LiveState>>;
pub fn direction_pool_symbols(mut existing:Vec<String>,active:&str,enabled:bool)->Vec<String>{
    if enabled {for symbol in ["QQQ","SPY","NVDA","AAPL","MSFT","AMZN","AVGO","META","GOOGL","TSLA"] {
        if existing.len()>=10 {break;}
        if symbol!=active && !existing.iter().any(|s|s==symbol) && snipers_core::universe::stock_asset(symbol).is_some(){existing.push(symbol.into());}
    }}existing
}
/// Per-asset evidence: never borrow the selected monitor's flow, time or candles.
pub fn bot_frame(state:&SharedState,symbol:&str,now:u64)->serde_json::Value {
    let mut s=state.lock().unwrap();
    let history=s.scanner_store.bot_history(symbol,now);
    let data=s.scanner_options.bot_data(symbol,now);
    let tick=s.bot_ticks.get(symbol).cloned();
    let (mut metrics,minutes)=history.unwrap_or_default();
    if metrics.session.is_empty(){metrics=crate::metrics::calculate(&BTreeMap::new(),now);}
    let cvd=s.bot_cvd_engines.get(symbol).map(|e|e.snapshot(symbol,now,s.stocks.phase=="streaming"));
    let session=snipers_core::calendar::date_at(now).and_then(|d|match snipers_core::calendar::schedule(d){snipers_core::calendar::Day::Open(s)=>Some(s),_=>None});
    serde_json::json!({"priceBaselines":s.scanner_store.bot_baselines(symbol,now),"rthOpen":s.scanner_store.bot_open(symbol,now),"cvd":cvd,"gapReport":s.gap_reports.get(symbol),"backgroundBot":true,"symbol":symbol,"startedMs":s.started_ms,"publishedMs":now,
        "optionTiming":s.option_timing,"optionFlowStatus":data["flowStatus"],"ticks":{symbol:tick},"stocks":s.stocks,"options":{"phase":s.options.phase,"lastEventMs":data["lastEventMs"]},
        "treasury":s.treasury,"directionOi":data["context"]["directionOi"],"flowExpiry":data["flowExpiry"],"dailyRange":s.scanner_store.bot_daily_range(symbol,now),"metrics":metrics,"chartHistory":{"minutes":minutes,"sessions":session.into_iter().collect::<Vec<_>>()},"flow":data["flow"],"localVolume":null,
        "optionSnapshotStatus":data["context"]})
}
pub fn new_state() -> SharedState {
    new_state_for("QQQ").expect("QQQ configured")
}
pub fn new_state_for(symbol: &str) -> Result<SharedState, &'static str> {
    if snipers_core::universe::stock_asset(symbol).is_none() {
        return Err("Activo no habilitado para Stocks.");
    }
    let mut state = LiveState {
        symbol: symbol.into(),
        started_ms: now_ms(),
        cvd_engine: snipers_core::cvd::Engine::analysis(),
        ..Default::default()
    };
    state.scanner_store.initialize(now_ms());
    Ok(Arc::new(Mutex::new(state)))
}

/// Bound IPC payload independently from the history retained for RVOL baselines.
pub fn snapshot(state: &SharedState) -> LiveState {
    snapshot_at(state,now_ms())
}
/// Explicit clock for reproducible offline recordings; live callers use snapshot.
pub fn snapshot_for_audit(state:&SharedState,as_of_ms:u64)->LiveState {snapshot_at(state,as_of_ms)}
fn snapshot_at(state:&SharedState,now:u64)->LiveState {
    let mut s = state.lock().unwrap();
    let etf_enabled=s.etf_scanner_enabled;
    let date = snipers_core::calendar::date_at(now)
        .map(|d| d.to_string())
        .unwrap_or_default();
    if !s.scanner_store.tapes.is_empty() && s.scanner_store.date != date {
        s.scanner_store.date = date;
        s.scanner_store.invalidate();
        s.scanner_notify.notify_one();
    }
    let connected = s.stocks.phase == "streaming";
    let mut scanner = s.scanner_store.snapshot(now, connected);
    scanner.dealer_contexts=s.neuro_contexts.iter().map(|(symbol,c)|
        (symbol.clone(),c.checked_display(symbol,now,s.ticks.get(symbol).or_else(||s.bot_ticks.get(symbol)).and_then(|t|t.price.zip(t.trade_ms)),connected))).collect();
    let mut pool_symbols=if s.bot_enabled {if s.bot_qqq_only {vec!["QQQ".into()]}else{scanner.bot_selection.symbols.clone()}}else{scanner.selection.symbols.clone()};
    for symbol in &s.bot_held {if !pool_symbols.contains(symbol){pool_symbols.push(symbol.clone());}}
    pool_symbols=crate::qqq_components::pool_symbols(pool_symbols,&s.bot_held,s.bot_enabled,&s.qqq_components);
    pool_symbols=direction_pool_symbols(pool_symbols,&s.symbol,s.direction_enabled);
    for symbol in s.snapshot_lanes.keys() {if !pool_symbols.contains(symbol){pool_symbols.push(symbol.clone());}}
    for symbol in &s.research_symbols {if !pool_symbols.contains(symbol){pool_symbols.push(symbol.clone());}}
    if s.etf_scanner_enabled {for symbol in ["SPY","QQQ"] {if !pool_symbols.iter().any(|s|s==symbol)&&pool_symbols.len()<10 {pool_symbols.push(symbol.into());}}}
    s.scanner_options.background=s.bot_enabled || !s.bot_held.is_empty() || s.direction_enabled || s.etf_scanner_enabled || !s.qqq_components.is_empty() || !s.research_symbols.is_empty() || !s.snapshot_lanes.is_empty();
    s.scanner_options.capacity=if s.qqq_components.is_empty()&&s.research_symbols.is_empty()&&s.snapshot_lanes.is_empty(){10}else{20};
    s.scanner_options.preparation=s.direction_enabled || !s.qqq_components.is_empty();
    s.scanner_options.priority=s.bot_held.clone();
    for symbol in s.snapshot_lanes.keys().cloned().collect::<Vec<_>>() {if !s.scanner_options.priority.contains(&symbol){s.scanner_options.priority.push(symbol);}}
    for symbol in s.research_symbols.clone() {if !s.scanner_options.priority.contains(&symbol){s.scanner_options.priority.push(symbol);}}
    if s.etf_scanner_enabled || !s.qqq_components.is_empty() {for symbol in ["SPY","QQQ"] {if !s.scanner_options.priority.iter().any(|s|s==symbol) {s.scanner_options.priority.push(symbol.into());}}}
    for symbol in s.qqq_components.clone(){if !s.scanner_options.priority.contains(&symbol){s.scanner_options.priority.push(symbol);}}
    if s.scanner_options.reconcile(&pool_symbols, now)
        && (!scanner.selection.symbols.is_empty() || s.options.phase == "streaming")
    {
        s.scanner_options_notify.notify_one();
    }
    for row in &scanner.rows {
        if let (Some(price),Some(at))=(row.price,row.bar_end_ms) {s.scanner_options.reference(&row.symbol,price,at,now);}
    }
    for candidate in &scanner.top {
        s.scanner_options.reference(
            &candidate.symbol,
            candidate.price,
            candidate.bar_end_ms,
            now,
        );
    }
    if s.direction_enabled || s.etf_scanner_enabled || !s.qqq_components.is_empty() || !s.research_symbols.is_empty() {
        for symbol in &pool_symbols {
            if let Some((price,at))=s.bot_ticks.get(symbol).or_else(||s.ticks.get(symbol)).and_then(|t|t.price.zip(t.trade_ms)) {
                s.scanner_options.reference(symbol,price,at,now);
            }
        }
    }
    scanner.options_context = s.scanner_options.snapshot(now, connected);
    if scanner.selection.symbols.contains(&s.symbol) {
        let snap = &s.option_snapshot_status;
        let valid = connected
            && snap.phase == "ready"
            && snap.exposure_error.is_none()
            && !snap.horizons.is_empty()
            && snap.observed_ms.is_some_and(|t| {
                t <= now
                    && now - t <= 900_000
                    && snipers_core::calendar::date_at(t) == snipers_core::calendar::date_at(now)
            });
        scanner.options_context.insert(
            s.symbol.clone(),
            crate::scanner_options::Context {
                daily_range:if valid{snap.daily_range.clone()}else{None},
                direction_oi: serde_json::Value::Null,
        symbol: s.symbol.clone(),
                phase: if valid { "ready" } else { "waiting" }.into(),
                detail: if valid {
                    "Snapshot del monitor compartido · no es flujo live"
                } else {
                    "Esperando snapshot vigente del monitor"
                }
                .into(),
                observed_ms: snap.observed_ms,
                reference_ms: None,
                contracts: snap.contracts,
                horizons: if valid { snap.horizons.clone() } else { vec![] },
                entry_confirmed: false,
            },
        );
    }
    let key = (s.minute_revision, now / 120_000);
    if s.metrics_key != Some(key) {
        s.metrics = crate::metrics::calculate(&s.minutes, now);
        s.chart_history = crate::chart_history::collect(&s.minutes, now);
        s.metrics_key = Some(key);
    }
    s.stocks.quality = s.telemetry.quality(Market::Stocks);
    s.options.quality = s.telemetry.quality(Market::Options);
    refresh_flow(&mut s, now);
    scanner_flow_confirmations(&mut s, &mut scanner, now);
    scanner_final_confirmations(&mut s, &mut scanner, now);
    let adjusted_readings = crate::interpretation::adjusted_readings(&s, now);
    let side = |modes: &[&str]| {
        modes
            .iter()
            .find_map(|m| adjusted_readings.get(*m))
            .filter(|r| r.reading.available)
            .map_or("N/A", |r| r.reading.side.as_str())
    };
    let adjusted_alignment = Some(snipers_core::reading::align([
        side(&["DAY", "NEXT_SESSION"]),
        side(&["WEEK"]),
        side(&["NEXT4"]),
    ]));
    let option_liquidity = s.flow_engine.liquidity(now, s.options.phase == "streaming");
    let greek_symbol = s.symbol.clone();
    let greek_connected = s.options.phase == "streaming"
        && s.stocks.phase == "streaming"
        && s.option_snapshot_status.phase == "ready";
    let greek_support = {
        let LiveState {
            greek_tracker,
            option_snapshot,
            ..
        } = &mut *s;
        greek_tracker.update(
            option_snapshot.as_ref(),
            &greek_symbol,
            greek_connected,
            now,
        )
    };
    let mut level_tracker = std::mem::take(&mut s.level_tracker);
    let level_dynamics = level_tracker.update(&s, now);
    s.level_tracker = level_tracker;
    let auction_map={let symbol=s.symbol.clone();let bars=s.chart_history.minutes.iter().filter(|b|b.timestamp_ms+60000<=now).map(|b|snipers_core::Candle{timestamp_ms:b.timestamp_ms,open:b.open,high:b.high,low:b.low,close:b.close,volume:b.volume.max(0.) as u64}).collect();s.auction_engine.snapshot(&symbol,now,connected,bars)};
    LiveState {
        neuro_context: s.neuro_contexts.get(&s.symbol).cloned().unwrap_or_default().checked_display(&s.symbol, now, s.ticks.get(&s.symbol).and_then(|t|t.price.zip(t.trade_ms)), connected),
        neuro_contexts: s.neuro_contexts.clone(),
        level_dynamics,
        greek_support,
        treasury: s.treasury.clone(),
        direction_oi: s.option_snapshot.as_ref().filter(|o|connected && o.symbol==s.symbol && o.observed_ms<=now && now-o.observed_ms<=900_000).map(crate::direction_context::oi).unwrap_or_default(),
        level_deltas: s.option_snapshot.as_ref().filter(|o| connected && o.symbol == s.symbol && o.observed_ms <= now && now - o.observed_ms <= 900_000).map(|o| o.contracts.values().filter(|c| c.delta.is_some_and(|d| d.is_finite() && d.abs() <= 1.)).map(|c| serde_json::json!({"ticker":c.ticker,"strike":c.strike,"kind":c.kind,"expiration":c.expiration,"delta":c.delta,"gamma":c.gamma,"oi":c.open_interest,"observedMs":o.observed_ms})).collect()).unwrap_or_default(),
        theta_vega: s
            .option_snapshot
            .as_ref()
            .map(|o| snipers_core::theta_vega::overview(o, &s.symbol, now))
            .unwrap_or_default(),
        range_context: crate::range_context::calculate(&s, now, adjusted_alignment.as_ref()),
        local_volume: s.local_volume.clone(),
        spx_synthetic: s.spx_engine.snapshot(now, s.options.phase == "streaming"),
        spx_reading: s.spx_reading.current(
            now,
            &s.spx_engine.snapshot(now, s.options.phase == "streaming"),
        ),
        held_symbol: s.held_symbol.clone(),
        legacy_symbols: s.legacy_symbols.clone(),
        legacy_ticks: s.legacy_ticks.clone(),
        held_tick: s.held_tick.clone(),
        structural_paths: crate::structural_path::calculate(&s, now, &option_liquidity),
        option_liquidity,
        scanner,
        symbol: s.symbol.clone(),
        price_alerts: s.price_alert_engine.snapshot(),
        option_timing:s.option_timing.clone(),
        stocks: s.stocks.clone(),
        options: s.options.clone(),
        ticks: s.ticks.clone(),
        bars: s.bars.clone(),
        minutes: s
            .minutes
            .iter()
            .rev()
            .take(900)
            .map(|(t, b)| (*t, b.clone()))
            .collect(),
        chart_history: s.chart_history.clone(),
        daily_atr: s.daily_atr.clone(),
        gap_report: s.gap_report.clone(),
        gap_reports: BTreeMap::new(),
        swing_report: s.swing_report.clone(),
        history: s.history.clone(),
        metrics: s.metrics.clone(),
        selection: s.selection.clone(),
        journal_records: s.telemetry.records.len(),
        overwritten_records: s.telemetry.overwritten_records,
        auction_map,
        auction_engine: Default::default(),
        flow: s.flow.clone(),
        cvd: {let mut cvd=s.cvd_engine.snapshot(&s.symbol, now, connected);cvd.recent_five=s.cvd_engine.recent_at(s.flow.asof_ms);cvd},
        etf_cvd: ["SPY","QQQ"].into_iter().filter(|_|etf_enabled).map(|symbol| (symbol.to_owned(),s.etf_cvd_engines.get(symbol).map_or_else(|| snipers_core::cvd::Engine::default().snapshot(symbol,now,connected),|engine|engine.snapshot(symbol,now,connected)))).collect(),
        bot_cvd_engines: Default::default(),
        qqq_report:s.qqq_report.clone(),
        bot_targets:s.bot_targets.clone(),
        bot_qqq_only:s.bot_qqq_only,
        etf_cvd_engines: Default::default(),
        etf_flow: {let options_connected=connected && s.options.phase=="streaming";["SPY","QQQ"].into_iter().filter(|_|etf_enabled).map(|symbol| (symbol.to_owned(),s.scanner_options.etf_data(symbol,now,options_connected))).collect()},
        etf_scanner_enabled: s.etf_scanner_enabled,
        etf_blocks: s.etf_block_engines.iter_mut().filter(|_|etf_enabled).map(|(symbol,engine)|(symbol.clone(),engine.snapshot(now).blocks)).collect(),
        etf_block_engines: Default::default(),
        etf_block_quotes: Default::default(),
        off_exchange: s.off_exchange.clone(),
        magnet_confirmations: crate::interpretation::confirmations(&s, now),
        position_maps: crate::interpretation::position_maps(&s, now),
        wall_flow: crate::interpretation::wall_flow(&s, now),
        wall_pull: crate::interpretation::wall_pulls(&s, now),
        adjusted_readings,
        adjusted_alignment,
        option_snapshot_status: s.option_snapshot_status.clone(),
        published_ms: now,
        started_ms: s.started_ms,
        ..Default::default()
    }
}
fn option_faults(s: &LiveState) -> (u64, u64, u64, u64) {
    let q = s.telemetry.quality(Market::Options);
    (
        s.options.malformed,
        q.sequence_conflicts,
        q.unidentified_trades,
        q.identity_evictions,
    )
}
fn reset_option_coverage(s: &mut LiveState) {
    s.spx_engine.reset();
    s.option_fault_baseline = option_faults(s);
    s.flow_engine.reset_coverage();
    s.scanner_options.reset_coverage();
    s.flow_second = None;
}
fn refresh_flow(s: &mut LiveState, now: u64) {
    if option_faults(s) != s.option_fault_baseline {
        s.flow_engine.invalidate_coverage();
        s.scanner_options.invalidate_coverage();
        s.flow_second = None;
    }
    if s.flow_second != Some(now / 1000) {
        s.flow = s.flow_engine.snapshot(now);
        s.local_volume = s
            .flow_engine
            .local_volume(&s.symbol, now, s.options.phase == "streaming");
        s.flow_second = Some(now / 1000);
    }
    if s.options.phase != "streaming" {
        s.local_volume.usable = false;
        s.local_volume.state = "ESPERANDO".into();
        s.local_volume.reason = "WEBSOCKET SIN SUSCRIPCION LIVE VERIFICADA".into();
    }
}
fn scanner_flow_confirmations(s: &mut LiveState, scanner: &mut crate::scanner::Report, now: u64) {
    let coverage: Vec<_> = s
        .flow
        .contract_windows
        .iter()
        .filter(|w| w.minutes == 1)
        .collect();
    let coverage_complete = !coverage.is_empty()
        && coverage
            .iter()
            .all(|w| w.covered && option_underlying(&w.ticker) == Some(s.symbol.as_str()));
    for candidate in &scanner.top {
        if candidate.symbol != s.symbol {
            scanner.flow_confirmations.insert(
                candidate.symbol.clone(),
                s.scanner_options.flow_read(
                    &candidate.symbol,
                    &candidate.direction,
                    now,
                    s.options.phase == "streaming",
                ),
            );
            scanner.option_liquidity.insert(
                candidate.symbol.clone(),
                s.scanner_options
                    .liquidity(&candidate.symbol, now, s.options.phase == "streaming"),
            );
            continue;
        }
        scanner.option_liquidity.insert(
            candidate.symbol.clone(),
            s.flow_engine.liquidity(now, s.options.phase == "streaming"),
        );
        let evidence = s
            .flow
            .windows
            .iter()
            .find(|w| w.minutes == 1)
            .filter(|_| candidate.symbol == s.symbol)
            .map(|window| snipers_core::scanner_confirmation::FlowEvidence {
                symbol: &s.symbol,
                calculated_ms: s.flow_second.unwrap_or(0) * 1000,
                verified: s.options.phase == "streaming",
                coverage_complete,
                window,
            });
        scanner.flow_confirmations.insert(
            candidate.symbol.clone(),
            snipers_core::scanner_confirmation::flow(
                &candidate.symbol,
                &candidate.direction,
                evidence,
                now,
            ),
        );
    }
}

fn scanner_final_confirmations(s: &mut LiveState, report: &mut crate::scanner::Report, now: u64) {
    use snipers_core::{scanner_memory, scanner_structure, scanner_trigger};
    s.scanner_memory.reconcile(&report.selection.symbols, now);
    for symbol in &report.selection.symbols {
        let candidate = report.top.iter().find(|c| &c.symbol == symbol);
        let flow = report.flow_confirmations.get(symbol);
        let liquid = report.option_liquidity.get(symbol);
        let context = report
            .options_context
            .get(symbol)
            .filter(|c| c.symbol == *symbol && c.phase == "ready");
        let contracts = if *symbol == s.symbol {
            s.flow.contract_windows.as_slice()
        } else {
            s.scanner_options.contract_windows(symbol, now)
        };
        let bullish = candidate.is_some_and(|c| c.direction == "ALCISTA");
        let verified = report.connected && s.options.phase == "streaming" && candidate.is_some();
        let structure = scanner_structure::evaluate(
            context.and_then(|c| c.horizons.iter().find(|h| h.mode == "NEXT4")),
            candidate.map_or(f64::NAN, |c| c.price),
            bullish,
            contracts,
            verified,
            now,
        );
        let all_gates = candidate.is_some_and(|c| c.base_ready)
            && flow.is_some_and(|f| f.ready && f.aligned == Some(true))
            && liquid.is_some_and(|l| l.ready)
            && structure.ready;
        let empty_trigger = scanner_trigger::Read::default();
        let trigger = report.triggers.get(symbol).unwrap_or(&empty_trigger);
        let mut confirmation = s.scanner_memory.evaluate(
            &scanner_memory::Evidence {
                symbol,
                bar_ms: candidate.map_or(0, |c| c.bar_end_ms),
                close: candidate.map_or(f64::NAN, |c| c.price),
                vwap: candidate.and_then(|c| c.vwap).unwrap_or(f64::NAN),
                bullish,
                verified,
                all_gates,
                targets: &structure.targets,
                trigger,
            },
            now,
        );
        if confirmation.state == "NO CONFIRMADO" && verified {
            confirmation.reason = if !candidate.is_some_and(|c| c.base_ready) {
                "RVOL / DIRECCION PENDIENTES".into()
            } else if !flow.is_some_and(|f| f.ready && f.aligned == Some(true)) {
                flow.map_or("ESPERANDO FLOW PROPIO", |f| f.reason.as_str())
                    .into()
            } else if !liquid.is_some_and(|l| l.ready) {
                liquid
                    .map_or("ESPERANDO LIQUIDEZ PROPIA", |l| l.status.as_str())
                    .into()
            } else {
                structure.reason.clone()
            };
        }
        report
            .structure_confirmations
            .insert(symbol.clone(), structure);
        report.confirmations.insert(symbol.clone(), confirmation);
    }
}

fn feed(s: &mut LiveState, market: Market) -> &mut FeedState {
    match market {
        Market::Stocks => &mut s.stocks,
        Market::Options => &mut s.options,
    }
}
fn status(state: &SharedState, market: Market, phase: &str, detail: &str) {
    crate::snapshot_lanes::phase(state,market,phase,detail);
    let mut s = state.lock().unwrap();
    if matches!(market, Market::Stocks) && phase == "connecting" {
        s.price_alert_engine.reset();
        s.cvd_engine.gap();
        s.auction_engine.gap();
        for engine in s.bot_cvd_engines.values_mut(){engine.gap();}
        for engine in s.etf_cvd_engines.values_mut(){engine.gap();}
        s.etf_block_engines.clear();
        s.etf_block_quotes.clear();
        s.off_exchange.gap(now_ms());
        s.scanner_store.invalidate();
        s.scanner_notify.notify_one();
    }
    let f = feed(&mut s, market);
    f.phase = phase.into();
    f.detail = detail.into();
}

/// Futures are joined in this task, not detached. Dropping it closes both sockets.
pub async fn run(key: Credentials, state: SharedState) {
    crate::snapshot_lanes::initialize(&state);
    let (tx, rx) = tokio::sync::mpsc::channel(10_000);
    state.lock().unwrap().off_exchange_tx = Some(tx.clone());
    if state.lock().unwrap().scanner_store.tapes.is_empty() {
        state.lock().unwrap().scanner_store.initialize(now_ms());
    }
    tokio::join!(
        crate::snapshot_lanes::workers(&key,state.clone()),
        crate::snapshot_lanes::file_capture(state.clone()),
        supervise(&key, state.clone(), Market::Stocks),
        supervise(&key, state.clone(), Market::Options),
        crate::history::worker(&key, state.clone()),
        crate::auction_session::worker(&key,state.clone()),
        crate::daily_atr::worker(&key, state.clone()),
        crate::gap_engine::worker(&key, state.clone()),
        crate::option_snapshot::worker(&key, state.clone()),
        crate::off_exchange::worker(&key, state.clone(), tx, rx),
        crate::scanner::worker(&key, state.clone()),
        crate::scanner_options::worker(&key, state.clone()),
        crate::spx_synthetic::worker(&key, state.clone()),
        crate::treasury::worker(&key, state.clone()),
        crate::spx_reading::worker(&key, state)
    );
}

async fn supervise(key: &Credentials, state: SharedState, market: Market) {
    supervise_sessions(&state, market, || session(key, &state, market)).await;
}

async fn supervise_sessions<F, Fut>(state: &SharedState, market: Market, mut connect: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<(), Failure>>,
{
    let mut attempt: u32 = 0;
    let mut connection_limits: u32 = 0;
    loop {
        status(
            state,
            market,
            "connecting",
            "Conectando al feed realtime de Massive",
        );
        let started = std::time::Instant::now();
        let attempt_ms = now_ms();
        let result = connect().await;
        if let Err(error) = &result {
            record_session_failure(state, market, attempt_ms, error);
        }
        match result {
            Err(Failure::ConnectionLimit) if connection_limits < 4 => {
                // A just-closed socket can still occupy the provider's slot when
                // switching assets. Retry only this transient rejection, bounded.
                let wait = 2u64.pow(connection_limits + 1);
                connection_limits += 1;
                status(
                    state,
                    market,
                    "reconnecting",
                    &format!("Massive aún tiene una conexión ocupada. Reintento {connection_limits}/4 en {wait} s."),
                );
                feed(&mut state.lock().unwrap(), market).reconnects += 1;
                sleep(Duration::from_secs(wait)).await;
                continue;
            }
            Err(Failure::ConnectionLimit) => {
                status(state, market, "denied", "Límite de conexiones persistente tras 4 reintentos. Cierra otros consumidores de Options/Stocks y vuelve a conectar.");
                return;
            }
            Err(Failure::Denied) => {
                status(
                    state,
                    market,
                    "denied",
                    "Autenticación o suscripción rechazada. Revisa los permisos de la cuenta.",
                );
                return;
            }
            Err(Failure::Limit) => {
                status(
                    state,
                    market,
                    "denied",
                    "La selección supera el límite de suscripciones permitido.",
                );
                return;
            }
            _ => {}
        }
        if started.elapsed() > Duration::from_secs(60) {
            attempt = 0;
            connection_limits = 0;
        }
        let wait = (2u64.pow(attempt.min(5))).min(30);
        status(
            state,
            market,
            "reconnecting",
            "Conexión interrumpida. Puede haber huecos: recuperación histórica pendiente.",
        );
        {
            feed(&mut state.lock().unwrap(), market).reconnects += 1;
        }
        // Bounded backoff plus jitter avoids synchronized reconnect storms.
        sleep(Duration::from_millis(wait * 1000 + now_ms() % 500)).await;
        attempt = attempt.saturating_add(1);
    }
}

#[derive(Debug)]
enum Failure {
    Transport,
    Denied,
    Limit,
    ConnectionLimit,
    Diagnostic(&'static str, &'static str, Option<i32>),
}

fn record_session_failure(state: &SharedState, market: Market, attempt_ms: u64, error: &Failure) {
    let stage = {
        let mut s = state.lock().unwrap();
        let f = feed(&mut s, market);
        if f.connection_faults
            .last()
            .is_some_and(|e| e.at >= attempt_ms)
        {
            return;
        }
        f.phase.clone()
    };
    let (stage, kind, code) = match error {
        Failure::Diagnostic(stage, kind, code) => (*stage, *kind, *code),
        Failure::Denied => (stage.as_str(), "access_denied", None),
        Failure::Limit => (stage.as_str(), "connection_or_subscription_limit", None),
        Failure::ConnectionLimit => (stage.as_str(), "connection_limit", None),
        Failure::Transport => (stage.as_str(), "selection_or_transport_failure", None),
    };
    connection_fault(state, market, stage, kind, code);
}

fn passive_stock_quote_channels(base:&[String],state:&LiveState)->Vec<String>{
    let mut channels=base.to_vec();
    let mut roots=std::collections::BTreeSet::new();
    for root in state.research_symbols.iter().chain(&state.bot_targets).chain(&state.bot_held).chain(&state.qqq_components){
        if roots.len()>=20&&!roots.contains(root){continue;}
        if snipers_core::universe::stock_asset(root).is_some(){roots.insert(root.clone());}
    }
    roots.extend(state.snapshot_lanes.keys().cloned());
    for root in roots {let channel=format!("Q.{root}");if !channels.contains(&channel){channels.push(channel);}}
    channels
}
async fn session(key: &Credentials, state: &SharedState, market: Market) -> Result<(), Failure> {
    let symbol = state.lock().unwrap().symbol.clone();
    let channels = match market {
        Market::Stocks => {
            let mut all = ["T", "Q", "A", "AM"]
                .map(|k| format!("{k}.{symbol}"))
                .to_vec();
            all.extend(["SPY","QQQ"].into_iter().filter(|ticker| *ticker != symbol).flat_map(|ticker| [format!("T.{ticker}"),format!("Q.{ticker}")]));
            all.extend(state.lock().unwrap().snapshot_lanes.keys().flat_map(|ticker|["T","Q","A","AM"].map(|k|format!("{k}.{ticker}"))));
            all.extend(
                state
                    .lock()
                    .unwrap()
                    .scanner_store
                    .tapes
                    .keys()
                    .filter(|s| *s != &symbol)
                    .map(|s| format!("AM.{s}")),
            );

            all.extend(state.lock().unwrap().scanner_store.tapes.keys().filter(|s|*s!=&symbol).map(|s|format!("T.{s}")));
            if let Some(held) = state
                .lock()
                .unwrap()
                .held_symbol
                .as_ref()
                .filter(|held| *held != &symbol)
            {
                all.push(format!("T.{held}"));
            }
            for held in &state.lock().unwrap().legacy_symbols {
                let channel = format!("T.{held}");
                if held != &symbol && !all.contains(&channel) {
                    all.push(channel);
                }
            }
            let mut seen=std::collections::BTreeSet::new();all.retain(|c|seen.insert(c.clone()));
            all
        }
        Market::Options => {
            status(
                state,
                market,
                "discovering",
                "Seleccionando contratos del activo y vencimiento vigente",
            );
            let mut selection = discover(key, &symbol, current_spot(state, now_ms())).await?;
            extend_structural_selection(&mut selection, &state.lock().unwrap(), now_ms())?;
            extend_pool_selection(&mut selection, &state.lock().unwrap(), now_ms())?;
            {
                let mut s = state.lock().unwrap();
                s.selection = selection.info;
                install_option_contracts(&mut s, selection.contracts);
                s.snapshot_notify.notify_one();
            }
            selection.channels
        }
    };
    session_channels(
        key,
        state,
        market,
        &channels,
        endpoint(market, Feed::Realtime),
    )
    .await
}

async fn session_channels(
    key: &Credentials,
    state: &SharedState,
    market: Market,
    channels: &[String],
    url: &str,
) -> Result<(), Failure> {
    session_with_rotation(key, state, market, channels, url, || {
        next_selection(key, state)
    })
    .await
}

async fn session_with_rotation<F, Fut>(
    key: &Credentials,
    state: &SharedState,
    market: Market,
    channels: &[String],
    url: &str,
    mut next: F,
) -> Result<(), Failure>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Selection, Failure>>,
{
    let request = subscription(market, channels).map_err(|_| Failure::Transport)?;
    if matches!(market, Market::Options) {
        let mut s = state.lock().unwrap();
        reset_option_coverage(&mut s);
    }
    {
        feed(&mut state.lock().unwrap(), market).channels = channels.len();
    }
    let (mut socket, _) = timeout(Duration::from_secs(15), connect_async(url))
        .await
        .map_err(|_| connection_fault(state, market, "connect", "timeout_15s", None))?
        .map_err(|e| socket_fault(state, market, "connect", e))?;
    status(
        state,
        market,
        "authenticating",
        "Socket abierto; esperando autorización",
    );
    socket
        .send(Message::Text(
            serde_json::json!({"action":"auth", "params":key.0})
                .to_string()
                .into(),
        ))
        .await
        .map_err(|e| socket_fault(state, market, "send_auth", e))?;
    let mut authenticated = false;
    let mut received_data = false;
    let auth_deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(20));
    let mut last_message = std::time::Instant::now();
    let mut active_channels = channels.to_vec();
    let mut option_queue=crate::option_time_queue::Queue::default();
    let mut settle=tokio::time::interval(Duration::from_millis(20));
    let mut rotation = Box::pin(next());
    loop {
        tokio::select! {
            _ = settle.tick(), if authenticated && matches!(market,Market::Options) => {
                let now=now_ms();
                let before=option_queue.stats.dropped;
                let ready=option_queue.ready(now);
                for event in &ready {if event_selected(market,&active_channels,event){crate::snapshot_lanes::fanout(state,market,event,now);}}
                let mut accepted=false;
                {
                    let mut s=state.lock().unwrap();
                    if option_queue.stats.dropped!=before {reset_option_coverage(&mut s);}
                    for event in ready {if event_selected(market,&active_channels,&event) {accepted|=apply(&mut s,market,&event,now);}}
                    s.option_timing=option_queue.stats.clone();
                }
                if accepted {received_data=true;status(state,market,"streaming","Eventos asentados recibidos; timestamps originales conservados");}
            }
            result = &mut rotation, if authenticated && matches!(market, Market::Options) => {
                match result {
                    Ok(selection) => {
                        for request in subscription_changes(&active_channels, &selection.channels)? {
                            socket.send(Message::Text(request.into())).await.map_err(|e| socket_fault(state, market, "rotate_subscription", e))?;
                        }
                        active_channels = selection.channels;
                        option_queue.retain(|e|event_selected(market,&active_channels,e));
                        let allowed = contract_symbols(&active_channels);
                        let mut s = state.lock().unwrap();
                        s.ticks.retain(|symbol, _| !symbol.starts_with("O:") || allowed.contains(symbol));
                        s.options.channels = active_channels.len();
                        let changed_expiry = s.selection.expiry != selection.info.expiry;
                        let changed_day = s.selection.updated_ms.and_then(snipers_core::calendar::date_at) != selection.info.updated_ms.and_then(snipers_core::calendar::date_at);
                        s.selection = selection.info;
                        install_option_contracts(&mut s,selection.contracts);
                        s.flow_second = None;
                        if changed_expiry || changed_day { s.snapshot_notify.notify_one(); }
                    }
                    Err(_) => {
                        state.lock().unwrap().selection.error = Some("No se pudo renovar la selección; se conservan los contratos anteriores y se reintentará.".into());
                    }
                }
                rotation = Box::pin(next());
            }
            _ = tokio::time::sleep_until(auth_deadline), if !authenticated => return Err(connection_fault(state, market, "auth", "timeout_20s", None)),
            _ = heartbeat.tick() => {
                if last_message.elapsed() > Duration::from_secs(65) { return Err(connection_fault(state, market, "heartbeat", "no_frames_65s", None)); }
                if authenticated&&matches!(market,Market::Stocks){
                    let wanted=passive_stock_quote_channels(channels,&state.lock().unwrap());
                    for request in subscription_changes_for(Market::Stocks,&active_channels,&wanted)?{
                        socket.send(Message::Text(request.into())).await.map_err(|e|socket_fault(state,market,"research_quotes",e))?;
                    }
                    if wanted!=active_channels{
                        let added:Vec<_>=wanted.iter().filter(|c|!active_channels.contains(c)&&c.starts_with("Q.")).map(|c|c[2..].to_owned()).collect();
                        let mut current=state.lock().unwrap();for symbol in added {current.bot_cvd_engines.entry(symbol).or_default().gap();}current.stocks.channels=wanted.len();
                    }
                    active_channels=wanted;
                }
                socket.send(Message::Ping(Vec::new().into())).await.map_err(|e| socket_fault(state, market, "send_ping", e))?;
            }
            frame = socket.next() => {
                let frame = frame.ok_or_else(|| connection_fault(state, market, "receive", "stream_ended", None))?.map_err(|e| socket_fault(state, market, "receive", e))?;
                last_message = std::time::Instant::now();
                match frame {
                    Message::Text(text) => {
                        // Decode each element independently: new or malformed events cannot discard valid neighbors.
                        let values: Vec<serde_json::Value> = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(_) => { feed(&mut state.lock().unwrap(), market).malformed += 1; continue; }
                        };
                        for value in values {
                            let event: Event = match serde_json::from_value(value) {
                                Ok(v) => v,
                                Err(_) => { feed(&mut state.lock().unwrap(), market).malformed += 1; continue; }
                            };
                            if let Event::Status { status: code, message } = &event {
                                match code.as_str() {
                                    "auth_success" if !authenticated => {
                                        authenticated = true;
                                        if matches!(market, Market::Stocks) { state.lock().unwrap().history_notify.notify_one(); }
                                        socket.send(Message::Text(request.clone().into())).await.map_err(|e| socket_fault(state, market, "subscribe", e))?;
                                        status(state, market, "subscribing", "Autorizado; suscripción enviada, esperando confirmación o datos");
                                    }
                                    "auth_failed" | "not_authorized" => return Err(Failure::Denied),
                                    "max_connections" => return Err(Failure::ConnectionLimit),
                                    "error" => return Err(Failure::Denied),
                                    "success" if authenticated && !received_data && message.to_lowercase().contains("subscribed to:") => {
                                        status(state, market, "waiting", "Suscripción aceptada; aún no se han recibido eventos de mercado");
                                        if matches!(market,Market::Stocks) {state.lock().unwrap().off_exchange_notify.notify_one();}
                                    }
                                    _ => {}
                                }
                            } else if authenticated && event_selected(market, &active_channels, &event) {
                                let now=now_ms();
                                if let Some(capture)=&state.lock().unwrap().background_capture {capture.offer(market,&event,now);}
                                if !matches!(market,Market::Options) || match &event {Event::Trade{t,..}|Event::Quote{t,..}=>*t<=now,_=>true} {crate::snapshot_lanes::fanout(state,market,&event,now);}
                                let accepted=if matches!(market,Market::Options){apply_option_buffered(&mut state.lock().unwrap(),&mut option_queue,event,now)}else{apply(&mut state.lock().unwrap(),market,&event,now)};
                                if !accepted{continue}
                                let first=!received_data;
                                received_data = true;
                                status(state, market, "streaming", "Eventos recibidos; consulta su antigüedad para evaluar frescura");
                                if first && matches!(market,Market::Stocks) {state.lock().unwrap().off_exchange_notify.notify_one();}
                            }
                        }
                    }
                    Message::Ping(payload) => socket.send(Message::Pong(payload)).await.map_err(|e| socket_fault(state, market, "send_pong", e))?,
                    Message::Close(frame) => return Err(connection_fault(state, market, "receive", "server_close", frame.map(|f| i32::from(u16::from(f.code))))),
                    _ => {}
                }
            }
        }
    }
}

fn option_underlying(ticker: &str) -> Option<&str> {
    snipers_core::options::underlying(ticker)
}
pub(crate) fn option_root_for_diagnostic(ticker:&str)->String {option_underlying(ticker).unwrap_or("UNKNOWN").into()}
fn apply_option_buffered(s:&mut LiveState,queue:&mut crate::option_time_queue::Queue,event:Event,received:u64)->bool {
    let timestamp=match &event{Event::Trade{t,..}|Event::Quote{t,..}=>*t,_=>received};
    let before=queue.stats.dropped;
    let ready=queue.ready(timestamp.min(received));
    let immediate=queue.ingest(event,received);
    if queue.stats.dropped!=before {reset_option_coverage(s);}
    let mut accepted=false;
    for event in ready {accepted|=apply(s,Market::Options,&event,received);}
    if let Some(event)=immediate {accepted|=apply(s,Market::Options,&event,received);}
    accepted
}
#[cfg(test)]mod option_buffer_integration_tests {
    use super::*;
    #[test]
    fn clock_lead_does_not_break_one_and_five_minute_coverage(){
        let now=snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let shared=new_state();let mut s=shared.lock().unwrap();s.options.phase="streaming".into();
        s.scanner_options=crate::scanner_options::fixture_pool(now);s.scanner_options.background=true;
        let plan=s.scanner_options.contracts("QQQ",now,1000);s.scanner_options.install(&plan,"QQQ");
        let ticker=plan.keys().next().unwrap().clone();let mut queue=crate::option_time_queue::Queue::default();
        for second in 0..310 {let at=now+second*1000;let events=crate::decode_batch(&serde_json::json!([
            {"ev":"Q","sym":ticker,"bp":1.,"ap":1.1,"bs":10,"as":10,"t":at+650},
            {"ev":"T","sym":ticker,"p":1.1,"s":1,"t":at+700,"q":second+1}
        ]).to_string()).unwrap();for event in events {apply_option_buffered(&mut s,&mut queue,event,at);}for event in queue.ready(at+1000){assert!(apply(&mut s,Market::Options,&event,at+1000));}}
        let data=s.scanner_options.bot_data("SPY",now+310000);
        for minutes in [1,5] {let windows=data["flow"]["contract_windows"].as_array().unwrap();let w=windows.iter().find(|w|w["minutes"]==minutes).unwrap();assert_eq!(w["covered"],true,"{data}");}
        assert_eq!(queue.stats.dropped,0);assert_eq!(queue.stats.released,620);
    }
    #[test]
    fn delayed_background_quotes_and_trades_preserve_classification_and_deduplication(){
        let now=snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let shared=new_state();let mut s=shared.lock().unwrap();s.options.phase="streaming".into();
        s.scanner_options=crate::scanner_options::fixture_pool(now);s.scanner_options.background=true;
        let plan=s.scanner_options.contracts("QQQ",now,1000);s.scanner_options.install(&plan,"QQQ");
        let ticker=plan.keys().next().unwrap().clone();
        let events=crate::decode_batch(&serde_json::json!([
            {"ev":"Q","sym":ticker,"bp":1.,"ap":1.1,"bs":10,"as":10,"t":now+650},
            {"ev":"T","sym":ticker,"p":1.1,"s":1,"t":now+700,"q":123}
        ]).to_string()).unwrap();
        let mut queue=crate::option_time_queue::Queue::default();
        for event in &events {assert!(!apply_option_buffered(&mut s,&mut queue,event.clone(),now));}
        assert!(s.scanner_options.bot_data("SPY",now)["lastEventMs"].is_null());
        for event in queue.ready(now+700){assert!(apply(&mut s,Market::Options,&event,now+700));}
        assert_eq!(s.scanner_options.bot_data("SPY",now+700)["lastEventMs"],now+700);
        let flow=s.scanner_options.bot_data("SPY",now+700);
        assert_eq!(flow["flow"]["intraday_windows"][0]["buckets"]["CALL BUY"]["contracts"],1.);
        assert!(!apply_option_buffered(&mut s,&mut queue,events[1].clone(),now+701));
        assert_eq!(s.scanner_options.bot_data("SPY",now+701)["flow"]["intraday_windows"][0]["buckets"]["CALL BUY"]["contracts"],1.);
    }
}

fn positive(n: f64) -> bool {
    n.is_finite() && n > 0.0
}
pub(crate) fn apply(s: &mut LiveState, market: Market, event: &Event, received: u64) -> bool {
    if matches!(market, Market::Options) {
        if let Event::Trade { sym, t, .. } | Event::Quote { sym, t, .. } = event {
            if option_underlying(sym) != Some(s.symbol.as_str()) {
                if s.spx_engine.accepts(sym) {
                    let valid = s.spx_engine.receive(event, received);
                    if !s.telemetry.observe(market, event, received) || !valid {
                        return false;
                    }
                    s.options.events += 1;
                    s.options.last_received_ms = Some(received);
                    s.options.last_event_ms =
                        Some(s.options.last_event_ms.map_or(*t, |old| old.max(*t)));
                    return true;
                }
                if !s.scanner_options.accepts(sym) {
                    return false;
                }
                let valid = match event {
                    Event::Trade { p, s: size, .. } => {
                        positive(*p) && positive(*size) && *t <= received
                    }
                    Event::Quote { bp, ap, .. } => {
                        positive(*bp) && positive(*ap) && ap >= bp && *t <= received
                    }
                    _ => false,
                };
                if !valid {
                    s.scanner_options.receive(event, received);
                    return false;
                }
                if !s.telemetry.observe(market, event, received) {
                    return false;
                }
                if !s.scanner_options.receive(event, received) {
                    return false;
                }
                s.options.events += 1;
                s.options.last_event_ms = Some(s.options.last_event_ms.unwrap_or(0).max(*t));
                s.options.last_received_ms = Some(received);
                return true;
            }
        }
    }
    let bot_symbol=match event {Event::Quote{sym,..}|Event::Trade{sym,..}=>Some(sym),_=>None};
    if let Some(sym)=bot_symbol.filter(|sym|matches!(market,Market::Stocks)&&(s.bot_targets.contains(sym)||s.bot_held.contains(sym)||s.research_symbols.contains(sym)||s.qqq_components.contains(sym)||sym.as_str()=="QQQ"&&s.direction_enabled)) {
        let engine=s.bot_cvd_engines.entry(sym.clone()).or_default();
        match event {Event::Quote{bp,ap,bs,ask_size,t,..}=>{if *t>received.saturating_add(1000){engine.gap();}else{engine.quote_with_depth(*bp,*ap,Some(*bs),Some(*ask_size),*t);}},Event::Trade{p,s:size,t,q,metadata,..}=>{if let Some(shares)=crate::trade_units::stock_trade_size(*size,metadata.ds.as_deref()){engine.trade(sym,*p,shares,*t,*q,&metadata.c,received);}},_=>{}}
    }
    if s.etf_scanner_enabled && matches!(market, Market::Stocks) {
        match event {
            Event::Quote {sym,bp,ap,bs,ask_size,t,..} if matches!(sym.as_str(),"SPY"|"QQQ") => {
                let engine=s.etf_cvd_engines.entry(sym.clone()).or_insert_with(snipers_core::cvd::Engine::analysis);
                if *t>received.saturating_add(1000){engine.gap();}else{engine.quote_with_depth(*bp,*ap,Some(*bs),Some(*ask_size),*t);}
                if *t<=received && s.etf_block_quotes.get(sym).is_none_or(|q|q.timestamp_ms<=*t) {
                    let quote=snipers_core::off_exchange::Quote{bid:*bp,ask:*ap,timestamp_ms:*t};
                    s.etf_block_quotes.insert(sym.clone(),quote);
                }
            },
            Event::Trade {sym,p,s:size,t,q,metadata} if matches!(sym.as_str(),"SPY"|"QQQ") => {
                if let Some(shares)=crate::trade_units::stock_trade_size(*size,metadata.ds.as_deref()){s.etf_cvd_engines.entry(sym.clone()).or_insert_with(snipers_core::cvd::Engine::analysis).trade(sym,*p,shares,*t,*q,&metadata.c,received);}
                if *t<=received && received-*t<=15000 && p.is_finite() && *p>0. {s.scanner_options.reference(sym,*p,*t,received);}
                if let Ok(value)=serde_json::to_value(event) {
                    if let Ok(Some(trade))=snipers_core::off_exchange_trade::normalize(&value,sym,snipers_core::off_exchange_trade::Source::Live) {
                        // Only block-sized rows and corrections are retained by this live-only sample.
                        if trade.price*trade.shares>=100_000. || trade.correction.is_some_and(|c|c!=0) {
                            let quote=s.etf_block_quotes.get(sym).copied();
                            let at=snipers_core::block_pressure::execution(&trade);
                            let engine=s.etf_block_engines.entry(sym.clone()).or_insert_with(||snipers_core::off_exchange_tape::Engine::new(sym).unwrap());
                            if let Some((at,quote))=at.zip(quote){engine.block_quote(at,quote);}
                            engine.trade(trade,received);
                        }
                    }
                }
            },
            _=>(),
        }
    }
    if matches!(market, Market::Stocks) {
        let accepted = s.scanner_store.receive(event, received);
        if let Event::MinuteBar { sym, e, .. } = event {
            if sym != &s.symbol {
                if accepted {
                    s.stocks.events += 1;
                    s.stocks.last_event_ms = Some(s.stocks.last_event_ms.unwrap_or(0).max(*e));
                    s.stocks.last_received_ms = Some(received);
                }
                return accepted;
            }
        }
    }
    if matches!(market, Market::Stocks) {
        if let Event::Quote { sym, bp, ap, bs, ask_size, t, .. } = event {
            if sym == &s.symbol {
                if *t > received.saturating_add(1000) {s.cvd_engine.gap();}
                else {s.cvd_engine.quote_with_depth(*bp,*ap,Some(*bs),Some(*ask_size),*t);}
            }
        }
        let eligible = match event {
            Event::Quote { sym, .. } => sym == &s.symbol,
            Event::Trade { sym, metadata, .. } => sym == &s.symbol && metadata.x == Some(4),
            _ => false,
        };
        // Independent tape sees invalid NBBO and identity conflicts before generic filtering.
        if eligible {
            if let Some(tx) = &s.off_exchange_tx {
                if tx
                    .try_send(crate::off_exchange::Input::Event(event.clone(), received))
                    .is_err()
                {
                    s.off_exchange.queue_drops += 1;
                    s.off_exchange.gap(received);
                }
            }
        }
    }
    if matches!(market, Market::Options) {
        if let Event::Trade { p, s: size, t, .. } = event {
            if !positive(*p) || !positive(*size) || *t > received.saturating_add(1_000) {
                s.flow_engine.invalidate_coverage();
                s.flow_second = None;
            }
        }
        if let Event::Quote {
            sym,
            bp,
            ap,
            t,
            bs,
            ask_size,
        } = event
        {
            if *t > received.saturating_add(1_000) {
                s.flow_engine.quote(sym, 0.0, 0.0, received);
                s.flow_engine.invalidate_coverage();
                s.flow_second = None;
                s.telemetry.observe(market, event, received);
                return false;
            }
            // Invalid NBBO must invalidate flow inference even if excluded from the price display.
            s.flow_engine
                .quote_with_depth(sym, *bp, *ap, *t, Some(*bs), Some(*ask_size));
        }
    }
    let (sym, t) = match event {
        Event::Trade { sym, p, t, .. } if positive(*p) => (sym, *t),
        Event::Quote { sym, bp, ap, t, .. } if positive(*bp) && positive(*ap) && ap >= bp => {
            (sym, *t)
        }
        Event::SecondBar {
            sym,
            o,
            h,
            l,
            c,
            v,
            e,
            ..
        }
        | Event::MinuteBar {
            sym,
            o,
            h,
            l,
            c,
            v,
            e,
            ..
        } if [*o, *h, *l, *c].into_iter().all(positive)
            && h >= &o.max(*c)
            && l <= &o.min(*c)
            && v.is_finite()
            && *v >= 0.0 =>
        {
            (sym, *e)
        }
        _ => return false,
    };
    if matches!(market,Market::Stocks) {if let Event::Trade{p,..}=event {s.price_alert_engine.trade(sym,*p,t,received);}}
    // Keep independent scanner prices even before Paper is connected. Raw source time
    // remains intact: only display validation permits a bounded clock difference.
    if matches!(market,Market::Stocks) && snipers_core::universe::stock_asset(sym).is_some() {
        if let Event::Trade{p,q,..}=event {
            if t<=received.saturating_add(1000) && received.saturating_sub(t)<=15000 && s.bot_ticks.get(sym).and_then(|v|v.trade_ms).is_none_or(|old|t>old) {
                s.bot_ticks.insert(sym.clone(),Tick{symbol:sym.clone(),price:Some(*p),trade_ms:Some(t),trade_sequence:*q,bid:None,ask:None,quote_ms:None});
            }
        }
    }
    if matches!(market,Market::Stocks) && (s.bot_enabled || !s.bot_held.is_empty()) && snipers_core::universe::stock_asset(sym).is_some() {
        if let Event::Trade{p,q,..}=event {
            if t<=received && received-t<=5000 && s.bot_ticks.get(sym).and_then(|v|v.trade_ms).is_none_or(|old|t>old) {
                s.bot_ticks.insert(sym.clone(),Tick{symbol:sym.clone(),price:Some(*p),trade_ms:Some(t),trade_sequence:*q,bid:None,ask:None,quote_ms:None});
            }
            if sym!=&s.symbol && s.held_symbol.as_ref()!=Some(sym) && !s.legacy_symbols.contains(sym) {return t<=received && received-t<=5000;}
        }
    }
    // A held position has its own trade-only lane. It must never alter the
    // selected asset's candles, flow, NBBO or structural calculations.
    if matches!(market, Market::Stocks)
        && sym != &s.symbol
        && (s.held_symbol.as_ref() == Some(sym) || s.legacy_symbols.contains(sym))
    {
        let Event::Trade { p, q, .. } = event else {
            return false;
        };
        if t > received || received - t > 15_000 || !s.telemetry.observe(market, event, received) {
            return false;
        }
        if s.legacy_ticks
            .get(sym)
            .or(s.held_tick.as_ref().filter(|tick| &tick.symbol == sym))
            .and_then(|tick| tick.trade_ms)
            .is_some_and(|old| t <= old)
        {
            return false;
        }
        let tick = Tick {
            symbol: sym.clone(),
            price: Some(*p),
            bid: None,
            ask: None,
            trade_ms: Some(t),
            trade_sequence: *q,
            quote_ms: None,
        };
        if s.held_symbol.as_ref() == Some(sym) {
            s.held_tick = Some(tick.clone());
        }
        if s.legacy_symbols.contains(sym) {
            s.legacy_ticks.insert(sym.clone(), tick);
        }
        s.stocks.events += 1;
        s.stocks.last_event_ms = Some(s.stocks.last_event_ms.unwrap_or(0).max(t));
        s.stocks.last_received_ms = Some(received);
        return true;
    }
    // Feed ownership bounds the state even if unexpected symbols are sent.
    if matches!(market, Market::Stocks) && sym != &s.symbol {
        return false;
    }
    if matches!(market, Market::Options)
        && (option_underlying(sym) != Some(s.symbol.as_str())
            || s.ticks.len() >= 1_001 && !s.ticks.contains_key(sym))
    {
        return false;
    }
    let auction_conflicts=s.telemetry.quality(market).sequence_conflicts;
    if !s.telemetry.observe(market, event, received) {
        if matches!(market,Market::Stocks)&&s.telemetry.quality(market).sequence_conflicts>auction_conflicts {s.auction_engine.gap();}
        return false;
    }
    if matches!(market, Market::Stocks) {
        if let Event::Trade{sym,p,s:size,t,q,metadata}=event {
            if let Some(shares)=crate::trade_units::stock_trade_size(*size,metadata.ds.as_deref()){
                s.auction_engine.trade(sym,*p,shares,*t,*q,&metadata.c,received);
                s.cvd_engine.trade(sym,*p,shares,*t,*q,&metadata.c,received);
            }
        }
    }
    if matches!(market,Market::Options) && s.scanner_options.background {s.scanner_options.receive(event,received);}
    if matches!(market, Market::Options) {
        if let Event::Trade {
            sym, p, s: size, t, ..
        } = event
        {
            s.flow_engine.trade(sym, *p, *size, *t, received);
        }
    }
    let tick = s.ticks.entry(sym.clone()).or_insert_with(|| Tick {
        symbol: sym.clone(),
        price: None,
        bid: None,
        ask: None,
        trade_ms: None,
        trade_sequence: None,
        quote_ms: None,
    });
    match event {
        Event::MinuteBar {
            o,
            h,
            l,
            c,
            v,
            s: start,
            vw,
            ..
        } if matches!(market, Market::Stocks) && start % 60_000 == 0 => {
            s.minute_revision += 1;
            s.minute_versions.insert(*start, s.minute_revision);
            s.minutes.insert(
                *start,
                Bar {
                    timestamp_ms: *start,
                    open: *o,
                    high: *h,
                    low: *l,
                    close: *c,
                    volume: *v,
                    vwap: vw.filter(|v| v.is_finite()),
                },
            );
            crate::history::trim(s);
        }
        Event::Trade { p, q, .. }
            if tick.trade_ms.is_none_or(|old| {
                t > old
                    || t == old
                        && q.zip(tick.trade_sequence)
                            .is_none_or(|(new, old)| new >= old)
            }) =>
        {
            tick.price = Some(*p);
            tick.trade_ms = Some(t);
            tick.trade_sequence = *q;
        }
        Event::Quote { bp, ap, .. } if tick.quote_ms.is_none_or(|old| t >= old) => {
            tick.bid = Some(*bp);
            tick.ask = Some(*ap);
            tick.quote_ms = Some(t);
        }
        Event::SecondBar {
            o,
            h,
            l,
            c,
            v,
            s: start,
            vw,
            ..
        } if matches!(market, Market::Stocks) => {
            s.bars.insert(
                *start,
                Bar {
                    timestamp_ms: *start,
                    open: *o,
                    high: *h,
                    low: *l,
                    close: *c,
                    volume: *v,
                    vwap: vw.filter(|v| v.is_finite()),
                },
            );
            while s.bars.len() > 1800 {
                s.bars.pop_first();
            }
        }
        _ => {}
    }
    let f = feed(s, market);
    f.events += 1;
    f.last_event_ms = Some(f.last_event_ms.unwrap_or(0).max(t));
    f.last_received_ms = Some(received);
    true
}

async fn get(
    client: &reqwest::Client,
    key: &Credentials,
    path: &str,
    query: &[(&str, String)],
) -> Result<serde_json::Value, Failure> {
    let response = client
        .get(format!("https://api.massive.com{path}"))
        .bearer_auth(&key.0)
        .query(query)
        .send()
        .await
        .map_err(|e| {
            Failure::Diagnostic(
                "rest_discovery",
                if e.is_timeout() {
                    "timeout"
                } else if e.is_connect() {
                    "connect"
                } else {
                    "request"
                },
                None,
            )
        })?;
    if [401, 403].contains(&response.status().as_u16()) {
        return Err(Failure::Denied);
    }
    if !response.status().is_success() {
        return Err(Failure::Diagnostic(
            "rest_discovery",
            "http_status",
            Some(i32::from(response.status().as_u16())),
        ));
    }
    response
        .json()
        .await
        .map_err(|_| Failure::Diagnostic("rest_discovery", "invalid_json", None))
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionState {
    pub structural_requested: usize,
    pub structural_resolved: usize,
    pub spot: Option<f64>,
    pub source: String,
    pub expiry: String,
    pub updated_ms: Option<u64>,
    pub error: Option<String>,
}
struct Selection {
    channels: Vec<String>,
    info: SelectionState,
    contracts: BTreeMap<String, snipers_core::flow::Contract>,
}

fn current_spot(state: &SharedState, now: u64) -> Option<f64> {
    let s = state.lock().unwrap();
    let tick = s.ticks.get(&s.symbol)?;
    let fresh = |t: Option<u64>| t.is_some_and(|t| t <= now && now - t <= 120_000);
    if fresh(tick.trade_ms) && tick.price.is_some_and(positive) {
        tick.price
    } else if fresh(tick.quote_ms) {
        tick.bid
            .zip(tick.ask)
            .filter(|(b, a)| positive(*b) && positive(*a) && a >= b)
            .map(|(b, a)| (b + a) / 2.0)
    } else {
        None
    }
}

fn contract_symbols(channels: &[String]) -> BTreeSet<String> {
    channels
        .iter()
        .filter_map(|c| c.split_once('.').map(|(_, s)| s.to_owned()))
        .collect()
}
fn event_selected(market: Market, channels: &[String], event: &Event) -> bool {
    if matches!(market, Market::Stocks) {
        return true;
    }
    let (kind, sym) = match event {
        Event::Trade { sym, .. } => ("T", sym),
        Event::Quote { sym, .. } => ("Q", sym),
        _ => return false,
    };
    channels
        .iter()
        .any(|c| c.strip_prefix(kind).and_then(|c| c.strip_prefix('.')) == Some(sym.as_str()))
}
fn subscription_changes(old: &[String], new: &[String]) -> Result<Vec<String>, Failure> {
    subscription_changes_for(Market::Options, old, new)
}
fn subscription_changes_for(market: Market, old: &[String], new: &[String]) -> Result<Vec<String>, Failure> {
    subscription(market, new).map_err(|_| Failure::Transport)?;
    let old: BTreeSet<_> = old.iter().cloned().collect();
    let new: BTreeSet<_> = new.iter().cloned().collect();
    let removed: Vec<_> = old.difference(&new).cloned().collect();
    let added: Vec<_> = new.difference(&old).cloned().collect();
    let mut requests = Vec::new();
    // Remove first so the transition cannot exceed the quote subscription cap.
    if !removed.is_empty() {
        requests.push(
            serde_json::json!({"action":"unsubscribe","params":removed.join(",")}).to_string(),
        );
    }
    if !added.is_empty() {
        requests.push(subscription(market, &added).map_err(|_| Failure::Transport)?);
    }
    Ok(requests)
}
async fn next_selection(key: &Credentials, state: &SharedState) -> Result<Selection, Failure> {
    // Reference refresh is independent of tick delivery; never block socket reads on REST.
    let notify = state.lock().unwrap().scanner_options_notify.clone();
    tokio::select! { _=sleep(Duration::from_secs(60))=>{}, _=notify.notified()=>{} }
    let symbol = state.lock().unwrap().symbol.clone();
    let mut selection = discover(key, &symbol, current_spot(state, now_ms())).await?;
    extend_structural_selection(&mut selection, &state.lock().unwrap(), now_ms())?;
    extend_pool_selection(&mut selection, &state.lock().unwrap(), now_ms())?;
    Ok(selection)
}

fn install_option_contracts(
    s: &mut LiveState,
    contracts: BTreeMap<String, snipers_core::flow::Contract>,
) {
    for (symbol,lane) in &s.snapshot_lanes {
        let own: BTreeMap<_,_>=contracts.iter().filter(|(ticker,_)|option_underlying(ticker)==Some(symbol.as_str())).map(|(k,v)|(k.clone(),v.clone())).collect();
        let mut target=lane.lock().unwrap();
        target.selection.structural_resolved=own.len();
        target.selection.expiry=own.values().map(|c|c.expiration.clone()).min().unwrap_or_default();
        target.selection.updated_ms=Some(now_ms());
        target.flow_engine.set_contracts(own);
    }
    s.spx_engine.install(&contracts);
    s.scanner_options.install(&contracts, &s.symbol);
    s.flow_engine.set_contracts(
        contracts
            .into_iter()
            .filter(|(ticker, _)| option_underlying(ticker) == Some(s.symbol.as_str()))
            .collect(),
    );
}
fn extend_pool_selection(
    selection: &mut Selection,
    state: &LiveState,
    now: u64,
) -> Result<(), Failure> {
    let extra = state.scanner_options.contracts(
        &state.symbol,
        now,
        1000usize.saturating_sub(selection.contracts.len()),
    );
    for (ticker, contract) in extra {
        if !selection.contracts.contains_key(&ticker) {
            selection.channels.push(format!("T.{ticker}"));
            selection.channels.push(format!("Q.{ticker}"));
            selection.contracts.insert(ticker, contract);
        }
    }
    let spx = state
        .spx_engine
        .contracts(now, 1000usize.saturating_sub(selection.contracts.len()));
    for (ticker, contract) in spx {
        if !selection.contracts.contains_key(&ticker) {
            selection.channels.push(format!("Q.{ticker}"));
            selection.contracts.insert(ticker, contract);
        }
    }
    subscription(Market::Options, &selection.channels).map_err(|_| Failure::Transport)?;
    Ok(())
}

fn extend_structural_selection(
    selection: &mut Selection,
    state: &LiveState,
    now: u64,
) -> Result<(), Failure> {
    let Some(observation) = state.option_snapshot.as_ref() else {
        return Ok(());
    };
    let mut requested = BTreeSet::new();
    for h in state.option_snapshot_status.horizons.iter().take(3) {
        if !crate::interpretation::structure_current(state, h, now) {
            continue;
        }
        for spec in snipers_core::wall_flow::structural_specs(&h.structure) {
            for expiry in &h.expirations {
                requested.insert((spec.kind.clone(), spec.strike.to_bits(), expiry.clone()));
            }
        }
    }
    selection.info.structural_requested = requested.len();
    for (kind, strike, expiration) in requested {
        let candidates: Vec<_> = observation
            .contracts
            .values()
            .filter(|c| {
                c.kind == kind
                    && c.strike.to_bits() == strike
                    && c.expiration == expiration
                    && option_underlying(&c.ticker) == Some(state.symbol.as_str())
            })
            .collect();
        if candidates.len() != 1 {
            continue;
        }
        let c = candidates[0];
        selection.info.structural_resolved += 1;
        selection
            .contracts
            .entry(c.ticker.clone())
            .or_insert_with(|| snipers_core::flow::Contract {
                ticker: c.ticker.clone(),
                kind: c.kind.clone(),
                strike: c.strike,
                expiration: c.expiration.clone(),
                shares_per_contract: c.shares_per_contract,
                premium_multiplier: c.premium_multiplier,
            });
    }
    selection.channels = selection
        .contracts
        .keys()
        .flat_map(|ticker| [format!("T.{ticker}"), format!("Q.{ticker}")])
        .collect();
    if !selection.channels.is_empty() {
        subscription(Market::Options, &selection.channels).map_err(|error| match error {
            crate::SubscriptionError::QuoteLimit => Failure::Limit,
            crate::SubscriptionError::Invalid => Failure::Transport,
        })?;
    }
    Ok(())
}

fn expiration_floor(now: u64) -> Result<String, Failure> {
    snipers_core::calendar::date_at(now)
        .map(|date| date.to_string())
        .ok_or(Failure::Transport)
}

async fn discover(
    key: &Credentials,
    symbol: &str,
    live_spot: Option<f64>,
) -> Result<Selection, Failure> {
    let range = snipers_core::universe::stock_asset(symbol)
        .ok_or(Failure::Denied)?
        .live_range;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| Failure::Transport)?;
    let spot = if let Some(spot) = live_spot.filter(|v| positive(*v)) {
        spot
    } else {
        let previous = get(
            &client,
            key,
            &format!("/v2/aggs/ticker/{symbol}/prev"),
            &[("adjusted", "true".into())],
        )
        .await?;
        previous["results"][0]["c"]
            .as_f64()
            .filter(|v| positive(*v))
            .ok_or(Failure::Transport)?
    };
    let today = expiration_floor(now_ms())?;
    let nearest = get(
        &client,
        key,
        "/v3/reference/options/contracts",
        &[
            ("underlying_ticker", symbol.into()),
            ("expired", "false".into()),
            ("expiration_date.gte", today.clone()),
            ("sort", "expiration_date".into()),
            ("order", "asc".into()),
            ("limit", "1".into()),
        ],
    )
    .await?;
    let expiry = nearest["results"][0]["expiration_date"]
        .as_str()
        .ok_or(Failure::Transport)?;
    if expiry < today.as_str() {
        return Err(Failure::Transport);
    }
    let data = get(
        &client,
        key,
        "/v3/reference/options/contracts",
        &[
            ("underlying_ticker", symbol.into()),
            ("expiration_date", expiry.into()),
            ("strike_price.gte", (spot - range).to_string()),
            ("strike_price.lte", (spot + range).to_string()),
            ("limit", "1000".into()),
        ],
    )
    .await?;
    // Never silently truncate contract pages. This narrow range must fit one page.
    if data["next_url"].is_string() {
        return Err(Failure::Transport);
    }
    let channels = select_contracts(
        data["results"].as_array().ok_or(Failure::Transport)?,
        spot,
        symbol,
    )?;
    let contracts = contract_metadata(data["results"].as_array().unwrap(), &channels, symbol);
    if contracts.len() != channels.len() / 2 {
        return Err(Failure::Transport);
    }
    Ok(Selection {
        contracts,
        channels,
        info: SelectionState {
            structural_requested: 0,
            structural_resolved: 0,
            spot: Some(spot),
            source: if live_spot.is_some() {
                "Precio WebSocket reciente"
            } else {
                "Cierre anterior REST (sin spot reciente)"
            }
            .into(),
            expiry: expiry.into(),
            updated_ms: Some(now_ms()),
            error: None,
        },
    })
}

fn contract_metadata(
    rows: &[serde_json::Value],
    channels: &[String],
    symbol: &str,
) -> BTreeMap<String, snipers_core::flow::Contract> {
    let selected = contract_symbols(channels);
    rows.iter()
        .filter_map(|row| {
            let ticker = row["ticker"].as_str()?;
            if !selected.contains(ticker) || row["underlying_ticker"] != symbol {
                return None;
            }
            let shares = row["shares_per_contract"].as_f64().filter(|v| positive(*v));
            let no_extra = row["additional_underlyings"].is_null()
                || row["additional_underlyings"]
                    .as_array()
                    .is_some_and(|a| a.is_empty());
            let standard =
                shares == Some(100.0) && no_extra && option_underlying(ticker) == Some(symbol);
            Some((
                ticker.into(),
                snipers_core::flow::Contract {
                    ticker: ticker.into(),
                    kind: row["contract_type"].as_str()?.into(),
                    strike: row["strike_price"].as_f64()?,
                    expiration: row["expiration_date"].as_str()?.into(),
                    shares_per_contract: shares,
                    premium_multiplier: standard.then_some(100.0),
                },
            ))
        })
        .collect()
}

/// Read-only REST check, safe while the legacy WebSockets are still connected.
/// Success proves REST access and contract discovery, not WebSocket entitlements.
pub async fn verify_rest(key: &Credentials) -> Result<usize, &'static str> {
    discover(key, "QQQ", None)
        .await
        .map(|selection| selection.channels.len() / 2)
        .map_err(|error| match error {
            Failure::Denied => "Massive rechazó el acceso REST (401/403).",
            _ => "No se completó la consulta REST o no se encontraron contratos en el rango.",
        })
}

fn select_contracts(
    rows: &[serde_json::Value],
    spot: f64,
    symbol: &str,
) -> Result<Vec<String>, Failure> {
    let mut channels = Vec::new();
    for kind in ["call", "put"] {
        let mut contracts: Vec<_> = rows
            .iter()
            .filter(|r| {
                r["contract_type"] == kind
                    && r["strike_price"].as_f64().is_some_and(positive)
                    && r["ticker"]
                        .as_str()
                        .is_some_and(|s| option_underlying(s) == Some(symbol))
            })
            .collect();
        contracts.sort_by(|a, b| {
            (a["strike_price"].as_f64().unwrap() - spot)
                .abs()
                .total_cmp(&(b["strike_price"].as_f64().unwrap() - spot).abs())
        });
        for row in contracts.into_iter().take(6) {
            let ticker = row["ticker"].as_str().unwrap();
            channels.push(format!("T.{ticker}"));
            channels.push(format!("Q.{ticker}"));
        }
    }
    if channels.is_empty() {
        return Err(Failure::Transport);
    }
    Ok(channels)
}

#[cfg(test)]
mod tests {
    #[test]
    fn passive_quotes_follow_watchlist_without_losing_selected_or_etf_channels(){
        let base=vec!["T.QQQ".into(),"Q.QQQ".into(),"Q.SPY".into(),"AM.AAPL".into()];
        let mut state=LiveState{research_symbols:vec!["AAPL".into(),"NVDA".into(),"AAPL".into()],..Default::default()};
        let first=passive_stock_quote_channels(&base,&state);
        assert!(first.contains(&"Q.AAPL".into()));assert!(first.contains(&"Q.NVDA".into()));assert_eq!(first.iter().filter(|c|*c=="Q.AAPL").count(),1);
        state.research_symbols=vec!["AMZN".into()];let next=passive_stock_quote_channels(&base,&state);
        assert!(!next.contains(&"Q.AAPL".into()));assert!(next.contains(&"Q.AMZN".into()));for channel in &base{assert!(next.contains(channel));}
        assert_eq!(subscription_changes_for(Market::Stocks,&first,&next).unwrap().len(),2);assert!(!state.bot_enabled);
    }
    #[test]
    fn accepted_ticks_publish_live_auction_and_conflicts_disconnect_and_staleness_revoke_it(){
        let t=snipers_core::calendar::legacy_eastern_ms("2026-10-09 10:00:00").unwrap();
        let mut s=LiveState{symbol:"QQQ".into(),..Default::default()};s.stocks.phase="streaming".into();
        let trade:Event=serde_json::from_value(serde_json::json!({"ev":"T","sym":"QQQ","p":100.,"s":20,"t":t,"q":1,"c":[0]})).unwrap();
        assert!(apply(&mut s,Market::Stocks,&trade,t));assert!(!apply(&mut s,Market::Stocks,&trade,t));
        let shared=Arc::new(Mutex::new(s));let published=snapshot_at(&shared,t);
        let r=published.auction_map.unwrap();assert_eq!(r["version"],"auction-map-ticks-v1");assert_eq!(r["totalVolume"],20.);assert_eq!(r["provisional"],true);
        assert!(serde_json::to_value(snapshot_at(&shared,t)).unwrap()["auctionMap"].is_object());
        assert!(snapshot_at(&shared,t+15001).auction_map.is_none());
        let changed:Event=serde_json::from_value(serde_json::json!({"ev":"T","sym":"QQQ","p":101.,"s":20,"t":t,"q":1,"c":[0]})).unwrap();
        assert!(!apply(&mut shared.lock().unwrap(),Market::Stocks,&changed,t));
        assert!(snapshot_at(&shared,t).auction_map.is_none());
        status(&shared,Market::Stocks,"connecting","test reconnect");
        assert!(snapshot_at(&shared,t).auction_map.is_none());
    }
    #[test]
    fn qqq_cvd_is_available_for_background_control_without_arming_bot(){
        let t=snipers_core::calendar::legacy_eastern_ms("2026-10-09 10:00:00").unwrap();
        let mut s=LiveState{symbol:"AAPL".into(),direction_enabled:true,..Default::default()};
        let quote=Event::Quote{sym:"QQQ".into(),bp:99.99,ap:100.,bs:10.,ask_size:10.,t};
        let trade:Event=serde_json::from_value(serde_json::json!({"ev":"T","sym":"QQQ","p":100.,"s":20,"t":t,"q":1,"c":[0]})).unwrap();
        apply(&mut s,Market::Stocks,&quote,t);apply(&mut s,Market::Stocks,&trade,t);
        assert_eq!(s.bot_cvd_engines["QQQ"].snapshot("QQQ",t,true).regular.delta,20.);
        assert!(!s.bot_enabled);assert_eq!(s.symbol,"AAPL");assert!(s.auction_engine.snapshot("AAPL",t,true,vec![]).is_none());
    }
    #[test]
    fn passive_research_cvd_retains_fractional_shares_and_other_asset_isolation(){
        let t=snipers_core::calendar::legacy_eastern_ms("2026-10-09 10:00:00").unwrap();
        let mut s=LiveState{symbol:"AAPL".into(),research_symbols:vec!["NBIS".into()],..Default::default()};
        let quote=Event::Quote{sym:"NBIS".into(),bp:99.99,ap:100.,bs:10.,ask_size:10.,t};
        let trade:Event=serde_json::from_value(serde_json::json!({"ev":"T","sym":"NBIS","p":100.,"s":3,"ds":"3.5","t":t,"q":1,"c":[0]})).unwrap();
        apply(&mut s,Market::Stocks,&quote,t);apply(&mut s,Market::Stocks,&trade,t);
        assert_eq!(s.bot_cvd_engines["NBIS"].snapshot("NBIS",t,true).regular.delta,3.5);
        assert!(!s.bot_enabled);assert_eq!(s.symbol,"AAPL");assert!(!s.bot_cvd_engines.contains_key("AAPL"));
    }
    #[test]
    fn execution_evidence_reaches_selected_bot_and_etf_cvd_without_signals(){
        let t=snipers_core::calendar::legacy_eastern_ms("2026-10-02 10:00:00").unwrap();
        let mut s=LiveState{symbol:"QQQ".into(),bot_targets:vec!["QQQ".into()],..Default::default()};s.set_etf_scanner_enabled(true);
        let quote=Event::Quote{sym:"QQQ".into(),bp:99.99,ap:100.,bs:300.,ask_size:700.,t};
        let trade:Event=serde_json::from_value(serde_json::json!({"ev":"T","sym":"QQQ","p":100.,"s":2000,"t":t,"q":1,"x":4,"i":"proof","c":[0],"pt":t})).unwrap();
        apply(&mut s,Market::Stocks,&quote,t);apply(&mut s,Market::Stocks,&trade,t);apply(&mut s,Market::Stocks,&trade,t);
        for r in [s.cvd_engine.snapshot("QQQ",t,true),s.bot_cvd_engines["QQQ"].snapshot("QQQ",t,true),s.etf_cvd_engines["QQQ"].snapshot("QQQ",t,true)]{assert_eq!(r.regular.delta,2000.);assert_eq!(r.execution_evidence.windows[0].trades,1);assert_eq!(r.execution_evidence.recent_trades[0].quote.as_ref().unwrap().ask_shares,Some(700.));}
    }

    #[test]
    fn etf_scanner_collects_only_while_enabled(){
        let t=snipers_core::calendar::legacy_eastern_ms("2026-10-02 10:00:00").unwrap();
        let mut s=LiveState {symbol:"NVDA".into(),..Default::default()};
        let quote=Event::Quote{sym:"SPY".into(),bp:99.,ap:100.,bs:10.,ask_size:10.,t};
        apply(&mut s,Market::Stocks,&quote,t);assert!(s.etf_cvd_engines.is_empty());
        s.set_etf_scanner_enabled(true);apply(&mut s,Market::Stocks,&quote,t);assert!(s.etf_cvd_engines.contains_key("SPY"));
        s.set_etf_scanner_enabled(false);assert!(s.etf_cvd_engines.is_empty());assert!(s.etf_block_quotes.is_empty());
        apply(&mut s,Market::Stocks,&quote,t);assert!(s.etf_cvd_engines.is_empty());
        let state=Arc::new(Mutex::new(s));let frame=snapshot(&state);assert!(frame.etf_cvd.is_empty());assert!(frame.etf_flow.is_empty());assert!(frame.etf_blocks.is_empty());
    }
    #[test]
    fn etf_blocks_are_separate_deduplicated_partial_and_reset_on_reconnect(){
        let t=snipers_core::calendar::legacy_eastern_ms("2026-10-02 10:00:00").unwrap();
        let mut s=LiveState{symbol:"NVDA".into(),..Default::default()};
        s.set_etf_scanner_enabled(true);
        for (symbol,price) in [("SPY",100.),("QQQ",99.)]{
            let quote=Event::Quote{sym:symbol.into(),bp:99.,ap:100.,bs:10.,ask_size:10.,t};
            let trade:Event=serde_json::from_value(serde_json::json!({"ev":"T","sym":symbol,"p":price,"s":2000,"t":t,"q":1,"x":4,"trfi":1,"i":"test","c":[0],"pt":t})).unwrap();
            apply(&mut s,Market::Stocks,&quote,t);apply(&mut s,Market::Stocks,&trade,t);apply(&mut s,Market::Stocks,&trade,t);
        }
        let spy=s.etf_block_engines.get_mut("SPY").unwrap().snapshot(t).blocks;
        let qqq=s.etf_block_engines.get_mut("QQQ").unwrap().snapshot(t).blocks;
        assert_eq!(spy.regular.count,1);assert_eq!(qqq.regular.count,1);
        assert_eq!(spy.regular.buy,200_000.);assert_eq!(qqq.regular.sell,198_000.);
        assert!(!spy.regular.complete);assert!(!qqq.regular.complete);
        let state=Arc::new(Mutex::new(s));status(&state,Market::Stocks,"connecting","reconnect");
        assert!(state.lock().unwrap().etf_block_engines.is_empty());
    }
    #[test]
    fn cvd_stream_rejects_duplicate_trades_and_invalidates_crossed_nbbo() {
        let t=snipers_core::calendar::legacy_eastern_ms("2026-10-02 10:00:00").unwrap();
        let mut s=LiveState {symbol:"NVDA".into(),..Default::default()};
        let quote=Event::Quote {sym:"NVDA".into(),bp:99.,ap:100.,bs:10.,ask_size:10.,t};
        let trade=Event::Trade {sym:"NVDA".into(),p:100.,s:10.,t,q:Some(1),metadata:Default::default()};
        assert!(apply(&mut s,Market::Stocks,&quote,t));assert!(apply(&mut s,Market::Stocks,&trade,t));
        assert!(!apply(&mut s,Market::Stocks,&trade,t));
        let crossed=Event::Quote {sym:"NVDA".into(),bp:101.,ap:100.,bs:10.,ask_size:10.,t:t+1};
        assert!(!apply(&mut s,Market::Stocks,&crossed,t+1));
        let next=Event::Trade {sym:"NVDA".into(),p:100.,s:5.,t:t+2,q:Some(2),metadata:Default::default()};
        assert!(apply(&mut s,Market::Stocks,&next,t+2));
        let r=s.cvd_engine.snapshot("NVDA",t+2,true);assert_eq!(r.regular.delta,10.);assert_eq!(r.regular.unknown,5.);
    }
    #[test]
    fn etf_cvd_is_independent_of_selected_asset_and_other_etf() {
        let t=snipers_core::calendar::legacy_eastern_ms("2026-10-02 10:00:00").unwrap();
        let mut s=LiveState {symbol:"NVDA".into(),..Default::default()};
        s.set_etf_scanner_enabled(true);
        for (symbol,price,size) in [("SPY",100.,10.),("QQQ",99.,25.)] {
            let quote=Event::Quote{sym:symbol.into(),bp:99.,ap:100.,bs:10.,ask_size:10.,t};
            let trade=Event::Trade{sym:symbol.into(),p:price,s:size,t,q:Some(1),metadata:Default::default()};
            apply(&mut s,Market::Stocks,&quote,t);
            apply(&mut s,Market::Stocks,&trade,t);
            apply(&mut s,Market::Stocks,&trade,t);
        }
        assert_eq!(s.etf_cvd_engines["SPY"].snapshot("SPY",t,true).regular.delta,10.);
        assert_eq!(s.etf_cvd_engines["QQQ"].snapshot("QQQ",t,true).regular.delta,-25.);
        assert_eq!(s.cvd_engine.snapshot("NVDA",t,true).regular.delta,0.);
        let state=Arc::new(Mutex::new(s));
        status(&state,Market::Stocks,"connecting","reconnecting");
        let s=state.lock().unwrap();
        assert_eq!(s.etf_cvd_engines["SPY"].snapshot("SPY",t,true).regular.delta,0.);
        assert_eq!(s.etf_cvd_engines["QQQ"].snapshot("QQQ",t,true).regular.delta,0.);
    }
    #[tokio::test]
    async fn connection_slot_retries_recover_after_asset_switch() {
        let state = new_state();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let result = timeout(
            Duration::from_secs(3),
            supervise_sessions(&state, Market::Options, || {
                let attempt = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let state = state.clone();
                async move {
                    if attempt == 0 {
                        return Err(Failure::ConnectionLimit);
                    }
                    status(&state, Market::Options, "streaming", "Test events received");
                    std::future::pending::<Result<(), Failure>>().await
                }
            }),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        let s = state.lock().unwrap();
        assert_eq!(s.options.phase, "streaming");
        assert_eq!(s.options.reconnects, 1);
        assert_eq!(s.options.connection_faults[0].kind, "connection_limit");
        assert_eq!(s.stocks.reconnects, 0);
    }

    #[tokio::test]
    async fn connection_slot_retries_stop_when_another_consumer_persists() {
        let state = new_state();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        timeout(
            Duration::from_secs(35),
            supervise_sessions(&state, Market::Options, || {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                std::future::ready(Err(Failure::ConnectionLimit))
            }),
        )
        .await
        .unwrap();
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 5);
        let s = state.lock().unwrap();
        assert_eq!(s.options.phase, "denied");
        assert_eq!(s.options.reconnects, 4);
        assert!(s.options.detail.contains("persistente"));
    }

    #[tokio::test]
    async fn connection_slot_retries_never_retry_permissions_or_subscription_limits() {
        for subscription_limit in [false, true] {
            let state = new_state();
            let calls = std::sync::atomic::AtomicUsize::new(0);
            supervise_sessions(&state, Market::Options, || {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                std::future::ready(Err(if subscription_limit {
                    Failure::Limit
                } else {
                    Failure::Denied
                }))
            })
            .await;
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
            assert_eq!(state.lock().unwrap().options.phase, "denied");
        }
    }

    #[test]
    fn discovery_failures_keep_http_code_and_do_not_duplicate_socket_faults() {
        let state = new_state();
        record_session_failure(
            &state,
            Market::Options,
            0,
            &Failure::Diagnostic("rest_discovery", "http_status", Some(429)),
        );
        record_session_failure(&state, Market::Options, 0, &Failure::Transport);
        let s = state.lock().unwrap();
        assert_eq!(s.options.connection_faults.len(), 1);
        assert_eq!(s.options.connection_faults[0].stage, "rest_discovery");
        assert_eq!(s.options.connection_faults[0].code, Some(429));
    }

    #[tokio::test]
    async fn server_close_code_is_preserved_without_server_reason_text() {
        use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            while let Some(Ok(frame)) = ws.next().await {
                match frame {
                    Message::Ping(p) => ws.send(Message::Pong(p)).await.unwrap(),
                    Message::Text(t) => {
                        let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                        if v["action"] == "auth" {
                            ws.send(Message::Text(
                                r#"[{"ev":"status","status":"auth_success"}]"#.into(),
                            ))
                            .await
                            .unwrap();
                        } else {
                            ws.send(Message::Close(Some(CloseFrame {
                                code: CloseCode::Restart,
                                reason: "secret reason".into(),
                            })))
                            .await
                            .unwrap();
                            // Keep the transport alive so this tests a Close frame,
                            // not the OS-specific race of an abruptly dropped TCP stream.
                            sleep(Duration::from_millis(250)).await;
                            break;
                        }
                    }
                    _ => {}
                }
            }
        });
        let state = new_state();
        let result = timeout(
            Duration::from_secs(3),
            session_with_rotation(
                &Credentials("test".into()),
                &state,
                Market::Stocks,
                &["T.QQQ".into()],
                &url,
                std::future::pending::<Result<Selection, Failure>>,
            ),
        )
        .await;
        assert!(matches!(result, Ok(Err(Failure::Transport))));
        server.await.unwrap();
        let s = state.lock().unwrap();
        let fault = s.stocks.connection_faults.last().unwrap();
        assert_eq!(fault.kind, "server_close");
        assert_eq!(fault.code, Some(1012));
        assert!(!serde_json::to_string(&s.stocks)
            .unwrap()
            .contains("secret reason"));
    }

    #[test]
    fn connection_diagnostics_are_bounded_and_do_not_retain_error_secrets() {
        let state = super::new_state();
        for _ in 0..25 {
            super::socket_fault(
                &state,
                crate::Market::Stocks,
                "receive",
                tokio_tungstenite::tungstenite::Error::Io(std::io::Error::other(
                    "secret api key and URL",
                )),
            );
        }
        let s = state.lock().unwrap();
        assert_eq!(s.stocks.connection_faults.len(), 20);
        assert_eq!(s.stocks.connection_faults[0].kind, "io");
        assert!(!serde_json::to_string(&s.stocks).unwrap().contains("secret"));
        assert!(s.options.connection_faults.is_empty());
    }

    #[test]
    fn connection_diagnostics_distinguish_reset_from_local_timeout() {
        let state = super::new_state();
        super::socket_fault(
            &state,
            crate::Market::Options,
            "receive",
            tokio_tungstenite::tungstenite::Error::Protocol(
                tokio_tungstenite::tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
            ),
        );
        super::connection_fault(
            &state,
            crate::Market::Options,
            "heartbeat",
            "no_frames_65s",
            None,
        );
        let s = state.lock().unwrap();
        assert_eq!(
            s.options.connection_faults[0].kind,
            "reset_without_close_handshake"
        );
        assert_eq!(s.options.connection_faults[1].kind, "no_frames_65s");
    }
    use super::*;
    use crate::decode_batch;
    #[test]
    fn scanner_minutes_do_not_contaminate_active_asset_and_options_accept_expanded_universe() {
        let now = snipers_core::calendar::day_bounds("2026-09-24").unwrap().0 + 36_000_000;
        let mut state = LiveState {
            symbol: "QQQ".into(),
            ..Default::default()
        };
        state.scanner_store.initialize(now);
        let event = Event::MinuteBar {
            sym: "SPY".into(),
            s: now - 60_000,
            e: now,
            o: 100.0,
            h: 102.0,
            l: 99.0,
            c: 101.0,
            v: 10.0,
            vw: Some(100.0),
        };
        assert!(apply(&mut state, Market::Stocks, &event, now));
        assert!(state.ticks.is_empty());
        assert!(state.minutes.is_empty());
        assert!(!apply(
            &mut state,
            Market::Stocks,
            &Event::Quote {
                sym: "SPY".into(),
                bp: 100.0,
                ap: 101.0,
                t: now,
                bs: 1.0,
                ask_size: 1.0
            },
            now
        ));
        assert!(apply(
            &mut state,
            Market::Stocks,
            &Event::Quote {
                sym: "QQQ".into(),
                bp: 100.0,
                ap: 101.0,
                t: now,
                bs: 1.0,
                ask_size: 1.0
            },
            now
        ));
        for i in 0..1000 {
            assert!(apply(
                &mut state,
                Market::Options,
                &Event::Quote {
                    sym: format!("O:QQQ260924C{:08}", 500000 + i * 1000),
                    bp: 1.0,
                    ap: 2.0,
                    t: now,
                    bs: 1.0,
                    ask_size: 1.0
                },
                now
            ));
        }
        assert_eq!(state.ticks.len(), 1001);
        assert!(!apply(
            &mut state,
            Market::Options,
            &Event::Quote {
                sym: "O:QQQ260924C09999000".into(),
                bp: 1.0,
                ap: 2.0,
                t: now,
                bs: 1.0,
                ask_size: 1.0
            },
            now
        ));
    }
    #[test]
    fn structural_selection_expands_exact_expirations_and_rejects_stale_or_foreign_state() {
        use snipers_core::{
            calendar,
            options::{normalize, Observation},
        };
        let now = calendar::day_bounds("2026-09-24").unwrap().0 + 36_000_000;
        let mut observation = Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: now,
            observed_date: "2026-09-24".into(),
            oi_reference_date: None,
            contracts: BTreeMap::new(),
        };
        for (expiry, code) in [("2026-09-24", "260924"), ("2026-09-25", "260925")] {
            for (kind, letter, strike) in [("call", "C", 510), ("put", "P", 490)] {
                let ticker = format!("O:QQQ{code}{letter}{:08}", strike * 1000);
                let mut c=normalize(&serde_json::json!({"details":{"ticker":ticker,"strike_price":strike,"contract_type":kind,"expiration_date":expiry},"open_interest":100,"day":{"volume":20},"greeks":{"gamma":0.02,"delta":0.5}}),"QQQ").unwrap();
                c.premium_multiplier = Some(100.0);
                observation.contracts.insert(ticker, c);
            }
        }
        let mut state = LiveState {
            symbol: "QQQ".into(),
            ..Default::default()
        };
        state.option_snapshot_status.phase = "ready".into();
        state.option_snapshot_status.horizons =
            snipers_core::horizons::calculate(&observation, 500.0, "test").unwrap();
        state.option_snapshot = Some(observation);
        for h in &mut state.option_snapshot_status.horizons {
            h.structure.selected.insert("M1".into(), None);
            for (wall, strike) in [
                (&mut h.structure.call_wall, 510.0),
                (&mut h.structure.put_wall, 490.0),
            ] {
                let wall = wall.as_mut().unwrap();
                wall.strike = strike;
                wall.below = None;
                wall.above = None;
            }
            for r in &mut h.structure.routes {
                r.pull_target = None;
            }
        }
        let empty = || Selection {
            channels: vec![],
            contracts: BTreeMap::new(),
            info: SelectionState::default(),
        };
        let mut selection = empty();
        extend_structural_selection(&mut selection, &state, now).unwrap();
        assert_eq!(selection.info.structural_requested, 4);
        assert_eq!(selection.info.structural_resolved, 4);
        assert_eq!(selection.channels.len(), 8);
        assert_eq!(
            subscription_changes(&selection.channels, &selection.channels)
                .unwrap()
                .len(),
            0
        );
        state.flow_engine.set_contracts(selection.contracts.clone());
        for ticker in selection.contracts.keys() {
            state.flow_engine.trade(ticker, 1.0, 1.0, now, now);
        }
        state.flow = state.flow_engine.snapshot(now + 60_000);
        state.options.phase = "streaming".into();
        state.options.last_event_ms = Some(now + 60_000);
        let reports = crate::interpretation::wall_flow(&state, now + 60_000);
        assert_eq!(reports["DAY"].windows[0].call_zone.coverage_pct, 100.0);
        assert_eq!(reports["WEEK"].windows[0].call_zone.contracts, Some(2.0));
        assert_eq!(reports["WEEK"].windows[1].call_zone.contracts, None);
        assert!(!crate::interpretation::wall_flow(&state, now + 75_001)["DAY"].available);
        state
            .option_snapshot
            .as_mut()
            .unwrap()
            .contracts
            .remove("O:QQQ260925C00510000");
        let mut missing = empty();
        extend_structural_selection(&mut missing, &state, now).unwrap();
        assert_eq!(missing.info.structural_requested, 4);
        assert_eq!(missing.info.structural_resolved, 3);
        // Retiring a contract immediately removes its coverage; no nearby replacement.
        state.flow_engine.set_contracts(missing.contracts);
        state.flow = state.flow_engine.snapshot(now + 60_000);
        assert_eq!(
            crate::interpretation::wall_flow(&state, now + 60_000)["WEEK"].windows[0]
                .call_zone
                .contracts,
            None
        );
        for time in [now - 1, now + 900_001, now + 86_400_000] {
            let mut s = empty();
            extend_structural_selection(&mut s, &state, time).unwrap();
            assert!(s.channels.is_empty());
        }
        state.option_snapshot.as_mut().unwrap().symbol = "SPY".into();
        let mut s = empty();
        extend_structural_selection(&mut s, &state, now).unwrap();
        assert!(s.channels.is_empty());
        assert!(!crate::interpretation::wall_flow(&state, now + 60_000)["DAY"].available);
    }
    #[test]
    fn premium_multiplier_never_uses_adjusted_deliverable_share_count() {
        let ticker = "O:QQQ260925C00500000";
        let channels = vec![format!("T.{ticker}"), format!("Q.{ticker}")];
        let mut row = serde_json::json!({"ticker":ticker,"underlying_ticker":"QQQ","contract_type":"call","strike_price":500,"expiration_date":"2026-09-25","shares_per_contract":100});
        assert_eq!(
            contract_metadata(&[row.clone()], &channels, "QQQ")[ticker].premium_multiplier,
            Some(100.0)
        );
        row["shares_per_contract"] = serde_json::json!(10);
        assert_eq!(
            contract_metadata(&[row.clone()], &channels, "QQQ")[ticker].premium_multiplier,
            None
        );
        row["shares_per_contract"] = serde_json::json!(100);
        row["additional_underlyings"] = serde_json::json!([{"type":"currency","amount":5}]);
        assert_eq!(
            contract_metadata(&[row], &channels, "QQQ")[ticker].premium_multiplier,
            None
        );
    }
    #[test]
    fn live_flow_deduplicates_trades_and_invalidates_crossed_nbbo() {
        let state = new_state();
        let ticker = "O:QQQ260925C00500000";
        state
            .lock()
            .unwrap()
            .flow_engine
            .set_contracts(BTreeMap::from([(
                ticker.into(),
                snipers_core::flow::Contract {
                    ticker: ticker.into(),
                    kind: "call".into(),
                    strike: 500.0,
                    expiration: "2026-09-25".into(),
                    shares_per_contract: Some(100.0),
                    premium_multiplier: Some(100.0),
                },
            )]));
        let events=decode_batch(r#"[{"ev":"Q","sym":"O:QQQ260925C00500000","bp":1,"ap":1.2,"bs":1,"as":1,"t":999},{"ev":"T","sym":"O:QQQ260925C00500000","p":1.2,"s":1,"t":1000,"q":1},{"ev":"T","sym":"O:QQQ260925C00500000","p":1.2,"s":1,"t":1000,"q":1},{"ev":"Q","sym":"O:QQQ260925C00500000","bp":1.3,"ap":1.1,"bs":1,"as":1,"t":1001},{"ev":"T","sym":"O:QQQ260925C00500000","p":1.2,"s":1,"t":1002,"q":2}]"#).unwrap();
        for event in events {
            apply(&mut state.lock().unwrap(), Market::Options, &event, 1002);
        }
        let flow = state.lock().unwrap().flow_engine.snapshot(1002);
        assert_eq!(flow.windows[0].event_count, 2);
        assert_eq!(flow.windows[0].premium, 240.0);
        assert_eq!(flow.recent[0].side, "UNKNOWN");
        assert_eq!(flow.recent[1].side, "BUY");
        assert!(
            state
                .lock()
                .unwrap()
                .flow_engine
                .snapshot(61_002)
                .contract_windows[0]
                .covered
        );
        let bad = decode_batch(
            r#"[{"ev":"T","sym":"O:QQQ260925C00500000","p":0,"s":1,"t":61002,"q":3}]"#,
        )
        .unwrap();
        assert!(!apply(
            &mut state.lock().unwrap(),
            Market::Options,
            &bad[0],
            61_002
        ));
        assert!(
            !state
                .lock()
                .unwrap()
                .flow_engine
                .snapshot(61_002)
                .contract_windows[0]
                .covered
        );
    }
    #[test]
    fn options_clock_lead_keeps_quotes_for_classification_and_waits_to_publish() {
        let state = new_state();
        let ticker = "O:QQQ260925C00500000";
        let mut s = state.lock().unwrap();
        s.flow_engine.set_contracts(BTreeMap::from([(
            ticker.into(),
            snipers_core::flow::Contract {
                ticker: ticker.into(),
                kind: "call".into(),
                strike: 500.,
                expiration: "2026-09-25".into(),
                shares_per_contract: Some(100.),
                premium_multiplier: Some(100.),
            },
        )]));
        let events = decode_batch(r#"[{"ev":"Q","sym":"O:QQQ260925C00500000","bp":1,"ap":1.2,"bs":1,"as":1,"t":100500},{"ev":"T","sym":"O:QQQ260925C00500000","p":1.2,"s":1,"t":100600,"q":1}]"#).unwrap();
        for event in events {
            assert!(apply(&mut s, Market::Options, &event, 100000));
        }
        assert_eq!(s.flow_engine.snapshot(100000).windows[0].event_count, 0);
        let due = s.flow_engine.snapshot(100600);
        assert_eq!(due.recent[0].side, "BUY");
        assert_eq!(due.windows[0].buckets["CALL BUY"].premium, 120.);
        assert_eq!(due.rejected_trades, 0);
        assert!(s.flow_engine.snapshot(160601).contract_windows[0].covered);
    }
    #[test]
    fn trade_replay_cannot_double_count_or_regress_price_within_same_millisecond() {
        let state = new_state();
        let events = decode_batch(r#"[{"ev":"T","sym":"QQQ","p":501,"s":1,"t":1000,"q":9},{"ev":"T","sym":"QQQ","p":501,"s":1,"t":1000,"q":9},{"ev":"T","sym":"QQQ","p":500,"s":1,"t":1000,"q":7}]"#).unwrap();
        for event in events {
            apply(&mut state.lock().unwrap(), Market::Stocks, &event, 1100);
        }
        let s = snapshot(&state);
        assert_eq!(s.stocks.events, 2);
        assert_eq!(s.stocks.quality.duplicates, 1);
        assert_eq!(s.stocks.quality.out_of_order, 1);
        assert_eq!(s.ticks["QQQ"].price, Some(501.0));
    }
    #[test]
    fn expiration_floor_rolls_at_eastern_midnight_in_both_dst_seasons() {
        for (midnight, before, after) in [
            (1_790_222_400_000, "2026-09-23", "2026-09-24"),
            (1_769_230_800_000, "2026-01-23", "2026-01-24"),
        ] {
            assert_eq!(expiration_floor(midnight - 1).unwrap(), before);
            assert_eq!(expiration_floor(midnight).unwrap(), after);
        }
    }
    #[tokio::test]
    async fn failed_rotation_keeps_socket_and_existing_contracts() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let state = new_state();
        let observed = state.clone();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            while let Some(Ok(frame)) = ws.next().await {
                match frame {
                    Message::Ping(p) => ws.send(Message::Pong(p)).await.unwrap(),
                    Message::Text(text) => {
                        let request: serde_json::Value = serde_json::from_str(&text).unwrap();
                        if request["action"] == "auth" {
                            ws.send(Message::Text(
                                r#"[{"ev":"status","status":"auth_success"}]"#.into(),
                            ))
                            .await
                            .unwrap();
                        } else {
                            assert_eq!(request["action"], "subscribe");
                            sleep(Duration::from_millis(100)).await;
                            ws.send(Message::Text(
                                r#"[{"ev":"T","sym":"O:QQQ260925C00500000","p":1,"s":1,"t":1000}]"#
                                    .into(),
                            ))
                            .await
                            .unwrap();
                            timeout(Duration::from_secs(2), async {
                                while observed.lock().unwrap().options.events == 0 {
                                    sleep(Duration::from_millis(1)).await;
                                }
                            })
                            .await
                            .unwrap();
                            ws.close(None).await.unwrap();
                            break;
                        }
                    }
                    _ => {}
                }
            }
        });
        let result = timeout(
            Duration::from_secs(5),
            session_with_rotation(
                &Credentials("local-test".into()),
                &state,
                Market::Options,
                &["T.O:QQQ260925C00500000".into()],
                &url,
                || async {
                    sleep(Duration::from_millis(10)).await;
                    Err(Failure::Transport)
                },
            ),
        )
        .await;
        assert!(matches!(result, Ok(Err(Failure::Transport))));
        server.await.unwrap();
        let s = state.lock().unwrap();
        assert_eq!(s.options.events, 1);
        assert!(s.selection.error.is_some());
        assert_eq!(s.ticks["O:QQQ260925C00500000"].price, Some(1.0));
        // This server drops its TCP stream immediately after sending Close;
        // a concurrent rotation write can observe an I/O error first on Windows.
        assert!(!s.options.connection_faults.is_empty());
    }
    #[tokio::test]
    #[ignore = "75-second local WebSocket continuity audit; no provider credentials"]
    async fn socket_stays_alive_across_30_and_60_seconds() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let state = new_state();
        let observed = state.clone();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let deadline = tokio::time::Instant::now() + Duration::from_secs(75);
            let mut timer = tokio::time::interval(Duration::from_secs(1));
            let mut subscribed = false;
            let mut sent = 0;
            let mut auth_count = 0;
            loop {
                tokio::select! {
                    _ = tokio::time::sleep_until(deadline) => break,
                    _ = timer.tick(), if subscribed => {
                        sent += 1;
                        let trade = serde_json::json!([{"ev":"T","sym":"O:QQQ260925C00500000","p":1.0,"s":1,"t":now_ms(),"q":sent}]);
                        ws.send(Message::Text(trade.to_string().into())).await.unwrap();
                    }
                    frame = ws.next() => {
                        match frame.expect("socket ended before 75 seconds").unwrap() {
                            Message::Ping(p) => ws.send(Message::Pong(p)).await.unwrap(),
                            Message::Text(text) => {
                                let request: serde_json::Value = serde_json::from_str(&text).unwrap();
                                if request["action"] == "auth" {
                                    auth_count += 1;
                                    ws.send(Message::Text(r#"[{"ev":"status","status":"auth_success"}]"#.into())).await.unwrap();
                                }
                                if request["action"] == "subscribe" { subscribed = true; }
                            }
                            _ => {}
                        }
                    }
                }
            }
            assert_eq!(auth_count, 1);
            assert!(sent >= 70);
            assert!(observed.lock().unwrap().options.events >= 70);
            ws.close(None).await.unwrap();
        });
        let result = timeout(
            Duration::from_secs(80),
            session_with_rotation(
                &Credentials("local-test".into()),
                &state,
                Market::Options,
                &["T.O:QQQ260925C00500000".into()],
                &url,
                || async {
                    sleep(Duration::from_secs(30)).await;
                    Ok(Selection {
                        contracts: BTreeMap::new(),
                        channels: vec!["T.O:QQQ260925C00500000".into()],
                        info: SelectionState {
                            updated_ms: Some(now_ms()),
                            ..Default::default()
                        },
                    })
                },
            ),
        )
        .await;
        assert!(matches!(result, Ok(Err(Failure::Transport))));
        server.await.unwrap();
        let s = state.lock().unwrap();
        assert!(s.options.events >= 70);
        assert_eq!(s.options.reconnects, 0);
        assert!(s.selection.updated_ms.is_some());
        assert_eq!(s.ticks["O:QQQ260925C00500000"].price, Some(1.0));
    }

    #[tokio::test]
    async fn socket_rotates_without_reconnect_and_purges_retired_contracts() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let state = new_state();
        let observed = state.clone();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let mut actions = Vec::new();
            while let Some(Ok(frame)) = ws.next().await {
                match frame {
                    Message::Ping(p) => ws.send(Message::Pong(p)).await.unwrap(),
                    Message::Text(text) => {
                        let request: serde_json::Value = serde_json::from_str(&text).unwrap();
                        let action = request["action"].as_str().unwrap().to_owned();
                        actions.push(action.clone());
                        if action == "auth" {
                            ws.send(Message::Text(
                                r#"[{"ev":"status","status":"auth_success"}]"#.into(),
                            ))
                            .await
                            .unwrap();
                        } else if actions.len() == 2 {
                            ws.send(Message::Text(
                                r#"[{"ev":"T","sym":"O:QQQ260925C00500000","p":1,"s":1,"t":1000}]"#
                                    .into(),
                            ))
                            .await
                            .unwrap();
                        } else if actions.len() == 4 {
                            assert_eq!(request["params"], "T.O:QQQ260926C00500000");
                            ws.send(Message::Text(r#"[{"ev":"T","sym":"O:QQQ260925C00500000","p":9,"s":1,"t":2000},{"ev":"T","sym":"O:QQQ260926C00500000","p":2,"s":1,"t":2000}]"#.into())).await.unwrap();
                            timeout(Duration::from_secs(2), async {
                                while observed.lock().unwrap().options.events < 2 {
                                    sleep(Duration::from_millis(1)).await;
                                }
                            })
                            .await
                            .unwrap();
                            ws.close(None).await.unwrap();
                            break;
                        }
                    }
                    _ => {}
                }
            }
            assert_eq!(actions, ["auth", "subscribe", "unsubscribe", "subscribe"]);
        });
        let result = timeout(
            Duration::from_secs(5),
            session_with_rotation(
                &Credentials("local-test".into()),
                &state,
                Market::Options,
                &["T.O:QQQ260925C00500000".into()],
                &url,
                || async {
                    sleep(Duration::from_millis(100)).await;
                    Ok(Selection {
                        contracts: BTreeMap::new(),
                        channels: vec!["T.O:QQQ260926C00500000".into()],
                        info: SelectionState {
                            expiry: "2026-09-26".into(),
                            ..Default::default()
                        },
                    })
                },
            ),
        )
        .await;
        assert!(matches!(result, Ok(Err(Failure::Transport))));
        server.await.unwrap();
        let s = state.lock().unwrap();
        assert_eq!(s.options.events, 2);
        assert_eq!(s.options.reconnects, 0);
        assert!(!s.ticks.contains_key("O:QQQ260925C00500000"));
        assert_eq!(s.ticks["O:QQQ260926C00500000"].price, Some(2.0));
        assert_eq!(s.selection.expiry, "2026-09-26");
    }
    #[test]
    fn rotation_only_changes_members_and_rejects_retired_events() {
        let old = vec![
            "T.O:QQQ260925C00500000".into(),
            "Q.O:QQQ260925C00500000".into(),
        ];
        assert!(subscription_changes(&old, &old).unwrap().is_empty());
        let new = vec![
            "T.O:QQQ260925P00500000".into(),
            "Q.O:QQQ260925P00500000".into(),
        ];
        let requests: Vec<serde_json::Value> = subscription_changes(&old, &new)
            .unwrap()
            .iter()
            .map(|r| serde_json::from_str(r).unwrap())
            .collect();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["action"], "unsubscribe");
        assert_eq!(requests[1]["action"], "subscribe");
        let event = Event::Trade {
            sym: "O:QQQ260925C00500000".into(),
            p: 1.0,
            s: 1.0,
            t: 1,
            q: None,
            metadata: Default::default(),
        };
        assert!(event_selected(Market::Options, &old, &event));
        assert!(!event_selected(Market::Options, &new, &event));
        let too_many: Vec<String> = (0..1001).map(|i| format!("Q.O:QQQ260925C{i:08}")).collect();
        assert!(subscription_changes(&old, &too_many).is_err());
    }
    #[test]
    fn selection_reference_rejects_stale_and_future_prices() {
        let state = new_state();
        state.lock().unwrap().ticks.insert(
            "QQQ".into(),
            Tick {
                symbol: "QQQ".into(),
                price: Some(500.0),
                bid: Some(501.0),
                ask: Some(503.0),
                trade_ms: Some(1000),
                trade_sequence: None,
                quote_ms: Some(122000),
            },
        );
        assert_eq!(current_spot(&state, 1000), Some(500.0));
        assert_eq!(current_spot(&state, 122000), Some(502.0));
        assert_eq!(current_spot(&state, 999), None);
        assert_eq!(current_spot(&state, 242001), None);
    }
    #[test]
    fn independent_asset_sessions_reject_foreign_symbols_and_option_roots() {
        let qqq = new_state_for("QQQ").unwrap();
        let spy = new_state_for("SPY").unwrap();
        let trade = Event::Trade {
            sym: "SPY".into(),
            p: 600.0,
            s: 1.0,
            t: 1000,
            q: None,
            metadata: Default::default(),
        };
        assert!(!apply(
            &mut qqq.lock().unwrap(),
            Market::Stocks,
            &trade,
            1001
        ));
        assert!(apply(
            &mut spy.lock().unwrap(),
            Market::Stocks,
            &trade,
            1001
        ));
        assert!(qqq.lock().unwrap().ticks.is_empty());
        assert_eq!(spy.lock().unwrap().ticks["SPY"].price, Some(600.0));
        let wrong = Event::Trade {
            sym: "O:QQQJ260925C00500000".into(),
            p: 1.0,
            s: 1.0,
            t: 1000,
            q: None,
            metadata: Default::default(),
        };
        assert!(!apply(
            &mut qqq.lock().unwrap(),
            Market::Options,
            &wrong,
            1001
        ));
        assert!(new_state_for("SPX").is_err());
        assert!(new_state_for("../env").is_err());
    }
    #[tokio::test]
    async fn socket_auth_subscribe_and_mixed_batch_preserve_valid_events() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let state = new_state();
        let observed = state.clone();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let auth = ws.next().await.unwrap().unwrap().into_text().unwrap();
            let auth: serde_json::Value = serde_json::from_str(&auth).unwrap();
            assert_eq!(auth["action"], "auth");
            ws.send(Message::Text(
                r#"[{"ev":"status","status":"auth_success"}]"#.into(),
            ))
            .await
            .unwrap();
            loop {
                let frame = ws.next().await.unwrap().unwrap();
                match frame {
                    Message::Text(text) => {
                        let request: serde_json::Value = serde_json::from_str(&text).unwrap();
                        assert_eq!(request["action"], "subscribe");
                        assert_eq!(request["params"], "T.QQQ");
                        break;
                    }
                    Message::Ping(p) => ws.send(Message::Pong(p)).await.unwrap(),
                    _ => {}
                }
            }
            ws.send(Message::Text(r#"[{"ev":"T","sym":"QQQ","p":"bad"},{"ev":"FUTURE_TYPE"},{"ev":"T","sym":"QQQ","p":500,"s":1,"t":1234}]"#.into())).await.unwrap();
            timeout(Duration::from_secs(2), async {
                while observed.lock().unwrap().stocks.events < 1 {
                    sleep(Duration::from_millis(1)).await;
                }
            })
            .await
            .unwrap();
            ws.close(None).await.unwrap();
        });
        let outcome = timeout(
            Duration::from_secs(5),
            session_channels(
                &Credentials("test-only".into()),
                &state,
                Market::Stocks,
                &["T.QQQ".into()],
                &url,
            ),
        )
        .await;
        assert!(matches!(outcome, Ok(Err(Failure::Transport))));
        server.await.unwrap();
        let snapshot = state.lock().unwrap();
        assert_eq!(snapshot.stocks.malformed, 1);
        assert_eq!(snapshot.stocks.events, 1);
        assert_eq!(snapshot.ticks["QQQ"].price, Some(500.0));
        assert_eq!(snapshot.stocks.phase, "streaming");
    }

    #[tokio::test]
    async fn socket_rejected_auth_does_not_subscribe() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let _ = ws.next().await.unwrap();
            ws.send(Message::Text(
                r#"[{"ev":"status","status":"auth_failed"}]"#.into(),
            ))
            .await
            .unwrap();
            while let Some(Ok(frame)) = ws.next().await {
                match frame {
                    Message::Text(_) => panic!("Client subscribed after denied auth"),
                    Message::Ping(p) => {
                        let _ = ws.send(Message::Pong(p)).await;
                    }
                    _ => {}
                }
            }
        });
        let state = new_state();
        let result = timeout(
            Duration::from_secs(5),
            session_channels(
                &Credentials("test-only".into()),
                &state,
                Market::Stocks,
                &["T.QQQ".into()],
                &url,
            ),
        )
        .await;
        assert!(matches!(result, Ok(Err(Failure::Denied))));
        server.await.unwrap();
        assert_eq!(state.lock().unwrap().stocks.events, 0);
    }
    #[test]
    fn correction_replaces_volume_and_late_trade_does_not_regress_price() {
        let mut s = LiveState {
            symbol: "QQQ".into(),
            ..Default::default()
        };
        for volume in [10, 20, 20] {
            let event = decode_batch(&format!(r#"[{{"ev":"A","sym":"QQQ","o":10,"h":12,"l":9,"c":11,"v":{volume},"s":1000,"e":1999}}]"#)).unwrap().remove(0);
            apply(&mut s, Market::Stocks, &event, 2000);
        }
        assert_eq!(s.bars.len(), 1);
        assert_eq!(s.bars[&1000].volume, 20.0);
        for (t, p) in [(2000, 12.0), (1000, 10.0)] {
            apply(
                &mut s,
                Market::Stocks,
                &Event::Trade {
                    sym: "QQQ".into(),
                    p,
                    s: 1.0,
                    t,
                    q: None,
                    metadata: Default::default(),
                },
                3000,
            );
        }
        assert_eq!(s.ticks["QQQ"].price, Some(12.0));
        assert_eq!(s.stocks.last_event_ms, Some(2000));
    }
    #[test]
    fn nearest_contracts_are_capped_and_separated_by_type() {
        let rows: Vec<_> = (0..20).flat_map(|i| ["call","put"].map(move |k| serde_json::json!({"ticker":format!("O:QQQ260925{}{:08}",if k=="call"{"C"}else{"P"},(490+i)*1000),"strike_price":490+i,"contract_type":k}))).collect();
        let channels = select_contracts(&rows, 500.0, "QQQ").unwrap();
        assert_eq!(channels.len(), 24);
        assert_eq!(channels[0], "T.O:QQQ260925C00500000");
    }
    #[test]
    fn invalid_quotes_and_foreign_stock_symbols_cannot_contaminate_state() {
        let mut s = LiveState {
            symbol: "QQQ".into(),
            ..Default::default()
        };
        let events = decode_batch(r#"[{"ev":"Q","sym":"QQQ","bp":12,"ap":10,"bs":1,"as":1,"t":1},{"ev":"T","sym":"SPY","p":10,"s":1,"t":1}]"#).unwrap();
        for e in events {
            assert!(!apply(&mut s, Market::Stocks, &e, 2));
        }
        assert!(s.ticks.is_empty());
    }
    #[test]
    fn held_position_trade_lane_is_fresh_and_does_not_change_selected_asset() {
        let mut s = LiveState {
            symbol: "SPY".into(),
            held_symbol: Some("QQQ".into()),
            ..Default::default()
        };
        let events = decode_batch(r#"[{"ev":"T","sym":"QQQ","p":500,"s":1,"t":20000,"q":1},{"ev":"T","sym":"QQQ","p":501,"s":1,"t":19999,"q":2},{"ev":"T","sym":"QQQ","p":502,"s":1,"t":22000,"q":3},{"ev":"Q","sym":"QQQ","bp":499,"ap":501,"bs":1,"as":1,"t":20000},{"ev":"T","sym":"IWM","p":200,"s":1,"t":20000}]"#).unwrap();
        assert!(apply(&mut s, Market::Stocks, &events[0], 21000));
        for event in &events {
            assert!(!apply(&mut s, Market::Stocks, event, 21000));
        }
        assert_eq!(s.held_tick.as_ref().unwrap().price, Some(500.0));
        assert!(s.ticks.is_empty());
        assert!(s.bars.is_empty());
        assert!(s.minutes.is_empty());
        assert!(!apply(&mut s, Market::Stocks, &events[2], 40000));
        assert!(apply(&mut s, Market::Stocks, &events[2], 22000));
        assert_eq!(s.held_tick.as_ref().unwrap().price, Some(502.0));
        assert_eq!(
            snapshot(&Arc::new(Mutex::new(s))).held_tick.unwrap().symbol,
            "QQQ"
        );
    }
    #[test]
    fn multiple_imported_positions_keep_independent_trade_clocks_and_selected_chart() {
        let mut s = LiveState {
            symbol: "QQQ".into(),
            legacy_symbols: vec!["MSFT".into(), "WMT".into()],
            ..Default::default()
        };
        let events=decode_batch(r#"[{"ev":"T","sym":"MSFT","p":500,"s":1,"t":20000,"q":1},{"ev":"T","sym":"WMT","p":109,"s":1,"t":20000,"q":1},{"ev":"T","sym":"MSFT","p":499,"s":1,"t":19000,"q":2},{"ev":"T","sym":"IWM","p":200,"s":1,"t":20000,"q":1},{"ev":"Q","sym":"WMT","bp":108,"ap":110,"bs":1,"as":1,"t":20000}]"#).unwrap();
        assert!(apply(&mut s, Market::Stocks, &events[0], 21000));
        assert!(apply(&mut s, Market::Stocks, &events[1], 21000));
        for e in &events[2..] {
            assert!(!apply(&mut s, Market::Stocks, e, 21000));
        }
        assert_eq!(s.legacy_ticks["MSFT"].price, Some(500.));
        assert_eq!(s.legacy_ticks["WMT"].price, Some(109.));
        assert!(s.ticks.is_empty());
        assert!(s.bars.is_empty());
        assert!(s.held_tick.is_none());
        let copy = snapshot(&Arc::new(Mutex::new(s)));
        assert_eq!(copy.legacy_ticks.len(), 2);
        assert_eq!(copy.legacy_symbols.len(), 2);
    }
    #[test]
    fn scanner_flow_routes_only_covered_selected_asset_and_disconnect_revokes_it() {
        let now = snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut s = LiveState {
            symbol: "QQQ".into(),
            flow_second: Some(now / 1000),
            ..Default::default()
        };
        s.options.phase = "streaming".into();
        s.flow = s.flow_engine.snapshot(now);
        let window = &mut s.flow.windows[0];
        window.event_count = 1;
        window.premium = 100.0;
        window.premium_complete = true;
        window.direction = "BULLISH FLOW".into();
        window.bullish_pct = Some(70.0);
        window.bearish_pct = Some(30.0);
        s.flow
            .contract_windows
            .push(snipers_core::flow::ContractWindow {
                ticker: "O:QQQ260924C00500000".into(),
                kind: "call".into(),
                strike: 500.0,
                expiration: "2026-09-24".into(),
                minutes: 1,
                observed_since_ms: Some(now - 60_000),
                covered: true,
                contracts: Some(1.0),
                premium: Some(100.0),
                trades: Some(1),
                reason: "test".into(),
            });
        let mut report = crate::scanner::Report::default();
        for symbol in ["QQQ", "SPY"] {
            report.top.push(
                snipers_core::scanner::candidate(
                    &snipers_core::scanner::Input {
                        symbol: symbol.into(),
                        bar_end_ms: now,
                        price: 101.0,
                        volume_2m: 100_000.0,
                        vwap: Some(100.0),
                        rvol: Some(2.0),
                        source_verified: true,
                    },
                    now,
                )
                .unwrap(),
            );
        }
        scanner_flow_confirmations(&mut s, &mut report, now);
        assert_eq!(report.flow_confirmations["QQQ"].aligned, Some(true));
        assert!(!report.flow_confirmations["SPY"].ready);
        s.options.phase = "reconnecting".into();
        scanner_flow_confirmations(&mut s, &mut report, now);
        assert!(!report.flow_confirmations["QQQ"].ready);
        s.options.phase = "streaming".into();
        s.flow.contract_windows[0].ticker = "O:SPY260924C00500000".into();
        scanner_flow_confirmations(&mut s, &mut report, now);
        assert!(!report.flow_confirmations["QQQ"].ready);
    }
    #[test]
    fn shared_option_plan_routes_and_deduplicates_members_without_polluting_active_flow() {
        let now = snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut state = LiveState {
            symbol: "QQQ".into(),
            scanner_options: crate::scanner_options::fixture_pool(now),
            ..Default::default()
        };
        let mut primary = state
            .scanner_options
            .contracts("QQQ", now, 1000)
            .into_values()
            .next()
            .unwrap();
        primary.ticker = "O:QQQ260924C00500000".into();
        let mut selection = Selection {
            channels: vec![
                format!("T.{}", primary.ticker),
                format!("Q.{}", primary.ticker),
            ],
            info: SelectionState::default(),
            contracts: [(primary.ticker.clone(), primary)].into(),
        };
        extend_pool_selection(&mut selection, &state, now).unwrap();
        assert_eq!(selection.channels.len(), 4);
        assert_eq!(selection.contracts.len(), 2);
        install_option_contracts(&mut state, selection.contracts);
        for (index, t) in [now, now + 60_000].into_iter().enumerate() {
            let events=decode_batch(&serde_json::json!([{"ev":"Q","sym":"O:SPY260924C00500000","bp":1.0,"ap":1.1,"bs":10,"as":10,"t":t},{"ev":"T","sym":"O:SPY260924C00500000","p":1.1,"s":1,"t":t,"q":index+1}]).to_string()).unwrap();
            for event in &events {
                assert!(event_selected(Market::Options, &selection.channels, event));
                assert!(apply(&mut state, Market::Options, event, t));
            }
            assert!(!apply(&mut state, Market::Options, &events[1], t));
        }
        assert!(state.ticks.is_empty());
        assert_eq!(
            state.flow_engine.snapshot(now + 60_000).windows[0].event_count,
            0
        );
        assert_eq!(
            state
                .scanner_options
                .flow_read("SPY", "ALCISTA", now + 60_000, true)
                .aligned,
            Some(true)
        );
        state.scanner_options.reconcile(&[], now);
        let event=decode_batch(&serde_json::json!([{"ev":"T","sym":"O:SPY260924C00500000","p":1.1,"s":1,"t":now+60_001,"q":3}]).to_string()).unwrap().remove(0);
        assert!(!apply(&mut state, Market::Options, &event, now + 60_001));
    }
    #[test]
    fn spx_quote_lane_uses_shared_capacity_without_polluting_active_price_or_flow() {
        let now = snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut s = LiveState {
            symbol: "QQQ".into(),
            spx_engine: crate::spx_synthetic::fixture_engine(now),
            ..Default::default()
        };
        let mut selection = Selection {
            contracts: BTreeMap::new(),
            channels: vec![],
            info: Default::default(),
        };
        extend_pool_selection(&mut selection, &s, now).unwrap();
        assert_eq!(selection.channels.len(), 6);
        assert!(selection.channels.iter().all(|c| c.starts_with("Q.O:SPXW")));
        install_option_contracts(&mut s, selection.contracts);
        for (strike, encoded) in [
            (5995., "05995000"),
            (6000., "06000000"),
            (6005., "06005000"),
        ] {
            for (code, sign) in [("C", 1.), ("P", -1.)] {
                let mid = 20. + sign * (6000. - strike) / 2.;
                let e=decode_batch(&serde_json::json!([{"ev":"Q","sym":format!("O:SPXW260925{code}{encoded}"),"bp":mid-0.1,"ap":mid+0.1,"bs":1,"as":1,"t":now}]).to_string()).unwrap().remove(0);
                assert!(apply(&mut s, Market::Options, &e, now));
            }
        }
        assert_eq!(s.spx_engine.snapshot(now, true).phase, "ready");
        assert!(s.ticks.is_empty());
        assert!(s.flow_engine.snapshot(now).contract_windows.is_empty());
        reset_option_coverage(&mut s);
        assert!(s.spx_engine.snapshot(now, true).estimate.is_none());
    }
    #[test]
    fn scanner_final_layer_clears_missing_members_without_sharing_confirmations() {
        use snipers_core::{scanner_memory, scanner_trigger};
        let now = snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut state = LiveState::default();
        let symbols = vec!["QQQ".into(), "SPY".into()];
        state.scanner_memory.reconcile(&symbols, now);
        let trigger = scanner_trigger::Read {
            ready: true,
            reference_price: Some(102.),
            invalidation: Some(99.),
            ..Default::default()
        };
        for symbol in &symbols {
            assert!(
                state
                    .scanner_memory
                    .evaluate(
                        &scanner_memory::Evidence {
                            targets: &[],
                            symbol,
                            bar_ms: now,
                            close: 102.,
                            vwap: 100.,
                            bullish: true,
                            verified: true,
                            all_gates: true,
                            trigger: &trigger
                        },
                        now
                    )
                    .signal_active
            );
        }
        let mut report = crate::scanner::Report {
            selection: snipers_core::scanner_selection::Status {
                symbols,
                ..Default::default()
            },
            ..Default::default()
        };
        scanner_final_confirmations(&mut state, &mut report, now);
        for symbol in ["QQQ", "SPY"] {
            assert_eq!(report.confirmations[symbol].state, "CONDICIONES PERDIDAS");
            assert!(!report.confirmations[symbol].signal_active);
            assert!(!report.structure_confirmations[symbol].ready);
        }
        // Recovery without verified candles/structure cannot resurrect old entries.
        report.connected = true;
        state.options.phase = "streaming".into();
        scanner_final_confirmations(&mut state, &mut report, now + 1);
        assert!(report
            .confirmations
            .values()
            .all(|r| !r.signal_active && !r.entry_confirmed));
    }
    #[test]
    fn reconnect_recovers_a_new_full_window_without_erasing_old_fault_counters() {
        let now = snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut state = LiveState {
            symbol: "QQQ".into(),
            ..Default::default()
        };
        let mut contract = crate::scanner_options::fixture_pool(now)
            .contracts("QQQ", now, 1000)
            .into_values()
            .next()
            .unwrap();
        contract.ticker = "O:QQQ260924C00500000".into();
        state
            .flow_engine
            .set_contracts([(contract.ticker.clone(), contract.clone())].into());
        let add = |s: &mut LiveState, t: u64| {
            s.flow_engine.quote(&contract.ticker, 1.0, 1.1, t);
            s.flow_engine.trade(&contract.ticker, 1.1, 1.0, t, t);
        };
        add(&mut state, now);
        state.options.malformed = 1;
        add(&mut state, now + 60_000);
        refresh_flow(&mut state, now + 60_000);
        assert!(!state.flow.contract_windows[0].covered);
        reset_option_coverage(&mut state);
        assert_eq!(state.options.malformed, 1);
        add(&mut state, now + 61_000);
        refresh_flow(&mut state, now + 61_000);
        assert!(!state.flow.contract_windows[0].covered);
        add(&mut state, now + 121_000);
        refresh_flow(&mut state, now + 121_000);
        assert!(
            state
                .flow
                .contract_windows
                .iter()
                .find(|w| w.minutes == 1)
                .unwrap()
                .covered
        );
        state.options.malformed += 1;
        refresh_flow(&mut state, now + 122_000);
        assert!(!state.flow.contract_windows[0].covered);
        let future=decode_batch(&serde_json::json!([{"ev":"Q","sym":contract.ticker,"bp":1.0,"ap":1.1,"bs":1,"as":1,"t":now+1_000_000}]).to_string()).unwrap().remove(0);
        assert!(!apply(&mut state, Market::Options, &future, now + 122_000));
        assert_eq!(state.flow_engine.liquidity(now + 122_000, true).quoted, 0);
    }
}

#[cfg(test)]mod background_price_alarm_tests {
 use super::*;
 #[test]fn background_alarm_uses_real_feed_lane_without_contaminating_selected_chart(){
  let state=new_state_for("QQQ").unwrap();
  let mut s=state.lock().unwrap();
  s.price_alert_engine.configure(vec![crate::price_alerts::Alert{id:"alarm-aapl".into(),symbol:"AAPL".into(),price:100.,direction:"up".into(),sound:true,armed_at:1,fired_at:None,fired_price:None}]).unwrap();
  let events=crate::decode_batch(r#"[{"ev":"T","sym":"AAPL","p":99,"s":10,"t":10000,"q":1},{"ev":"T","sym":"AAPL","p":101,"s":10,"t":10001,"q":2},{"ev":"T","sym":"AAPL","p":99,"s":10,"t":10002,"q":3}]"#).unwrap();
  for e in events {apply(&mut s,Market::Stocks,&e,10002);}
  assert_eq!(s.symbol,"QQQ");assert!(!s.ticks.contains_key("AAPL"));assert!(s.bars.is_empty());
  assert_eq!(s.price_alert_engine.snapshot()[0].fired_at,Some(10001));
  drop(s);let snap=snapshot(&state);assert_eq!(snap.price_alerts[0].symbol,"AAPL");assert_eq!(snap.price_alerts[0].fired_price,Some(101.));
 }
}

#[cfg(test)]
mod stock_quantity_contract_tests {
 use super::*;
 fn print(symbol:&str,price:f64,legacy:f64,decimal:Option<&str>,t:u64,q:u64)->Event {
  serde_json::from_value(serde_json::json!({"ev":"T","sym":symbol,"p":price,"s":legacy,"ds":decimal,"t":t,"q":q,"c":[0]})).unwrap()
 }
 #[test]
 fn fractional_trade_ledger_is_conserved_by_amt_and_selected_cvd(){
  let t=snipers_core::calendar::legacy_eastern_ms("2026-10-12 10:00:00").unwrap();
  let mut s=LiveState{symbol:"NVDA".into(),..Default::default()};
  apply(&mut s,Market::Stocks,&Event::Quote{sym:"NVDA".into(),bp:99.99,ap:100.,bs:10.,ask_size:10.,t},t);
  for (q,p,legacy,ds) in [(1,100.,1.,"1.25"),(2,99.99,0.,"0.002"),(3,99.995,0.,"0.5")]{
   let event=print("NVDA",p,legacy,Some(ds),t+q,q);
   assert!(apply(&mut s,Market::Stocks,&event,t+q));
   assert!(!apply(&mut s,Market::Stocks,&event,t+q));
  }
  let r=s.cvd_engine.snapshot("NVDA",t+3,true);
  assert_eq!(r.regular.buy,1.25);assert_eq!(r.regular.sell,0.002);assert_eq!(r.regular.unknown,0.5);
  assert!((r.regular.delta-1.248).abs()<1e-12);
  let profile=s.auction_engine.snapshot("NVDA",t+3,true,vec![]).unwrap();
  assert!((profile["totalVolume"].as_f64().unwrap()-1.752).abs()<1e-12);
  assert!((r.regular.buy+r.regular.sell+r.regular.unknown-profile["totalVolume"].as_f64().unwrap()).abs()<1e-12);
  assert_eq!(profile["acceptedTrades"],3);
 }
 #[test]
 fn invalid_decimal_never_falls_back_to_legacy_for_analysis_engines(){
  let t=snipers_core::calendar::legacy_eastern_ms("2026-10-12 10:00:00").unwrap();
  let mut s=LiveState{symbol:"NVDA".into(),..Default::default()};
  for (i,ds) in ["bad","NaN","0","-1"].iter().enumerate(){let q=i as u64+1;apply(&mut s,Market::Stocks,&print("NVDA",100.,100.,Some(ds),t+q,q),t+q);}
  let r=s.cvd_engine.snapshot("NVDA",t+4,true);assert_eq!(r.regular.buy+r.regular.sell+r.regular.unknown,0.);
  assert!(s.auction_engine.snapshot("NVDA",t+4,true,vec![]).is_none());
  apply(&mut s,Market::Stocks,&print("NVDA",100.,10.,None,t+5,5),t+5);
  assert_eq!(s.cvd_engine.snapshot("NVDA",t+5,true).regular.unknown,10.);
 }
 #[test]
 fn independent_etf_cvd_uses_decimal_without_changing_bot_lane(){
  let t=snipers_core::calendar::legacy_eastern_ms("2026-10-12 10:00:00").unwrap();
  let mut s=LiveState{symbol:"NVDA".into(),..Default::default()};s.set_etf_scanner_enabled(true);
  apply(&mut s,Market::Stocks,&Event::Quote{sym:"QQQ".into(),bp:99.99,ap:100.,bs:10.,ask_size:10.,t},t);
  let event=print("QQQ",100.,0.,Some("0.25"),t+1,1);apply(&mut s,Market::Stocks,&event,t+1);apply(&mut s,Market::Stocks,&event,t+1);
  assert_eq!(s.etf_cvd_engines["QQQ"].snapshot("QQQ",t+1,true).regular.buy,0.25);
  assert!(s.bot_cvd_engines.is_empty());
 }
}

/// Offline replay adapter; never opens a connection or enables orders.
pub fn ingest_recording(state:&SharedState,recording:&crate::recording::Recording){for row in &recording.records{apply(&mut state.lock().unwrap(),row.market,&row.event,row.received_ms);}}
#[cfg(test)]mod foreign_option_coverage_tests {
    use super::*;
    fn one_minute(s:&LiveState)->bool{s.flow.contract_windows.iter().find(|w|w.minutes==1).unwrap().covered}
    #[test]
    fn other_root_trades_never_reach_the_selected_flow_or_revoke_its_coverage(){
        let now=snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut s=LiveState{symbol:"QQQ".into(),..Default::default()};s.options.phase="streaming".into();
        let mut contract=crate::scanner_options::fixture_pool(now).contracts("QQQ",now,1000).into_values().next().unwrap();
        contract.ticker="O:QQQ260924C00500000".into();
        s.flow_engine.set_contracts([(contract.ticker.clone(),contract.clone())].into());
        // Every event uses the production router, as the socket loop does.
        let event=|v:serde_json::Value|crate::decode_batch(&serde_json::json!([v]).to_string()).unwrap().remove(0);
        for (q,t) in [(1,now),(2,now+60_000)]{
            assert!(apply(&mut s,Market::Options,&event(serde_json::json!({"ev":"Q","sym":contract.ticker,"bp":1.0,"ap":1.1,"bs":1,"as":1,"t":t})),t));
            assert!(apply(&mut s,Market::Options,&event(serde_json::json!({"ev":"T","sym":contract.ticker,"p":1.1,"s":1,"t":t,"q":q})),t));
        }
        refresh_flow(&mut s,now+60_000);assert!(one_minute(&s));
        // Background pools subscribe other roots on the same socket; apply() must divert them.
        assert!(!apply(&mut s,Market::Options,&event(serde_json::json!({"ev":"T","sym":"O:SPY260924C00500000","p":1.1,"s":1,"t":now+60_500,"q":7})),now+60_500));
        refresh_flow(&mut s,now+61_000);assert!(one_minute(&s));
        // Same-root unknown contracts still fail closed: missing metadata is not zero volume.
        apply(&mut s,Market::Options,&event(serde_json::json!({"ev":"T","sym":"O:QQQ260924P00400000","p":1.0,"s":1,"t":now+61_500,"q":8})),now+61_500);
        refresh_flow(&mut s,now+62_000);assert!(!one_minute(&s));
    }
}
