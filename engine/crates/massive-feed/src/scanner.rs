//! Bounded multi-asset minute tapes on the existing Stocks connection.
use crate::{
    credentials::Credentials,
    live::{now_ms, Bar, SharedState},
    metrics::{self, SessionMetrics},
    Event,
};
use serde::Serialize;
use snipers_core::{
    calendar::{self, Day},
    scanner::{self, Input},
    scanner_selection, scanner_trigger, universe,
};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub struct Tape {
    volume_key: Option<(u64, u64)>,
    volume_windows: Vec<VolumeWindow>,
    morning_key: Option<(u64, u64)>,
    morning: Option<snipers_core::premarket::Read>,
    premarket_key: Option<(u64, u64)>,
    premarket: Option<snipers_core::premarket::Read>,
    minutes: BTreeMap<u64, Bar>,
    versions: BTreeMap<u64, u64>,
    revision: u64,
    history_ready: bool,
    through: Option<u64>,
    error: Option<String>,
    metrics_key: Option<(u64, u64)>,
    metrics: SessionMetrics,
}
#[derive(Clone, Default)]
pub struct Store {
    pub bot_selection: crate::bot_universe::Selection,
    sector_history: snipers_core::sector_activity::History,
    pub selection_storage_ready: bool,
    pub tapes: BTreeMap<String, Tape>,
    pub generation: u64,
    pub date: String,
    selection: scanner_selection::Selection,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeWindow {
    pub symbol: String,
    pub period: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub finalized: bool,
    pub volume: f64,
    pub price: f64,
    pub change_pct: f64,
    pub bars: usize,
    pub volume_metrics: crate::top_five_metrics::VolumeMetrics,
}
fn volume_windows(tape: &Tape, symbol: &str, now: u64) -> Vec<VolumeWindow> {
    let Some(date) = calendar::date_at(now) else {
        return vec![];
    };
    let Day::Open(day) = calendar::schedule(date) else {
        return vec![];
    };
    let Some(start) = calendar::legacy_eastern_ms(&format!("{date} 04:00:00")) else {
        return vec![];
    };
    let cutoff = now / 60_000 * 60_000;
    let rolling_end = cutoff.min(day.close_ms);
    [
        (
            "volume10",
            rolling_end.saturating_sub(600_000).max(start),
            rolling_end,
        ),
        (
            "volume30",
            rolling_end.saturating_sub(1_800_000).max(start),
            rolling_end,
        ),
        ("premarket", start, day.open_ms),
        ("opening", day.open_ms, day.open_ms + 900_000),
    ]
    .into_iter()
    .filter_map(|(period, start, finish)| {
        if period.starts_with("volume") && cutoff > day.close_ms {
            return None;
        }
        let end = cutoff.min(finish);
        if end <= start {
            return None;
        }
        let bars: Vec<_> = tape.minutes.range(start..end).map(|(_, bar)| bar).collect();
        let first = bars.first()?;
        let last = bars.last()?;
        let volume: f64 = bars.iter().map(|bar| bar.volume).sum();
        if !volume.is_finite() || volume <= 0.0 || first.open <= 0.0 {
            return None;
        }
        Some(VolumeWindow {
            symbol: symbol.into(),
            period: period.into(),
            start_ms: start,
            end_ms: end,
            finalized: cutoff >= finish,
            volume,
            price: last.close,
            change_pct: (last.close / first.open - 1.0) * 100.0,
            bars: bars.len(),
            volume_metrics: crate::top_five_metrics::calculate(&tape.minutes, start, end),
        })
    })
    .collect()
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub premarket: Option<snipers_core::premarket::Read>,
    pub symbol: String,
    pub sector: Option<String>,
    pub status: String,
    pub bar_end_ms: Option<u64>,
    pub price: Option<f64>,
    pub volume_2m: Option<f64>,
    pub vwap: Option<f64>,
    pub rvol: Option<f64>,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub dealer_contexts: BTreeMap<String, crate::neuro_context::Context>,
    pub bot_selection: crate::bot_universe::Selection,
    pub sector_activity: snipers_core::sector_activity::Report,
    pub volume_windows: Vec<VolumeWindow>,
    pub confirmations: BTreeMap<String, snipers_core::scanner_memory::Read>,
    pub structure_confirmations: BTreeMap<String, snipers_core::scanner_structure::Read>,
    pub option_liquidity: BTreeMap<String, snipers_core::option_liquidity::Read>,
    pub options_context: BTreeMap<String, crate::scanner_options::Context>,
    pub flow_confirmations: BTreeMap<String, snipers_core::scanner_confirmation::FlowRead>,
    pub premarket_watch: Vec<snipers_core::premarket::Read>,
    pub selection_saved: bool,
    pub storage_error: Option<String>,
    pub rows: Vec<Row>,
    pub top: Vec<scanner::Candidate>,
    pub historical_ready: usize,
    pub total: usize,
    pub connected: bool,
    pub triggers: BTreeMap<String, scanner_trigger::Read>,
    pub selection: scanner_selection::Status,
}
impl Store {
    pub fn bot_open(&self,symbol:&str,now:u64)->Option<Bar>{
        let tape=self.tapes.get(symbol)?;if !tape.history_ready{return None;}
        let Day::Open(session)=calendar::schedule(calendar::date_at(now)?)else{return None;};
        (now>=session.open_ms+60000).then(||tape.minutes.get(&session.open_ms).cloned()).flatten()
    }
    pub fn bot_daily_range(&self,symbol:&str,now:u64)->Option<f64>{
        let tape=self.tapes.get(symbol)?;if !tape.history_ready{return None;}
        let today=calendar::date_at(now)?;let mut days:BTreeMap<String,(f64,f64,usize,usize)>=BTreeMap::new();
        for b in tape.minutes.values(){let date=calendar::date_at(b.timestamp_ms)?;if date>=today{continue;}
            let Day::Open(day)=calendar::schedule(date) else {continue};
            if b.timestamp_ms<day.open_ms||b.timestamp_ms>=day.close_ms{continue;}
            let row=days.entry(date.to_string()).or_insert((b.high,b.low,0,((day.close_ms-day.open_ms)/60000)as usize));
            row.0=row.0.max(b.high);row.1=row.1.min(b.low);row.2+=1;
        }
        let ranges:Vec<_>=days.values().rev().filter(|r|r.2*10>=r.3*9&&r.0>r.1).take(5).map(|r|r.0-r.1).collect();
        (ranges.len()>=3).then(||ranges.iter().sum::<f64>()/ranges.len() as f64)
    }
    pub fn bot_baselines(&self,symbol:&str,now:u64)->serde_json::Value{
        let Some(tape)=self.tapes.get(symbol).filter(|t|t.history_ready)else{return serde_json::Value::Null};
        let Some(today)=calendar::date_at(now)else{return serde_json::Value::Null};
        let prior=(1..=7).filter_map(|i|calendar::date_at(now.saturating_sub(i*86400000))).filter(|d|*d<today).find_map(|d|match calendar::schedule(d){Day::Open(day)=>Some(day),_=>None}).and_then(|day|tape.minutes.get(&(day.close_ms-60000)));
        let opening=self.bot_open(symbol,now);
        serde_json::json!({"priorClose":prior.map(|b|b.close),"priorAt":prior.map(|b|b.timestamp_ms),"open":opening.as_ref().map(|b|b.open),"openAt":opening.map(|b|b.timestamp_ms)})
    }
    pub fn bot_history(&self,symbol:&str,now:u64)->Option<(crate::metrics::SessionMetrics,Vec<Bar>)>{
        let tape=self.tapes.get(symbol)?;
        if !tape.history_ready {return None;}
        Some((metrics::calculate(&tape.minutes,now),tape.minutes.range(now.saturating_sub(10_800_000)..=now).map(|(_,b)|b.clone()).collect()))
    }
    pub fn bot_smc_history(&self,symbol:&str,now:u64)->Vec<Bar>{
        let Some(tape)=self.tapes.get(symbol).filter(|t|t.history_ready)else{return vec![];};
        let mut rows:Vec<_>=tape.minutes.range(..=now).rev().take(2400).map(|(_,b)|b.clone()).collect();rows.reverse();rows
    }
    pub fn restore_selection(&mut self, symbols: Vec<String>, locked_ms: u64, now: u64) -> bool {
        self.selection.restore(symbols, locked_ms, now)
    }
    pub fn initialize(&mut self, now: u64) {
        self.sector_history = Default::default();
        self.selection_storage_ready = true;
        self.selection = scanner_selection::Selection::default();
        self.tapes = universe::assets()
            .keys()
            .filter(|s| universe::stock_asset(s).is_some())
            .map(|s| (s.clone(), Tape::default()))
            .collect();
        self.date = calendar::date_at(now)
            .map(|d| d.to_string())
            .unwrap_or_default();
    }
    pub fn invalidate(&mut self) {
        self.sector_history = Default::default();
        self.generation += 1;
        for tape in self.tapes.values_mut() {
            tape.history_ready = false;
        }
    }
    pub fn receive(&mut self, event: &Event, received: u64) -> bool {
        let Event::MinuteBar {
            sym,
            o,
            h,
            l,
            c,
            v,
            s,
            e,
            vw,
        } = event
        else {
            return false;
        };
        let Some(tape) = self.tapes.get_mut(sym) else {
            return false;
        };
        // An unfinished aggregate is provisional, not a completed scanner candle.
        if *e > received {
            return false;
        }
        if !s.is_multiple_of(60_000)
            || *s > received
            || *e > received
            || !s
                .checked_add(59_999)
                .is_some_and(|end| *e >= end && *e <= end + 1)
            || [*o, *h, *l, *c].iter().any(|v| !v.is_finite() || *v <= 0.0)
            || !v.is_finite()
            || *v < 0.0
            || *h < o.max(*c)
            || *l > o.min(*c)
        {
            tape.history_ready = false;
            tape.error = Some("Vela inválida; recuperación pendiente".into());
            return false;
        }
        let Some(date) = calendar::date_at(*s) else {
            return false;
        };
        let Day::Open(day) = calendar::schedule(date) else {
            return false;
        };
        let Some(premarket_start) = calendar::legacy_eastern_ms(&format!("{date} 04:00:00")) else {
            return false;
        };
        if *s < premarket_start || *s >= day.close_ms {
            return false;
        }
        tape.revision += 1;
        tape.versions.insert(*s, tape.revision);
        tape.minutes.insert(
            *s,
            Bar {
                timestamp_ms: *s,
                open: *o,
                high: *h,
                low: *l,
                close: *c,
                volume: *v,
                vwap: vw.filter(|p| p.is_finite() && *p > 0.0),
            },
        );
        trim(tape);
        true
    }
    pub fn snapshot(&mut self, now: u64, connected: bool) -> Report {
        let mut report = Report {
            total: self.tapes.len(),
            connected,
            ..Default::default()
        };
        let mut inputs = vec![];
        let mut morning = vec![];
        for (symbol, tape) in &mut self.tapes {
            if tape.history_ready {
                let key = (tape.revision, now / 60_000);
                if tape.volume_key != Some(key) {
                    tape.volume_windows = volume_windows(tape, symbol, now);
                    tape.volume_key = Some(key);
                }
                report.volume_windows.extend(tape.volume_windows.clone());
            }
            let cutoff = calendar::date_at(now)
                .and_then(|date| calendar::legacy_eastern_ms(&format!("{date} 09:15:00")));
            if cutoff.is_some_and(|cutoff| now >= cutoff && now < cutoff + 900_000)
                && tape.history_ready
            {
                let minute = now / 60_000 * 60_000;
                if tape.morning_key != Some((tape.revision, minute)) {
                    tape.morning = Some(premarket_read(tape, symbol, minute, now));
                    tape.morning_key = Some((tape.revision, minute));
                }
                if let Some(row) = &tape.morning {
                    morning.push(row.clone());
                }
            } else {
                tape.morning = None;
                tape.morning_key = None;
            }
            if let Some(cutoff) = cutoff.filter(|cutoff| now >= *cutoff && tape.history_ready) {
                if tape.premarket_key != Some((tape.revision, cutoff)) {
                    tape.premarket = Some(premarket_read(tape, symbol, cutoff, now));
                    tape.premarket_key = Some((tape.revision, cutoff));
                }
            } else {
                tape.premarket = None;
                tape.premarket_key = None;
            }
            let key = (tape.revision, now / 120_000);
            if tape.metrics_key != Some(key) {
                tape.metrics = metrics::calculate(&tape.minutes, now);
                tape.metrics_key = Some(key);
            }
            let m = &tape.metrics;
            let pair = m.asof_ms.and_then(|end| {
                Some((
                    end,
                    tape.minutes.get(&end.checked_sub(120_000)?)?,
                    tape.minutes.get(&end.checked_sub(60_000)?)?,
                ))
            });
            let (end, price, volume) = pair
                .map(|(end, a, b)| (Some(end), Some(b.close), Some(a.volume + b.volume)))
                .unwrap_or_default();
            let mut row = Row {
                premarket: tape.premarket.clone(),
                symbol: symbol.clone(),
                sector: universe::stock_asset(symbol).and_then(|a| a.sector.clone()),
                status: if tape.history_ready {
                    "Histórico verificado".into()
                } else {
                    tape.error.clone().unwrap_or("Recuperando histórico".into())
                },
                bar_end_ms: end,
                price,
                volume_2m: volume,
                vwap: m.vwap,
                rvol: m.rvol,
            };
            if tape.history_ready {
                report.historical_ready += 1;
            }
            if let (Some(bar_end_ms), Some(price), Some(volume_2m)) = (end, price, volume) {
                let input = Input {
                    symbol: symbol.clone(),
                    bar_end_ms,
                    price,
                    volume_2m,
                    vwap: m.vwap,
                    rvol: m.rvol,
                    source_verified: tape.history_ready && connected,
                };
                if tape.history_ready && connected && scanner::candidate(&input, now).is_none() {
                    row.status = "Sin vela 2m vigente o sesión cerrada".into();
                }
                inputs.push(input);
            }
            if !connected {
                row.status = "Sin conexión vigente".into();
            }
            report.rows.push(row);
        }
        self.bot_selection.update(&report.volume_windows,now,connected && self.selection_storage_ready && report.total>0 && report.historical_ready==report.total);
        report.bot_selection=self.bot_selection.clone();
        report.premarket_watch = snipers_core::premarket::watch(&morning, now, connected);
        report.sector_activity = self.sector_history.update(&inputs, now, connected);
        report.selection = self.selection.update(
            &inputs,
            now,
            connected
                && self.selection_storage_ready
                && report.total > 0
                && report.historical_ready == report.total,
        );
        report.top = if report.selection.locked {
            let mut candidates: BTreeMap<_, _> = scanner::rank_all(&inputs, now)
                .into_iter()
                .map(|r| (r.symbol.clone(), r))
                .collect();
            report
                .selection
                .symbols
                .iter()
                .filter_map(|s| candidates.remove(s))
                .collect()
        } else {
            scanner::rank(&inputs, now)
        };
        for candidate in &report.top {
            if let Some(tape) = self.tapes.get(&candidate.symbol) {
                report
                    .triggers
                    .insert(candidate.symbol.clone(), trigger(tape, candidate, now));
            }
        }
        report
    }
}
fn trigger(tape: &Tape, candidate: &scanner::Candidate, now: u64) -> scanner_trigger::Read {
    let pending = |reason: &str| scanner_trigger::Read {
        reason: reason.into(),
        ..Default::default()
    };
    let end = candidate.bar_end_ms;
    if !tape.history_ready || now < end.saturating_add(2_000) || now - end > 120_000 {
        return pending("ESPERANDO CIERRE VERIFICADO 2M");
    }
    let Some(date) = calendar::date_at(end) else {
        return pending("CALENDARIO NO DISPONIBLE");
    };
    let Day::Open(day) = calendar::schedule(date) else {
        return pending("SESION NO DISPONIBLE");
    };
    if end < day.open_ms + 120_000 || end > day.close_ms {
        return pending("VELA FUERA DE SESION");
    }
    let pair = |finish: u64| -> Option<(&Bar, &Bar)> {
        Some((
            tape.minutes.get(&finish.checked_sub(120_000)?)?,
            tape.minutes.get(&finish.checked_sub(60_000)?)?,
        ))
    };
    let Some((a, b)) = pair(end) else {
        return pending("VELA 2M INCOMPLETA");
    };
    // The legacy baseline takes the last ten complete pairs in this session,
    // then excludes zero-volume pairs; gaps are never synthesized.
    let previous: Vec<_> = (day.open_ms + 120_000..end)
        .step_by(120_000)
        .filter_map(|finish| pair(finish).map(|(a, b)| a.volume + b.volume))
        .collect();
    let volumes: Vec<_> = previous
        .iter()
        .rev()
        .take(10)
        .copied()
        .filter(|v| v.is_finite() && *v > 0.0)
        .collect();
    let average = volumes.iter().sum::<f64>() / volumes.len().max(1) as f64;
    let volume = a.volume + b.volume;
    let ratio =
        (volumes.len() >= 3 && average.is_finite() && average > 0.0).then_some(volume / average);
    scanner_trigger::evaluate(&scanner_trigger::Input {
        direction: candidate.direction.clone(),
        open: Some(a.open),
        high: Some(a.high.max(b.high)),
        low: Some(a.low.min(b.low)),
        close: Some(b.close),
        vwap: candidate.vwap,
        volume: Some(volume),
        volume_ratio: ratio,
        baseline_bars: volumes.len(),
    })
}
fn premarket_read(
    tape: &Tape,
    symbol: &str,
    cutoff: u64,
    now: u64,
) -> snipers_core::premarket::Read {
    let minutes = tape
        .minutes
        .range(cutoff.saturating_sub(6 * 3_600_000)..cutoff)
        .map(|(_, b)| snipers_core::bars::Minute {
            t: b.timestamp_ms,
            o: b.open,
            h: b.high,
            l: b.low,
            c: b.close,
            v: b.volume,
            vw: b.vwap,
        })
        .collect::<Vec<_>>();
    snipers_core::premarket::read(symbol, cutoff, now, &minutes, tape.history_ready)
}
fn trim(tape: &mut Tape) {
    while tape.minutes.len() > 8_000 {
        if let Some((t, _)) = tape.minutes.pop_first() {
            tape.versions.remove(&t);
        }
    }
}
fn merge(tape: &mut Tape, bars: Vec<Bar>, revision: u64) {
    for bar in bars {
        let Some(date) = calendar::date_at(bar.timestamp_ms) else {
            continue;
        };
        let Day::Open(day) = calendar::schedule(date) else {
            continue;
        };
        let Some(premarket_start) = calendar::legacy_eastern_ms(&format!("{date} 04:00:00")) else {
            continue;
        };
        if bar.timestamp_ms < premarket_start || bar.timestamp_ms >= day.close_ms {
            continue;
        }
        if tape
            .versions
            .get(&bar.timestamp_ms)
            .is_none_or(|v| *v <= revision)
        {
            tape.minutes.insert(bar.timestamp_ms, bar);
        }
    }
    tape.revision += 1;
    trim(tape);
}
pub async fn worker(key: &Credentials, state: SharedState) {
    let notify = state.lock().unwrap().scanner_notify.clone();
    loop {
        tokio::select! {_=notify.notified()=>{},_=tokio::time::sleep(std::time::Duration::from_secs(60))=>{}}
        let (mut symbols, generation) = {
            let s = state.lock().unwrap();
            (
                s.scanner_store.tapes.keys().cloned().collect::<Vec<_>>(),
                s.scanner_store.generation,
            )
        };
        while !symbols.is_empty() {
            // Serve the visible asset and demanded basket before the alphabetical
            // universe. Re-read demand so a basket enabled during recovery need
            // not wait for every unrelated company's historical request.
            let index={let s=state.lock().unwrap();
                let mut priority=s.bot_held.clone();priority.push(s.symbol.clone());
                priority.extend(s.qqq_components.clone());
                let priority=crate::live::direction_pool_symbols(priority,&s.symbol,s.direction_enabled);
                priority.iter().find_map(|root|symbols.iter().position(|symbol|symbol==root)).unwrap_or(0)};
            let symbol=symbols.remove(index);
            let to = now_ms() / 60_000 * 60_000 - 1;
            let request = {
                let s = state.lock().unwrap();
                if generation != s.scanner_store.generation {
                    break;
                }
                s.scanner_store
                    .tapes
                    .get(&symbol)
                    .filter(|t| !t.history_ready)
                    .map(|t| {
                        (
                            t.through
                                .map(|end| end.saturating_sub(120_000) / 60_000 * 60_000)
                                .unwrap_or(to.saturating_sub(21 * 86_400_000) / 60_000 * 60_000)
                                .max(to.saturating_sub(21 * 86_400_000) / 60_000 * 60_000),
                            t.revision,
                        )
                    })
            };
            let Some((from, revision)) = request else {
                continue;
            };
            let result = crate::history::fetch(key, &symbol, from, to).await;
            let mut s = state.lock().unwrap();
            if generation != s.scanner_store.generation {
                break;
            }
            let Some(tape) = s.scanner_store.tapes.get_mut(&symbol) else {
                continue;
            };
            match result {
                Ok(bars) => {
                    merge(tape, bars, revision);
                    tape.history_ready = true;
                    tape.through = Some(to);
                    tape.error = None;
                }
                Err(error) => {
                    tape.history_ready = false;
                    tape.error = Some(error.into());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(symbol: &str, start: u64, volume: f64) -> Event {
        Event::MinuteBar {
            sym: symbol.into(),
            s: start,
            e: start + 60_000,
            o: 100.0,
            h: 102.0,
            l: 99.0,
            c: 101.0,
            v: volume,
            vw: Some(100.0),
        }
    }
    #[test]
    fn volume_windows_isolate_periods_corrections_and_date() {
        let open = calendar::legacy_eastern_ms("2026-09-24 09:30:00").unwrap();
        let mut store = Store::default();
        store.initialize(open);
        store.tapes.retain(|s, _| s == "QQQ");
        for (time, vol) in [
            (open - 60000, 100.),
            (open, 200.),
            (open + 840000, 300.),
            (open + 900000, 999.),
        ] {
            assert!(store.receive(&event("QQQ", time, vol), open + 960000));
        }
        assert!(store
            .snapshot(open + 960000, true)
            .volume_windows
            .is_empty());
        store.tapes.get_mut("QQQ").unwrap().history_ready = true;
        let r = store.snapshot(open + 960000, true);
        assert_eq!(r.volume_windows.len(), 4);
        assert_eq!(r.volume_windows[0].volume, 1299.);
        assert_eq!(r.volume_windows[1].volume, 1599.);
        assert_eq!(r.volume_windows[2].volume, 100.);
        assert_eq!(r.volume_windows[3].volume, 500.);
        assert!(r.volume_windows.iter().all(|r| r.finalized));
        assert!(store.receive(&event("QQQ", open, 250.), open + 960000));
        assert_eq!(
            store.snapshot(open + 960000, true).volume_windows[3].volume,
            550.
        );
        let r = store.snapshot(open + 60000, true);
        assert_eq!(r.volume_windows[3].volume, 250.);
        assert!(!r.volume_windows[3].finalized);
        assert!(store
            .snapshot(open + 86400000, true)
            .volume_windows
            .is_empty());
        let r = store.snapshot(open + 1_500_000, true);
        assert_eq!(r.volume_windows[0].volume, 999.);
        let close = calendar::legacy_eastern_ms("2026-09-24 16:01:00").unwrap();
        assert!(store
            .snapshot(close, true)
            .volume_windows
            .iter()
            .all(|r| !r.period.starts_with("volume")));
        store.invalidate();
        assert!(store
            .snapshot(open + 960000, true)
            .volume_windows
            .is_empty());
    }
    #[test]
    #[ignore = "full configured universe capacity audit; synthetic data, release mode"]
    fn configured_universe_opening_capacity_keeps_top_five_bounded() {
        let open = calendar::legacy_eastern_ms("2026-09-24 09:30:00").unwrap();
        let mut store = Store::default();
        store.initialize(open);
        let symbols: Vec<_> = store.tapes.keys().cloned().collect();
        assert!(!symbols.iter().any(|s| s == "SPX"));
        let started = std::time::Instant::now();
        for date in [
            "2026-09-17",
            "2026-09-18",
            "2026-09-21",
            "2026-09-22",
            "2026-09-23",
        ] {
            let start = calendar::legacy_eastern_ms(&format!("{date} 09:30:00")).unwrap();
            for minute in 0..390 {
                for symbol in &symbols {
                    assert!(store.receive(&event(symbol, start + minute * 60000, 10000.), open));
                }
            }
        }
        for tape in store.tapes.values_mut() {
            tape.history_ready = true;
        }
        let mut max_snapshot_us = 0;
        let mut payload_bytes = 0;
        for minute in 0..30 {
            let start = open + minute * 60000;
            let now = start + 62000;
            for symbol in &symbols {
                assert!(store.receive(&event(symbol, start, 40000.), now));
            }
            if minute % 2 == 1 {
                let t = std::time::Instant::now();
                let report = store.snapshot(now, true);
                max_snapshot_us = max_snapshot_us.max(t.elapsed().as_micros());
                assert_eq!(report.total, symbols.len());
                assert!(report.selection.symbols.len() <= 5);
                if minute >= 15 {
                    assert_eq!(report.selection.symbols.len(), 5);
                }
                assert!(!report.selection.entry_confirmed);
                payload_bytes = serde_json::to_vec(&report).unwrap().len();
            }
        }
        println!(
            "CAPACITY {}",
            serde_json::json!({"symbols":symbols.len(),"bars":symbols.len()*(5*390+30),"elapsedMs":started.elapsed().as_millis(),"maxSnapshotMs":max_snapshot_us as f64/1000.,"reportBytes":payload_bytes,"synthetic":true})
        );
    }
    #[test]
    fn full_rth_replay_keeps_frozen_membership_and_recovers_without_double_volume() {
        let symbols = ["QQQ", "SPY", "NVDA", "TSLA", "MU", "AMD"];
        let open = calendar::legacy_eastern_ms("2026-09-24 09:30:00").unwrap();
        let mut store = Store::default();
        store.initialize(open);
        store
            .tapes
            .retain(|symbol, _| symbols.contains(&symbol.as_str()));
        for date in [
            "2026-09-17",
            "2026-09-18",
            "2026-09-21",
            "2026-09-22",
            "2026-09-23",
        ] {
            let start = calendar::legacy_eastern_ms(&format!("{date} 09:30:00")).unwrap();
            for minute in 0..390 {
                for symbol in symbols {
                    assert!(store.receive(&event(symbol, start + minute * 60000, 10000.), open));
                }
            }
        }
        for tape in store.tapes.values_mut() {
            tape.history_ready = true;
        }
        let mut frozen = Vec::new();
        let mut locked = 0;
        for minute in 0..390 {
            let start = open + minute * 60000;
            let now = start + 62000;
            for (index, symbol) in symbols.iter().enumerate() {
                let volume = if *symbol == "AMD" && minute > 30 {
                    200000.
                } else {
                    60000. - index as f64 * 5000.
                };
                assert!(store.receive(&event(symbol, start, volume), now));
            }
            if minute == 119 {
                let count = store.tapes["QQQ"].minutes.len();
                assert!(store.receive(&event("QQQ", start, 70000.), now));
                assert_eq!(store.tapes["QQQ"].minutes.len(), count);
                assert_eq!(store.tapes["QQQ"].minutes[&start].volume, 70000.);
            }
            if minute == 179 {
                store.invalidate();
                assert!(store.snapshot(now, false).top.is_empty());
                assert!(store.snapshot(now, true).top.is_empty());
                assert_eq!(store.snapshot(now, true).selection.symbols, frozen);
                for tape in store.tapes.values_mut() {
                    tape.history_ready = true;
                }
            }
            if minute == 239 {
                let mut restarted = Store::default();
                restarted.initialize(now);
                restarted.tapes = store.tapes.clone();
                assert!(restarted.restore_selection(frozen.clone(), locked, now));
                store = restarted;
            }
            if minute % 2 == 1 {
                let report = store.snapshot(now, true);
                if minute < 14 {
                    assert!(!report.selection.locked);
                }
                if minute >= 15 {
                    if frozen.is_empty() {
                        frozen = report.selection.symbols.clone();
                        locked = report.selection.locked_at.unwrap();
                        assert_eq!(frozen.len(), 5);
                        assert!(!frozen.contains(&"AMD".into()));
                    }
                    assert_eq!(report.selection.symbols, frozen);
                    assert!(!report.selection.entry_confirmed);
                    assert!(report.top.iter().all(|c| !c.entry_confirmed));
                    if (31..389).contains(&minute) {
                        assert!(report.top.iter().all(|c| c.rvol.is_some() && c.base_ready));
                    }
                }
            }
        }
        assert!(store.snapshot(open + 390 * 60000, true).top.is_empty());
        for tape in store.tapes.values() {
            assert_eq!(tape.minutes.len(), 6 * 390);
        }
    }
    #[test]
    fn isolated_tapes_preserve_live_corrections_during_rest_and_invalidate_on_gaps() {
        let date = calendar::date_at(calendar::day_bounds("2026-09-24").unwrap().0).unwrap();
        let Day::Open(day) = calendar::schedule(date) else {
            panic!()
        };
        let now = day.open_ms + 120_000;
        let mut store = Store::default();
        store.initialize(now);
        assert!(!store.tapes.contains_key("SPX"));
        store.tapes.retain(|s, _| s == "QQQ" || s == "SPY");
        assert!(store.receive(&event("QQQ", day.open_ms, 10.0), now));
        assert!(store.receive(&event("QQQ", day.open_ms + 60_000, 20.0), now));
        assert!(store.snapshot(now, true).top.is_empty());
        let tape = store.tapes.get_mut("QQQ").unwrap();
        merge(
            tape,
            vec![Bar {
                timestamp_ms: day.open_ms,
                open: 100.0,
                high: 102.0,
                low: 99.0,
                close: 101.0,
                volume: 1.0,
                vwap: Some(100.0),
            }],
            0,
        );
        tape.history_ready = true;
        let r = store.snapshot(now, true);
        assert_eq!(r.top.len(), 1);
        assert_eq!(r.top[0].volume_2m, 30.0);
        assert_eq!(r.top[0].vwap, Some(100.0));
        assert_eq!(store.tapes["SPY"].minutes.len(), 0);
        assert!(store.receive(&event("QQQ", day.open_ms + 60_000, 25.0), now));
        assert_eq!(store.snapshot(now, true).top[0].volume_2m, 35.0);
        assert!(store.snapshot(now, false).top.is_empty());
        assert!(store.snapshot(now + 120_001, true).top.is_empty());
        assert!(!store.receive(&event("QQQ", day.open_ms + 120_000, 25.0), now));
        assert!(store.tapes["QQQ"].history_ready);
        assert!(!store.receive(&event("QQQ", day.open_ms, -1.0), now));
        assert!(store.snapshot(now, true).top.is_empty());
        assert!(store
            .snapshot(now, true)
            .rows
            .iter()
            .find(|r| r.symbol == "QQQ")
            .unwrap()
            .status
            .contains("inválida"));
        store.tapes.get_mut("QQQ").unwrap().history_ready = true;
        store.invalidate();
        assert_eq!(store.generation, 1);
        assert!(store.snapshot(now, true).top.is_empty());
    }

    #[test]
    fn premarket_cutoff_recovers_corrects_and_stays_separate_from_rth() {
        let cutoff = calendar::legacy_eastern_ms("2026-09-24 09:15:00").unwrap();
        let open = cutoff + 900_000;
        let mut store = Store::default();
        store.initialize(cutoff);
        store.tapes.retain(|s, _| s == "QQQ");
        for offset in [300_000, 180_000, 60_000] {
            assert!(store.receive(&event("QQQ", cutoff - offset, 10.0), cutoff));
        }
        assert!(store.snapshot(cutoff, true).rows[0].premarket.is_none());
        let tape = store.tapes.get_mut("QQQ").unwrap();
        merge(
            tape,
            vec![Bar {
                timestamp_ms: cutoff - 300_000,
                open: 100.0,
                high: 102.0,
                low: 99.0,
                close: 101.0,
                volume: 999.0,
                vwap: None,
            }],
            0,
        );
        tape.history_ready = true;
        let report = store.snapshot(cutoff, true);
        assert_eq!(
            report.rows[0].premarket.as_ref().unwrap().volume_5m,
            Some(30.0)
        );
        assert!(report.top.is_empty());
        assert!(store.receive(&event("QQQ", cutoff - 60_000, 20.0), cutoff));
        assert_eq!(
            store.snapshot(cutoff, true).rows[0]
                .premarket
                .as_ref()
                .unwrap()
                .volume_5m,
            Some(40.0)
        );
        for offset in [0, 60_000] {
            assert!(store.receive(&event("QQQ", open + offset, 5.0), open + 120_000));
        }
        let report = store.snapshot(open + 120_000, true);
        assert_eq!(report.top[0].volume_2m, 10.0);
        assert_eq!(
            report.rows[0].premarket.as_ref().unwrap().volume_5m,
            Some(40.0)
        );
        store.invalidate();
        assert!(store.snapshot(open + 120_000, true).rows[0]
            .premarket
            .is_none());
    }

    #[test]
    fn trigger_uses_three_previous_pairs_and_never_certifies_unsettled_or_missing_minutes() {
        let date = calendar::date_at(calendar::day_bounds("2026-09-24").unwrap().0).unwrap();
        let Day::Open(day) = calendar::schedule(date) else {
            panic!()
        };
        let end = day.open_ms + 480_000;
        let mut store = Store::default();
        store.initialize(end);
        store.tapes.retain(|s, _| s == "QQQ");
        for minute in 0..8 {
            assert!(store.receive(&event("QQQ", day.open_ms + minute * 60_000, 10.0), end));
        }
        store.tapes.get_mut("QQQ").unwrap().history_ready = true;
        assert!(!store.snapshot(end + 1999, true).triggers["QQQ"].ready);
        let report = store.snapshot(end + 2000, true);
        let read = &report.triggers["QQQ"];
        assert!(read.ready);
        assert_eq!(read.baseline_bars, 3);
        assert_eq!(read.volume_ratio, Some(1.0));
        assert!(!read.entry_confirmed);
        assert!(store.receive(&event("QQQ", end - 60_000, 1.0), end + 2000));
        assert!(!store.snapshot(end + 2000, true).triggers["QQQ"].ready);
        assert!(store.snapshot(end + 2000, false).triggers.is_empty());
        let tape = store.tapes.get_mut("QQQ").unwrap();
        tape.minutes.remove(&(end - 60_000));
        tape.revision += 1;
        assert!(store.snapshot(end + 2000, true).triggers.is_empty());
    }
}
