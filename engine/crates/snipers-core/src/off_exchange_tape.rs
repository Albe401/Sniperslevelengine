//! Bounded incremental session tape with atomic history merge and shared REST/WS identity.
use crate::{
    calendar,
    off_exchange::{self, Level, Print, Quote},
    off_exchange_trade::{Source, Trade},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

// Shared REST/engine bound. Active names can exceed 500k TRF prints in one day.
pub const SESSION_LIMIT: usize = 5_000_000;
const WINDOW_LIMIT: usize = 100_000;
const RETENTION_MS: u64 = 30 * 60_000;

// Engine::date validates symbol, SIP date and exchange before a Day is touched.
// Those values are constant for this map; storing them in every alias wasted
// several heap allocations per print. Preserve TRF scope for IDs and the
// original day-wide sequence scope.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DayIdentity {
    Id(u32, String),
    Sequence(u64),
}
fn day_identities(trade: &Trade) -> Vec<DayIdentity> {
    let mut keys = Vec::with_capacity(2);
    if let Some(id) = &trade.trade_id {
        keys.push(DayIdentity::Id(trade.trf_id, id.clone()));
    }
    if let Some(sequence) = trade.sequence {
        keys.push(DayIdentity::Sequence(sequence));
    }
    keys
}

#[derive(Clone, Default, Debug, Serialize)]
pub struct Diagnostics {
    pub duplicates: u64,
    pub conflicts: u64,
    pub unidentified: u64,
    pub rejected: u64,
    pub capacity_drops: u64,
    pub corrections: u64,
}
#[derive(Clone, Default)]
struct Day {
    // Closed REST-only days need summary output, not millions of retained IDs.
    // A later event invalidates this summary until a full REST replacement.
    archived: Option<Summary>,
    records: Vec<Trade>,
    block_indices: Vec<usize>,
    sides: Vec<off_exchange::Side>,
    excluded: BTreeSet<DayIdentity>,
    excluded_correction_rows: usize,
    identities: BTreeMap<DayIdentity, usize>,
    levels: Vec<Level>,
    indices: BTreeMap<u64, usize>,
    level_times: BTreeMap<u64, u64>,
    latest_ms: Option<u64>,
    live_count: usize,
    history_count: usize,
    history_complete: bool,
    incomplete: bool,
    top_prints: Vec<TopPrint>,
}
#[derive(Clone, Debug, Serialize)]
pub struct TopPrint {
    pub timestamp_ms: u64,
    pub source: Source,
    pub print: Print,
    pub conditions: Vec<u32>,
    pub trf_id: u32,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Summary {
    pub date: String,
    pub print_count: usize,
    pub live_count: usize,
    pub history_count: usize,
    pub history_complete: bool,
    pub excluded_correction_rows: usize,
    pub complete: bool,
    pub latest_ms: Option<u64>,
    pub total_notional: f64,
    pub total_shares: f64,
    pub pressure_pct: Option<f64>,
    pub pressure_coverage_pct: Option<f64>,
    pub top_levels: Vec<Level>,
    pub top_prints: Vec<TopPrint>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Window {
    pub minutes: u64,
    pub print_count: usize,
    pub total_notional: f64,
    pub complete: bool,
    pub top_levels: Vec<Level>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Snapshot {
    pub blocks: crate::block_pressure::Report,
    pub symbol: String,
    pub session: Summary,
    pub history: Vec<Summary>,
    pub windows: Vec<Window>,
    pub diagnostics: Diagnostics,
    pub last_live_ms: Option<u64>,
    pub live_fresh: bool,
}
#[derive(Clone, Default)]
pub struct Engine {
    block_quotes: BTreeMap<u64,Quote>,
    symbol: String,
    days: BTreeMap<String, Day>,
    quote: Option<Quote>,
    rolling: BTreeMap<(u64, u64), Print>,
    order: u64,
    diagnostics: Diagnostics,
    last_live_ms: Option<u64>,
    incomplete_until: u64,
    session_limit: usize,
}
enum Added {
    New(Print),
    Duplicate,
    Conflict,
    Missing,
    Capacity,
    Correction,
    Excluded,
    HistoricalInvalidated,
}
impl Day {
    fn add(
        &mut self,
        trade: Trade,
        quote: Option<Quote>,
        limit: usize,
    ) -> Result<Added, &'static str> {
        self.add_preserving(trade, quote, limit, None)
    }
    fn add_preserving(
        &mut self,
        trade: Trade,
        quote: Option<Quote>,
        limit: usize,
        observed: Option<off_exchange::Side>,
    ) -> Result<Added, &'static str> {
        if let Some(summary) = &mut self.archived {
            summary.complete = false;
            summary.history_complete = false;
            return Ok(Added::HistoricalInvalidated);
        }
        if trade.correction.is_some_and(|c| c != 0) {
            self.incomplete = true;
            return Ok(Added::Correction);
        }
        let keys = day_identities(&trade);
        if keys.is_empty() {
            self.incomplete = true;
            return Ok(Added::Missing);
        }
        if keys.iter().any(|key| self.excluded.contains(key)) {
            return Ok(Added::Excluded);
        }
        let mut found = None;
        for key in &keys {
            if let Some(&index) = self.identities.get(key) {
                if found.is_some_and(|old| old != index) || !self.records[index].compatible(&trade)
                {
                    self.incomplete = true;
                    return Ok(Added::Conflict);
                }
                found = Some(index);
            }
        }
        if let Some(index) = found {
            // Each canonical trade gains at most one ID and one sequence alias.
            // Retain newly supplied aliases to reject contradictory later aliases.
            let record = &mut self.records[index];
            if record.trade_id.is_none() {
                record.trade_id = trade.trade_id.clone();
            }
            if record.sequence.is_none() {
                record.sequence = trade.sequence;
            }
            if record.participant_ms.is_none(){record.participant_ms=trade.participant_ms;}
            // Link a previously absent alias so later replay with either ID is deduplicated.
            for key in keys {
                self.identities.insert(key, index);
            }
            return Ok(Added::Duplicate);
        }
        if self.records.len() >= limit {
            self.incomplete = true;
            return Ok(Added::Capacity);
        }
        let mut print = trade.to_print(quote).ok_or("Print inválido")?;
        if let Some(side) = observed {
            print.side = side;
        }
        let index = match self.indices.get(&print.level.to_bits()) {
            Some(&index) => index,
            None => {
                if self.levels.len() >= 20_000 {
                    self.incomplete = true;
                    return Ok(Added::Capacity);
                }
                let index = self.levels.len();
                self.indices.insert(print.level.to_bits(), index);
                self.levels.push(Level {
                    level: print.level,
                    ..Default::default()
                });
                index
            }
        };
        let row = &mut self.levels[index];
        if !(row.notional + print.notional).is_finite() || !(row.shares + print.shares).is_finite()
        {
            self.incomplete = true;
            return Err("Desbordamiento del tape");
        }
        row.prints += 1;
        row.shares += print.shares;
        row.notional += print.notional;
        row.largest_print = row.largest_print.max(print.notional);
        if self
            .level_times
            .get(&print.level.to_bits())
            .is_none_or(|old| trade.sip_ms >= *old)
        {
            row.last_time = print.time_et.clone();
            self.level_times.insert(print.level.to_bits(), trade.sip_ms);
        }
        match print.side {
            off_exchange::Side::Buy | off_exchange::Side::BuyLean => {
                row.buy_notional += print.notional
            }
            off_exchange::Side::Sell | off_exchange::Side::SellLean => {
                row.sell_notional += print.notional
            }
            _ => row.neutral_notional += print.notional,
        }
        self.latest_ms = Some(self.latest_ms.unwrap_or(0).max(trade.sip_ms));
        self.top_prints.push(TopPrint {
            timestamp_ms: trade.sip_ms,
            source: trade.source,
            print: print.clone(),
            conditions: trade.conditions.clone(),
            trf_id: trade.trf_id,
        });
        self.top_prints
            .sort_by(|a, b| b.print.notional.total_cmp(&a.print.notional));
        self.top_prints.truncate(10);
        if trade.source == Source::Live {
            self.live_count += 1;
        } else {
            self.history_count += 1;
        }
        for key in keys {
            self.identities.insert(key, self.records.len());
        }
        if print.notional>=off_exchange::MIN_NOTIONAL {self.block_indices.push(self.records.len());}
        self.records.push(trade);
        self.sides.push(print.side);
        Ok(Added::New(print))
    }
    fn summary(&self, date: String) -> Summary {
        if let Some(summary) = &self.archived {
            return summary.clone();
        }
        let mut levels = self.levels.clone();
        let valid = off_exchange::decorate(&mut levels).is_ok();
        let notional: f64 = levels.iter().map(|l| l.notional).sum();
        let shares: f64 = levels.iter().map(|l| l.shares).sum();
        let buy: f64 = levels.iter().map(|l| l.buy_notional).sum();
        let sell: f64 = levels.iter().map(|l| l.sell_notional).sum();
        let finite = valid && [notional, shares, buy, sell].iter().all(|v| v.is_finite());
        if !finite {
            levels.clear();
        }
        levels.truncate(10);
        Summary {
            date,
            print_count: self.records.len(),
            live_count: self.live_count,
            history_count: self.history_count,
            history_complete: self.history_complete,
            excluded_correction_rows: self.excluded_correction_rows,
            complete: self.history_complete && !self.incomplete && finite,
            latest_ms: self.latest_ms,
            total_notional: if finite { notional } else { 0.0 },
            total_shares: if finite { shares } else { 0.0 },
            pressure_pct: (finite && buy + sell > 0.0).then(|| (buy - sell) / (buy + sell) * 100.0),
            pressure_coverage_pct: (finite && notional > 0.0)
                .then(|| (buy + sell) / notional * 100.0),
            top_levels: levels,
            top_prints: self.top_prints.clone(),
        }
    }
}
impl Engine {
    pub fn new(symbol: &str) -> Result<Self, &'static str> {
        crate::universe::stock_asset(symbol).ok_or("Activo no habilitado")?;
        Ok(Self {
            symbol: symbol.into(),
            session_limit: SESSION_LIMIT,
            ..Default::default()
        })
    }
    pub fn quote(&mut self, quote: Quote) {
        if self
            .quote
            .is_none_or(|old| quote.timestamp_ms >= old.timestamp_ms)
        {
            self.quote = Some(quote);
        }
    }
    pub fn block_quote(&mut self,at:u64,quote:Quote){
        if quote.timestamp_ms<=at&&at-quote.timestamp_ms<=off_exchange::QUOTE_MAX_AGE_MS{
            self.block_quotes.insert(at,quote);
        }
    }
    fn date(&self, trade: &Trade, now: u64) -> Result<String, &'static str> {
        if trade.symbol != self.symbol
            || trade.exchange != 4
            || trade.sip_ms > now
            || now - trade.sip_ms > 14 * 86_400_000
            || !trade.price.is_finite()
            || trade.price <= 0.0
            || !trade.shares.is_finite()
            || trade.shares <= 0.0
            || !(trade.price * trade.shares).is_finite()
        {
            return Err("Trade fuera del universo/fecha/dominio");
        }
        calendar::date_at(trade.sip_ms)
            .map(|d| d.to_string())
            .ok_or("Fecha inválida")
    }
    fn record_result(&mut self, result: Added, t: u64, now: u64) {
        if matches!(
            result,
            Added::Conflict | Added::Missing | Added::Capacity | Added::Correction
        ) {
            // Counters are lifetime diagnostics. Coverage gaps expire with the
            // affected rolling interval; they must not poison all future days.
            self.incomplete_until = self.incomplete_until.max(now.saturating_add(RETENTION_MS));
        }
        match result {
            Added::New(print)
                if print.notional >= off_exchange::MIN_NOTIONAL
                    && t <= now
                    && now - t <= RETENTION_MS =>
            {
                self.order += 1;
                self.rolling.insert((t, self.order), print);
                while self.rolling.len() > WINDOW_LIMIT {
                    if let Some(((t, _), _)) = self.rolling.pop_first() {
                        self.incomplete_until = self.incomplete_until.max(t + RETENTION_MS);
                        self.diagnostics.capacity_drops += 1;
                    }
                }
            }
            Added::Duplicate => self.diagnostics.duplicates += 1,
            Added::Conflict => self.diagnostics.conflicts += 1,
            Added::Missing => self.diagnostics.unidentified += 1,
            Added::Capacity => self.diagnostics.capacity_drops += 1,
            Added::Correction => self.diagnostics.corrections += 1,
            _ => {}
        }
    }
    pub fn trade(&mut self, trade: Trade, now: u64) {
        let Ok(date) = self.date(&trade, now) else {
            self.diagnostics.rejected += 1;
            self.incomplete_until = self.incomplete_until.max(now.saturating_add(RETENTION_MS));
            // A rejected live print is a coverage gap even if the REST batch succeeded.
            // Its date may be untrustworthy, so invalidate the receiving session.
            if let Some(date) = calendar::date_at(now) {
                self.days.entry(date.to_string()).or_default().incomplete = true;
            }
            return;
        };
        let t = trade.sip_ms;
        if trade.source == Source::Live {
            self.last_live_ms = Some(self.last_live_ms.unwrap_or(0).max(t));
        }
        let result = self
            .days
            .entry(date)
            .or_default()
            .add(trade, self.quote, self.session_limit);
        match result {
            Ok(result) => self.record_result(result, t, now),
            Err(_) => self.diagnostics.rejected += 1,
        };
        self.prune(now);
    }
    /// Only call after every requested page was downloaded and normalized successfully.
    /// Failure leaves the pre-existing live day untouched and cannot certify history.
    pub fn history(
        &mut self,
        date: &str,
        trades: Vec<Trade>,
        now: u64,
        pagination_complete: bool,
    ) -> Result<(), &'static str> {
        if !pagination_complete || trades.len() > self.session_limit {
            return Err("Historial parcial o capacidad excedida");
        }
        let parsed =
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| "Fecha inválida")?;
        let today = calendar::date_at(now).ok_or("Reloj inválido")?;
        if parsed > today || (today - parsed).num_days() > 14 {
            return Err("Historial fuera de rango");
        }
        // Rebuild a compacted closed day from the complete authoritative batch.
        let mut candidate = self
            .days
            .get(date)
            .filter(|day| day.archived.is_none())
            .cloned()
            .unwrap_or_default();
        let mut excluded = candidate.excluded.clone();
        let mut corrected_rows = 0;
        for trade in &trades {
            if trade.source != Source::History || self.date(trade, now)? != date {
                return Err("Historial de otro activo/fecha/origen");
            }
            match trade.correction {
                None | Some(0) => {}
                Some(1 | 8 | 10 | 12) => {
                    let keys = day_identities(trade);
                    if keys.is_empty() {
                        return Err("Corrección sin identidad verificable");
                    }
                    // A cancellation arriving on another date cannot be applied
                    // to that date's reused IDs. Keep it unverified until both
                    // original-day records and their marks are reconciled.
                    if trade
                        .participant_ms
                        .and_then(calendar::date_at)
                        .is_some_and(|day| day.to_string() != date)
                    {
                        return Err(
                            "Corrección entre sesiones requiere reconciliar el día original",
                        );
                    }
                    excluded.extend(keys);
                    corrected_rows += 1;
                }
                Some(_) => return Err("Historial con correcciones no resueltas"),
            }
        }
        if excluded.len() > self.session_limit * 2 {
            return Err("Correcciones exceden capacidad de sesión");
        }
        let rebuilt = excluded != candidate.excluded;
        if rebuilt {
            let old = candidate;
            candidate = Day {
                excluded,
                incomplete: old.incomplete,
                ..Default::default()
            };
            for (trade, side) in old.records.into_iter().zip(old.sides) {
                match candidate.add_preserving(trade, None, self.session_limit, Some(side))? {
                    Added::New(_) | Added::Excluded => {}
                    _ => return Err("No se pudo reconciliar la corrección"),
                }
            }
        }
        candidate.excluded_correction_rows = corrected_rows;
        let mut results = VecDeque::new();
        for trade in trades {
            if trade.source != Source::History || self.date(&trade, now)? != date {
                return Err("Historial de otro activo/fecha/origen");
            }
            // Massive documents 1/8/10/12 as original/cancel/correction rows;
            // all marked rows are excluded from its aggregate calculations.
            if trade.correction.is_some_and(|code| code != 0) {
                continue;
            }
            let t = trade.sip_ms;
            let result = candidate.add(trade, None, self.session_limit)?;
            match &result {
                Added::Conflict => return Err("Historial con identidades conflictivas"),
                Added::Correction => return Err("Historial con correcciones no resueltas"),
                Added::Missing => return Err("Historial con operaciones sin identidad"),
                Added::Capacity => return Err("Historial excede capacidad de sesión"),
                _ => {}
            }
            if let Added::New(print) = &result {
                if print.notional >= off_exchange::MIN_NOTIONAL && now - t <= RETENTION_MS {
                    results.push_back((result, t));
                }
            } else {
                results.push_back((result, t));
            }
        }
        candidate.history_complete = true;
        if rebuilt {
            let prints = candidate
                .records
                .iter()
                .zip(&candidate.sides)
                .filter(|(trade, _)| {
                    trade.sip_ms <= now
                        && now - trade.sip_ms <= RETENTION_MS
                        && trade.price * trade.shares >= off_exchange::MIN_NOTIONAL
                })
                .map(|(trade, side)| {
                    let mut print = trade
                        .to_print(None)
                        .ok_or("Print inválido al reconciliar")?;
                    print.side = *side;
                    Ok((trade.sip_ms, print))
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            self.rolling.retain(|(timestamp, _), _| {
                calendar::date_at(*timestamp).is_none_or(|day| day.to_string() != date)
            });
            for (timestamp, print) in prints {
                self.record_result(Added::New(print), timestamp, now);
            }
        }
        if parsed < today
            && candidate.live_count == 0
            && candidate
                .latest_ms
                .is_none_or(|t| now.saturating_sub(t) > RETENTION_MS)
        {
            candidate = Day {
                archived: Some(candidate.summary(date.into())),
                ..Default::default()
            };
        }
        self.days.insert(date.into(), candidate);
        for (result, t) in results {
            if rebuilt && matches!(result, Added::New(_)) {
                continue;
            }
            self.record_result(result, t, now);
        }
        self.prune(now);
        Ok(())
    }
    fn prune(&mut self, now: u64) {
        while self
            .rolling
            .first_key_value()
            .is_some_and(|((t, _), _)| now.saturating_sub(*t) > RETENTION_MS)
        {
            self.rolling.pop_first();
        }
        if let Some(today) = calendar::date_at(now) {
            self.days.retain(|date, _| {
                chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                    .is_ok_and(|d| (today - d).num_days() <= 14)
            });
        }
        while self.days.len() > 6 {
            self.days.pop_first();
        }
    }
    pub fn snapshot(&mut self, now: u64) -> Snapshot {
        self.prune(now);
        let date = calendar::date_at(now)
            .map(|d| d.to_string())
            .unwrap_or_default();
        let session = self.days.get(&date).map_or_else(
            || Day::default().summary(date.clone()),
            |day| day.summary(date.clone()),
        );
        let history = self
            .days
            .iter()
            .rev()
            .filter(|(d, _)| *d < &date)
            .take(5)
            .map(|(d, s)| s.summary(d.clone()))
            .collect();
        let windows = [1, 5, 15, 30]
            .into_iter()
            .map(|minutes| {
                let events: Vec<_> = self
                    .rolling
                    .range((now.saturating_sub(minutes * 60_000), 0)..=(now, u64::MAX))
                    .map(|(_, e)| e.clone())
                    .collect();
                let levels = off_exchange::build_levels(&events);
                let complete = levels.is_ok() && now > self.incomplete_until;
                Window {
                    minutes,
                    print_count: events.len(),
                    total_notional: events.iter().map(|e| e.notional).sum(),
                    complete,
                    top_levels: levels.unwrap_or_default().into_iter().take(10).collect(),
                }
            })
            .collect();
        let blocks=crate::block_pressure::build(&self.symbol,now,session.complete,
            self.days.get(&date).into_iter().flat_map(|day|day.block_indices.iter().map(|&i|&day.records[i]))
            .map(|trade|(trade,crate::block_pressure::execution(trade).and_then(|at|self.block_quotes.get(&at).copied()))));
        self.block_quotes.retain(|at,_|calendar::date_at(*at).is_some_and(|d|d.to_string()==date));
        Snapshot {
            blocks,
            symbol: self.symbol.clone(),
            session,
            history,
            windows,
            diagnostics: self.diagnostics.clone(),
            last_live_ms: self.last_live_ms,
            live_fresh: self
                .last_live_ms
                .is_some_and(|t| t <= now && now - t <= 15_000),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::off_exchange_trade::normalize;
    use serde_json::json;
    const NOW: u64 = 1_790_258_400_000;
    fn trade(id: u64, source: Source) -> Trade {
        let mut trade=normalize(&json!({"ev":"T","sym":"QQQ","x":4,"trfi":12,"i":id.to_string(),"q":id,"p":500.0,"s":300,"t":NOW,"pt":NOW}),"QQQ",Source::Live).unwrap().unwrap();
        trade.source = source;
        trade
    }
    #[test]
    fn twenty_sessions_of_mixed_live_and_rest_keep_five_days_and_no_cross_day_aliases() {
        let mut engine = Engine::new("QQQ").unwrap();
        let mut date = chrono::NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let mut sessions = 0;
        while sessions < 20 {
            let calendar::Day::Open(day) = calendar::schedule(date) else {
                date = date.succ_opt().unwrap();
                continue;
            };
            let mut history = vec![];
            for id in 1..=200 {
                let mut row = trade(id, Source::Live);
                row.sip_ms = day.open_ms + id * 1000;
                row.participant_ms = Some(row.sip_ms);
                engine.quote(Quote {
                    bid: 499.0,
                    ask: 500.0,
                    timestamp_ms: row.sip_ms,
                });
                engine.trade(row.clone(), row.sip_ms);
                row.source = Source::History;
                history.push(row);
            }
            let now = day.open_ms + 201_000;
            engine
                .history(&day.date, history.clone(), now, true)
                .unwrap();
            let report = engine.snapshot(now);
            assert!(report.session.complete);
            assert_eq!(report.session.print_count, 200);
            assert_eq!(report.session.total_shares, 60_000.0);
            assert_eq!(report.session.pressure_pct, Some(100.0));
            assert_eq!(report.windows[1].print_count, 200);
            assert_eq!(report.history.len(), sessions.min(5));
            assert!(report
                .history
                .iter()
                .all(|h| h.complete && h.print_count == 200 && h.pressure_pct == Some(100.0)));
            assert!(engine.days.len() <= 6);
            assert!(engine.days.values().map(|d| d.records.len()).sum::<usize>() <= 1200);
            // A broken refresh after many days must preserve the valid tape.
            let before = serde_json::to_value(&report).unwrap();
            history[150].symbol = "SPY".into();
            assert!(engine.history(&day.date, history, now, true).is_err());
            assert_eq!(before, serde_json::to_value(engine.snapshot(now)).unwrap());
            sessions += 1;
            date = date.succ_opt().unwrap();
        }
    }
    #[test]
    fn previous_session_diagnostics_do_not_permanently_poison_a_new_clean_day() {
        for failure in ["rejected", "missing", "conflict", "capacity", "correction"] {
            let mut engine = Engine::new("QQQ").unwrap();
            engine.session_limit = 2;
            let date = calendar::date_at(NOW).unwrap().to_string();
            engine
                .history(
                    &date,
                    vec![trade(1, Source::History), trade(2, Source::History)],
                    NOW,
                    true,
                )
                .unwrap();
            let mut broken = trade(3, Source::Live);
            match failure {
                "rejected" => broken.symbol = "SPY".into(),
                "missing" => {
                    broken.trade_id = None;
                    broken.sequence = None;
                }
                "conflict" => {
                    broken = trade(1, Source::Live);
                    broken.price = 501.0;
                }
                "correction" => broken.correction = Some(10),
                _ => {}
            }
            engine.trade(broken, NOW);
            assert!(!engine.snapshot(NOW).session.complete, "{failure}");
            let next = NOW + 86_400_000;
            let next_date = calendar::date_at(next).unwrap().to_string();
            let mut clean = trade(1, Source::History);
            clean.sip_ms = next;
            clean.participant_ms = Some(next);
            engine.history(&next_date, vec![clean], next, true).unwrap();
            let recovered = engine.snapshot(next);
            assert!(
                recovered.session.complete,
                "previous {failure} poisoned new day"
            );
            assert!(
                recovered.windows[0].complete,
                "previous {failure} poisoned new window"
            );
            assert!(
                !recovered.history[0].complete,
                "old incomplete day must remain visible"
            );
        }
    }
    #[test]
    fn compact_identity_keeps_trf_ids_distinct_and_sequence_conflicts_atomic() {
        let date = calendar::date_at(NOW).unwrap().to_string();
        let mut a = trade(1, Source::History);
        a.sequence = None;
        let mut b = a.clone();
        b.trf_id += 1;
        let mut engine = Engine::new("QQQ").unwrap();
        engine
            .history(&date, vec![a.clone(), b.clone()], NOW, true)
            .unwrap();
        assert_eq!(engine.snapshot(NOW).session.print_count, 2);
        a.sequence = Some(42);
        b.sequence = Some(42);
        let mut engine = Engine::new("QQQ").unwrap();
        assert!(engine.history(&date, vec![a, b], NOW, true).is_err());
        assert_eq!(engine.snapshot(NOW).session.print_count, 0);
    }
    #[test]
    fn closed_rest_days_release_raw_rows_and_replace_without_double_counting() {
        let mut engine = Engine::new("QQQ").unwrap();
        let date = calendar::date_at(NOW).unwrap().to_string();
        let later = NOW + 86_400_000;
        let rows = vec![trade(1, Source::History), trade(2, Source::History)];
        engine.history(&date, rows.clone(), later, true).unwrap();
        assert!(engine.days[&date].records.is_empty());
        assert!(engine.days[&date].identities.is_empty());
        let first = serde_json::to_value(&engine.snapshot(later).history[0]).unwrap();
        engine
            .history(&date, rows.clone(), later + 1, true)
            .unwrap();
        assert_eq!(
            first,
            serde_json::to_value(&engine.snapshot(later + 1).history[0]).unwrap()
        );
        let mut wrong = trade(3, Source::History);
        wrong.symbol = "SPY".into();
        assert!(engine.history(&date, vec![wrong], later, true).is_err());
        assert_eq!(
            first,
            serde_json::to_value(&engine.snapshot(later).history[0]).unwrap()
        );
        engine.trade(trade(3, Source::Live), later);
        let invalidated = engine.snapshot(later);
        assert!(!invalidated.history[0].history_complete);
        assert!(!invalidated.history[0].complete);
        assert_eq!(invalidated.history[0].print_count, 2);
        let mut recovered = rows;
        recovered.push(trade(3, Source::History));
        engine.history(&date, recovered, later, true).unwrap();
        let fixed = engine.snapshot(later);
        assert!(fixed.history[0].complete);
        assert_eq!(fixed.history[0].print_count, 3);
        assert_eq!(fixed.history[0].total_shares, 900.0);
    }
    #[test]
    fn closed_days_with_observed_live_sides_keep_deduplication_evidence() {
        let mut engine = Engine::new("QQQ").unwrap();
        let date = calendar::date_at(NOW).unwrap().to_string();
        engine.quote(Quote {
            bid: 499.0,
            ask: 500.0,
            timestamp_ms: NOW,
        });
        engine.trade(trade(1, Source::Live), NOW);
        engine
            .history(
                &date,
                vec![trade(1, Source::History)],
                NOW + 86_400_000,
                true,
            )
            .unwrap();
        assert!(engine.days[&date].archived.is_none());
        let result = engine.snapshot(NOW + 86_400_000);
        assert_eq!(result.history[0].print_count, 1);
        assert_eq!(result.history[0].pressure_pct, Some(100.0));
    }
    #[test]
    #[ignore = "large-session capacity audit; run explicitly in release mode"]
    fn history_above_previous_half_million_limit_is_complete_and_deduplicated() {
        let mut engine = Engine::new("QQQ").unwrap();
        let count = 500_001;
        let date = calendar::date_at(NOW).unwrap().to_string();
        let base = trade(0, Source::History);
        let rows = (0..count)
            .map(|id| {
                let mut row = base.clone();
                row.trade_id = Some(id.to_string());
                row.sequence = Some(id as u64);
                row
            })
            .collect();
        // Keep the stress input outside the rolling 30-minute window.
        engine
            .history(&date, rows, NOW + RETENTION_MS + 1, true)
            .unwrap();
        let report = engine.snapshot(NOW + RETENTION_MS + 1);
        assert!(report.session.complete);
        assert_eq!(report.session.print_count, count);
        assert_eq!(report.session.total_shares, count as f64 * 300.0);
        engine
            .history(&date, vec![base], NOW + RETENTION_MS + 1, true)
            .unwrap();
        let replay = engine.snapshot(NOW + RETENTION_MS + 1);
        assert_eq!(replay.session.print_count, count);
        assert_eq!(replay.diagnostics.duplicates, 1);
        assert_eq!(replay.diagnostics.capacity_drops, 0);
    }
    #[test]
    fn atomic_history_merge_preserves_live_side_and_deduplicates_both_orders() {
        for first_live in [true, false] {
            let mut engine = Engine::new("QQQ").unwrap();
            let date = calendar::date_at(NOW).unwrap().to_string();
            engine.quote(Quote {
                bid: 499.0,
                ask: 500.0,
                timestamp_ms: NOW,
            });
            if first_live {
                engine.trade(trade(1, Source::Live), NOW);
            }
            engine
                .history(
                    &date,
                    vec![trade(1, Source::History), trade(2, Source::History)],
                    NOW,
                    true,
                )
                .unwrap();
            engine.trade(trade(1, Source::Live), NOW);
            let result = engine.snapshot(NOW);
            assert_eq!(result.session.print_count, 2);
            assert_eq!(result.session.total_notional, 300_000.0);
            assert_eq!(result.windows[0].print_count, 2);
            assert!(result.session.complete);
            assert_eq!(result.session.live_count, usize::from(first_live));
            assert_eq!(
                result.session.pressure_coverage_pct,
                Some(if first_live { 50.0 } else { 0.0 })
            );
            let mut wrong = trade(1, Source::History);
            wrong.price = 501.0;
            assert!(engine
                .history(&date, vec![trade(3, Source::History), wrong], NOW, true)
                .is_err());
            assert_eq!(engine.snapshot(NOW).session.print_count, 2);
            assert!(engine
                .history(&date, vec![trade(4, Source::History)], NOW, false)
                .is_err());
            assert_eq!(engine.snapshot(NOW).session.print_count, 2);
        }
    }
    #[test]
    fn capacity_conflicts_rollover_and_fractional_identity_are_bounded() {
        let mut engine = Engine::new("QQQ").unwrap();
        engine.session_limit = 2;
        engine.trade(trade(1, Source::Live), NOW);
        engine.trade(trade(2, Source::Live), NOW);
        engine.trade(trade(3, Source::Live), NOW);
        let mut conflict = trade(1, Source::Live);
        conflict.shares = 300.25;
        engine.trade(conflict, NOW);
        let result = engine.snapshot(NOW);
        assert_eq!(result.session.print_count, 2);
        assert_eq!(result.diagnostics.capacity_drops, 1);
        assert_eq!(result.diagnostics.conflicts, 1);
        assert!(!result.windows[0].complete);
        let next = engine.snapshot(NOW + 86_400_000);
        assert_eq!(next.session.print_count, 0);
        assert_eq!(next.history.len(), 1);
        assert!(!next.live_fresh);
        assert!(next.windows.iter().all(|w| w.print_count == 0));
    }
    #[test]
    fn rejected_live_print_invalidates_a_previously_reconciled_session() {
        for variant in 0..3 {
            let mut engine = Engine::new("QQQ").unwrap();
            let date = calendar::date_at(NOW).unwrap().to_string();
            engine
                .history(&date, vec![trade(1, Source::History)], NOW, true)
                .unwrap();
            assert!(engine.snapshot(NOW).session.complete);
            let mut invalid = trade(2, Source::Live);
            match variant {
                0 => invalid.sip_ms = NOW + 1,
                1 => invalid.price = f64::NAN,
                _ => invalid.shares = 0.0,
            }
            engine.trade(invalid, NOW);
            let result = engine.snapshot(NOW);
            assert_eq!(result.session.print_count, 1);
            assert_eq!(result.diagnostics.rejected, 1);
            assert!(!result.session.complete);
            assert!(result.windows.iter().all(|w| !w.complete));
        }
    }
    #[test]
    fn unresolved_corrections_reject_the_entire_batch_without_mutating_live_tape() {
        for code in [7, 11, 999] {
            let mut engine = Engine::new("QQQ").unwrap();
            engine.trade(trade(1, Source::Live), NOW);
            let before = serde_json::to_value(engine.snapshot(NOW)).unwrap();
            let date = calendar::date_at(NOW).unwrap().to_string();
            let mut corrected = trade(3, Source::History);
            corrected.correction = Some(code);
            assert_eq!(
                engine.history(&date, vec![trade(2, Source::History), corrected], NOW, true),
                Err("Historial con correcciones no resueltas")
            );
            assert_eq!(serde_json::to_value(engine.snapshot(NOW)).unwrap(), before);
            assert!(!engine.snapshot(NOW).session.history_complete);
        }
    }
    #[test]
    fn documented_correction_pairs_remove_live_prints_and_rolling_volume_atomically() {
        for (original_code, replacement_code) in [(8, 10), (1, 12)] {
            let mut engine = Engine::new("QQQ").unwrap();
            let date = calendar::date_at(NOW).unwrap().to_string();
            engine.quote(Quote {
                bid: 499.,
                ask: 500.,
                timestamp_ms: NOW,
            });
            engine.trade(trade(1, Source::Live), NOW);
            engine.trade(trade(2, Source::Live), NOW);
            let before = engine.snapshot(NOW);
            assert_eq!(before.session.total_shares, 600.);
            let mut original = trade(1, Source::History);
            original.correction = Some(original_code);
            let mut replacement = original.clone();
            replacement.correction = Some(replacement_code);
            replacement.sip_ms = NOW + 1000;
            let rows = vec![
                trade(1, Source::History),
                original,
                replacement,
                trade(2, Source::History),
            ];
            for _ in 0..2 {
                engine
                    .history(&date, rows.clone(), NOW + 1000, true)
                    .unwrap();
                let result = engine.snapshot(NOW + 1000);
                assert!(result.session.complete);
                assert_eq!(result.session.print_count, 1);
                assert_eq!(result.session.total_shares, 300.);
                assert_eq!(result.session.excluded_correction_rows, 2);
                assert_eq!(result.session.pressure_pct, Some(100.));
                assert_eq!(result.windows[0].print_count, 1);
                assert_eq!(result.windows[0].total_notional, 150000.);
                assert_eq!(result.diagnostics.corrections, 0);
            }
            engine.trade(trade(1, Source::Live), NOW + 1001);
            assert_eq!(engine.snapshot(NOW + 1001).session.print_count, 1);
            let before = serde_json::to_value(engine.snapshot(NOW + 1001)).unwrap();
            let mut unknown = trade(2, Source::History);
            unknown.correction = Some(999);
            assert!(engine
                .history(&date, vec![unknown], NOW + 1001, true)
                .is_err());
            assert_eq!(
                serde_json::to_value(engine.snapshot(NOW + 1001)).unwrap(),
                before
            );
        }
    }
    #[test]
    fn aliases_are_bounded_and_late_events_keep_per_level_time() {
        let mut engine = Engine::new("QQQ").unwrap();
        let mut original = trade(1, Source::Live);
        original.trade_id = None;
        engine.trade(original, NOW);
        engine.trade(trade(1, Source::Live), NOW);
        let mut bad = trade(1, Source::Live);
        bad.trade_id = Some("different".into());
        engine.trade(bad, NOW);
        let date = calendar::date_at(NOW).unwrap().to_string();
        assert_eq!(engine.days[&date].identities.len(), 2);
        assert_eq!(engine.snapshot(NOW).diagnostics.conflicts, 1);
        let mut late = trade(2, Source::Live);
        late.sip_ms = NOW - 1000;
        late.price = 501.0;
        engine.trade(late, NOW);
        let result = engine.snapshot(NOW);
        assert!(result
            .session
            .top_levels
            .iter()
            .all(|l| !l.last_time.is_empty()));
    }
}
