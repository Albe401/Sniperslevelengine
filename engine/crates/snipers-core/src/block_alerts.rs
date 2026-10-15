//! Alert decisions for verified off-exchange stock blocks. The caller owns
//! polling, persistence, and notification delivery; this engine has no side effects.
use crate::{calendar, investment_structure::Block};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MAX_BATCH: usize = 10;
const MAX_SEEN: usize = 50_000;
const MAX_HISTORY: usize = 5_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    Selected(Vec<String>),
    Watchlist,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub scope: Scope,
    pub minimum_notional: f64,
    pub alert_buy: bool,
    pub alert_sell: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub key: String,
    pub symbol: String,
    pub side: Side,
    pub execution_ms: u64,
    pub price: f64,
    pub shares: f64,
    pub notional: f64,
    /// Classification is inferred from the print against a verified NBBO.
    pub nbbo: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredAlert {
    pub alert: Alert,
    pub received_ms: u64,
    pub read: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Engine {
    date: String,
    cursor: usize,
    seen: BTreeSet<String>,
    #[serde(default)]
    history: Vec<StoredAlert>,
}

fn valid_symbol(symbol: &str) -> bool {
    !symbol.is_empty()
        && symbol.len() <= 12
        && symbol
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
}

fn symbols(config: &Config, watchlist: &[String]) -> Result<Vec<String>, String> {
    let selected = match &config.scope {
        Scope::Selected(selected) => selected,
        Scope::Watchlist => watchlist,
    };
    if selected.is_empty() {
        return Err("Selecciona activos o carga la watchlist".into());
    }
    let mut unique = BTreeSet::new();
    for raw in selected {
        let symbol = raw.trim().to_ascii_uppercase();
        if !valid_symbol(&symbol) {
            return Err(format!("Símbolo inválido: {raw}"));
        }
        unique.insert(symbol);
    }
    Ok(unique.into_iter().collect())
}

impl Engine {
    pub fn history(&self) -> &[StoredAlert] {
        &self.history
    }

    pub fn unread_count(&self) -> usize {
        self.history.iter().filter(|item| !item.read).count()
    }

    pub fn mark_read(&mut self, key: &str) -> bool {
        let Some(item) = self.history.iter_mut().find(|item| item.alert.key == key) else {
            return false;
        };
        item.read = true;
        true
    }

    pub fn mark_all_read(&mut self) {
        for item in &mut self.history {
            item.read = true;
        }
    }

    pub fn validate(config: &Config, watchlist: &[String]) -> Result<(), String> {
        if !config.minimum_notional.is_finite()
            || !(100_000. ..=1_000_000_000.).contains(&config.minimum_notional)
        {
            return Err("Mínimo válido: $100.000 a $1.000 millones".into());
        }
        if !config.alert_buy && !config.alert_sell {
            return Err("Activa al menos un lado de alarma".into());
        }
        symbols(config, watchlist).map(|_| ())
    }

    /// Returns the next bounded group for the existing REST scanner.
    /// A full watchlist is rotated across calls; no group exceeds the provider's
    /// ten-symbol limit. The caller must report scan failures and coverage.
    pub fn next_batch(
        &mut self,
        config: &Config,
        watchlist: &[String],
    ) -> Result<Vec<String>, String> {
        Self::validate(config, watchlist)?;
        let universe = symbols(config, watchlist)?;
        if self.cursor >= universe.len() {
            self.cursor = 0;
        }
        let end = (self.cursor + MAX_BATCH).min(universe.len());
        let batch = universe[self.cursor..end].to_vec();
        self.cursor = if end == universe.len() { 0 } else { end };
        Ok(batch)
    }

    /// Call only after a scanner result has been received. Repeated scans and
    /// restarts do not repeat an alert if Engine is serialized between runs.
    pub fn observe(
        &mut self,
        config: &Config,
        watchlist: &[String],
        rows: &[(String, Block)],
        now_ms: u64,
    ) -> Result<Vec<Alert>, String> {
        Self::validate(config, watchlist)?;
        let today = calendar::date_at(now_ms)
            .ok_or("Fecha ET no disponible")?
            .to_string();
        if self.date != today {
            self.date = today.clone();
            self.seen.clear();
        }
        let allowed: BTreeSet<_> = symbols(config, watchlist)?.into_iter().collect();
        let mut alerts = BTreeMap::new();
        for (raw, block) in rows {
            let symbol = raw.trim().to_ascii_uppercase();
            if !allowed.contains(&symbol)
                || block.id.is_empty()
                || block.trf_id == 0
                || !block.notional.is_finite()
                || block.notional < config.minimum_notional
                || !block.price.is_finite()
                || block.price <= 0.
                || !block.shares.is_finite()
                || block.shares <= 0.
                || block.conditions.iter().any(|condition| ![0, 12, 37].contains(condition))
            {
                continue;
            }
            let Some(execution_ms) = block.execution_ms.filter(|t| *t <= now_ms) else {
                continue;
            };
            if calendar::date_at(execution_ms).is_none_or(|d| d.to_string() != today) {
                continue;
            }
            let side = match block.nbbo.as_str() {
                "En ask" | "Sobre ask" if config.alert_buy => Side::Buy,
                "En bid" | "Bajo bid" if config.alert_sell => Side::Sell,
                _ => continue,
            };
            let valid_quote = block
                .bid
                .zip(block.ask)
                .zip(block.quote_ms)
                .is_some_and(|((bid, ask), quote_ms)| {
                    bid.is_finite()
                        && ask.is_finite()
                        && bid > 0.
                        && ask >= bid
                        && (ask - bid) / ((ask + bid) / 2.) <= 0.01
                        && quote_ms <= execution_ms
                        && execution_ms - quote_ms <= 2_000
                });
            if !valid_quote {
                continue;
            }
            let key = format!("{today}:{symbol}:{}:{}:{execution_ms}", block.trf_id, block.id);
            if self.seen.contains(&key) {
                continue;
            }
            alerts.insert(
                key.clone(),
                Alert {
                    key,
                    symbol,
                    side,
                    execution_ms,
                    price: block.price,
                    shares: block.shares,
                    notional: block.notional,
                    nbbo: block.nbbo.clone(),
                },
            );
        }
        let mut out: Vec<_> = alerts.into_values().collect();
        out.sort_by(|a, b| a.execution_ms.cmp(&b.execution_ms).then_with(|| a.key.cmp(&b.key)));
        if self.seen.len().saturating_add(out.len()) > MAX_SEEN {
            return Err("Límite diario de identidades de bloques alcanzado; no se emiten alarmas sin deduplicación".into());
        }
        for alert in &out {
            self.seen.insert(alert.key.clone());
            self.history.push(StoredAlert {
                alert: alert.clone(),
                received_ms: now_ms,
                read: false,
            });
        }
        if self.history.len() > MAX_HISTORY {
            self.history.drain(..self.history.len() - MAX_HISTORY);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(scope: Scope) -> Config {
        Config { scope, minimum_notional: 100_000., alert_buy: true, alert_sell: true }
    }
    fn block(id: &str, at: u64, nbbo: &str) -> Block {
        Block {
            id: id.into(), price: 100., shares: 2_000., notional: 200_000.,
            execution_ms: Some(at), sip_ms: at, trf_ms: Some(at), trf_id: 7,
            conditions: vec![], nbbo: nbbo.into(), bid: Some(99.9),
            ask: Some(100.), quote_ms: Some(at - 200),
        }
    }
    fn at(date: &str) -> u64 {
        calendar::legacy_eastern_ms(&format!("{date} 11:00:00")).unwrap()
    }

    #[test]
    fn scans_entire_watchlist_in_bounded_rotating_batches() {
        let watchlist: Vec<_> = (0..23).map(|i| format!("T{i:02}")).collect();
        let mut engine = Engine::default();
        let config = config(Scope::Watchlist);
        let first = engine.next_batch(&config, &watchlist).unwrap();
        let second = engine.next_batch(&config, &watchlist).unwrap();
        let third = engine.next_batch(&config, &watchlist).unwrap();
        assert_eq!((first.len(), second.len(), third.len()), (10, 10, 3));
        assert_eq!(first[0], "T00");
        assert_eq!(second[0], "T10");
        assert_eq!(third[0], "T20");
        assert_eq!(engine.next_batch(&config, &watchlist).unwrap()[0], "T00");
    }

    #[test]
    fn selected_symbols_are_normalized_and_watchlist_outsiders_suppressed() {
        let mut engine = Engine::default();
        let c = config(Scope::Selected(vec![" googl ".into(), "QQQ".into(), "GOOGL".into()]));
        assert_eq!(engine.next_batch(&c, &[]).unwrap(), vec!["GOOGL", "QQQ"]);
        let time = at("2026-10-08");
        let found = engine.observe(&c, &[], &[("AAPL".into(), block("1", time, "En ask")), ("GOOGL".into(), block("2", time, "En ask"))], time + 1_000).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].side, Side::Buy);
    }

    #[test]
    fn only_verified_bid_ask_and_execution_date_create_alerts() {
        let mut engine = Engine::default();
        let c = config(Scope::Watchlist);
        let time = at("2026-10-08");
        let mut stale = block("stale", time, "En bid");
        stale.quote_ms = Some(time - 3_000);
        let mut previous = block("old", at("2026-10-07"), "En ask");
        previous.quote_ms = previous.execution_ms.map(|t| t - 100);
        let mut special = block("special", time, "En ask");
        special.conditions = vec![99];
        let rows = vec![
            ("QQQ".into(), block("ask", time, "En ask")),
            ("QQQ".into(), block("bid", time, "En bid")),
            ("QQQ".into(), block("unknown", time, "Sin NBBO verificable")),
            ("QQQ".into(), stale), ("QQQ".into(), previous), ("QQQ".into(), special),
        ];
        let found = engine.observe(&c, &["QQQ".into()], &rows, time + 1_000).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found.iter().map(|a| a.side.clone()).collect::<Vec<_>>(), vec![Side::Buy, Side::Sell]);
    }

    #[test]
    fn repeat_scan_and_serialized_restart_do_not_repeat_alerts() {
        let c = config(Scope::Watchlist);
        let time = at("2026-10-08");
        let rows = vec![("QQQ".into(), block("same", time, "Sobre ask"))];
        let mut engine = Engine::default();
        assert_eq!(engine.observe(&c, &["QQQ".into()], &rows, time + 1_000).unwrap().len(), 1);
        assert_eq!(engine.unread_count(), 1);
        assert_eq!(engine.history().len(), 1);
        let mut recovered: Engine = serde_json::from_str(&serde_json::to_string(&engine).unwrap()).unwrap();
        assert!(recovered.observe(&c, &["QQQ".into()], &rows, time + 2_000).unwrap().is_empty());
        assert_eq!(recovered.history().len(), 1);
        assert!(recovered.mark_read(&recovered.history()[0].alert.key.clone()));
        assert_eq!(recovered.unread_count(), 0);
        let next = at("2026-10-09");
        assert!(recovered.observe(&c, &["QQQ".into()], &rows, next).unwrap().is_empty());
        assert_eq!(recovered.date, "2026-10-09");
        assert_eq!(recovered.history().len(), 1);
    }

    #[test]
    fn invalid_scope_and_threshold_are_rejected() {
        let from_button: Config = serde_json::from_str(r#"{"scope":{"selected":["QQQ","GOOGL"]},"minimumNotional":10000000,"alertBuy":true,"alertSell":true}"#).unwrap();
        assert!(Engine::validate(&from_button, &[]).is_ok());
        let from_watchlist: Config = serde_json::from_str(r#"{"scope":"watchlist","minimumNotional":10000000,"alertBuy":true,"alertSell":true}"#).unwrap();
        assert!(Engine::validate(&from_watchlist, &["QQQ".into()]).is_ok());
        assert!(Engine::validate(&config(Scope::Selected(vec![])), &[]).is_err());
        assert!(Engine::validate(&config(Scope::Selected(vec!["$BAD".into()])), &[]).is_err());
        let mut c = config(Scope::Watchlist);
        c.minimum_notional = f64::NAN;
        assert!(Engine::validate(&c, &["QQQ".into()]).is_err());
    }
}
