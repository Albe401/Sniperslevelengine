//! Session-bound scanner opportunity memory. Signals never admit a trade.
use crate::{calendar, scanner_confirmation::continuity, scanner_trigger};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
struct Entry {
    targets: Vec<f64>,
    bullish: bool,
    last_bar: u64,
    reference: Option<f64>,
    invalidation: Option<f64>,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub targets: Vec<f64>,
    pub state: String,
    pub reason: String,
    pub signal_active: bool,
    pub reference: Option<f64>,
    pub invalidation: Option<f64>,
    /// Deliberately separate from the analytical scanner signal.
    pub entry_confirmed: bool,
}
fn transition(memory: &mut Option<Entry>, e: &Evidence<'_>, now: u64) -> Read {
    let (structure, bullish, close, vwap, bar, trigger) =
        (e.all_gates, e.bullish, e.close, e.vwap, e.bar_ms, e.trigger);
    if let Some(entry) = memory.as_mut() {
        let sequence = continuity(Some(entry.last_bar), Some(bar), now);
        let keep = sequence.ok
            && structure
            && if entry.bullish {
                close > vwap && entry.invalidation.is_none_or(|v| close > v)
            } else {
                close < vwap && entry.invalidation.is_none_or(|v| close < v)
            };
        if keep {
            entry.last_bar = bar;
            return Read {
                targets: entry.targets.clone(),
                state: "ENTRADA CONFIRMADA".into(),
                reason: "TRIGGER VWAP 2M ACTIVO".into(),
                signal_active: true,
                reference: entry.reference,
                invalidation: entry.invalidation,
                ..Default::default()
            };
        }
        *memory = None;
        return Read {
            state: "CONDICIONES PERDIDAS".into(),
            reason: if sequence.ok {
                "TRIGGER 2M INVALIDADO O CONDICIONES PREVIAS PERDIDAS".into()
            } else {
                format!("STALE DATA GUARD · {}", sequence.reason)
            },
            ..Default::default()
        };
    }
    if structure && trigger.ready {
        *memory = Some(Entry {
            targets: e.targets.to_vec(),
            bullish,
            last_bar: bar,
            reference: trigger.reference_price,
            invalidation: trigger.invalidation,
        });
        Read {
            targets: e.targets.to_vec(),
            state: "ENTRADA CONFIRMADA".into(),
            reason: trigger.reason.clone(),
            signal_active: true,
            reference: trigger.reference_price,
            invalidation: trigger.invalidation,
            ..Default::default()
        }
    } else {
        Read {
            state: if structure {
                "CONFIRMANDO ESTRUCTURA"
            } else {
                "NO CONFIRMADO"
            }
            .into(),
            reason: if structure {
                trigger.reason.clone()
            } else {
                "ESPERANDO CONFIRMACIONES".into()
            },
            ..Default::default()
        }
    }
}

#[derive(Clone, Default)]
pub struct Memory {
    date: Option<chrono::NaiveDate>,
    members: BTreeSet<String>,
    entries: BTreeMap<String, Entry>,
    blocked_through: BTreeMap<String, u64>,
}
pub struct Evidence<'a> {
    pub targets: &'a [f64],
    pub symbol: &'a str,
    pub bar_ms: u64,
    pub close: f64,
    pub vwap: f64,
    pub bullish: bool,
    pub verified: bool,
    /// Includes all upstream base, flow, liquidity, structure and final gates.
    pub all_gates: bool,
    pub trigger: &'a scanner_trigger::Read,
}
impl Memory {
    pub fn reconcile(&mut self, symbols: &[String], now: u64) {
        let date = calendar::date_at(now);
        let next: BTreeSet<_> = symbols.iter().cloned().collect();
        let valid = symbols.len() <= 5
            && next.len() == symbols.len()
            && symbols
                .iter()
                .all(|s| crate::universe::stock_asset(s).is_some());
        if date != self.date || !valid {
            self.entries.clear();
            self.blocked_through.clear();
        }
        self.date = date;
        self.members = if valid { next } else { BTreeSet::new() };
        self.entries.retain(|symbol, entry| {
            if self.members.contains(symbol) {
                true
            } else {
                self.blocked_through.insert(symbol.clone(), entry.last_bar);
                false
            }
        });
    }
    pub fn evaluate(&mut self, e: &Evidence<'_>, now: u64) -> Read {
        let session = calendar::date_at(now).and_then(|d| match calendar::schedule(d) {
            calendar::Day::Open(s) => Some(s),
            _ => None,
        });
        let valid = e.verified
            && self.members.contains(e.symbol)
            && self.date == calendar::date_at(now)
            && continuity(Some(e.bar_ms), Some(e.bar_ms), now).ok
            && session.is_some_and(|s| {
                now >= s.open_ms
                    && now < s.close_ms
                    && e.bar_ms >= s.open_ms + 120_000
                    && (e.bar_ms - s.open_ms).is_multiple_of(120_000)
            })
            && [e.close, e.vwap].iter().all(|v| v.is_finite() && *v > 0.0);
        if !valid {
            let previous = self.entries.remove(e.symbol);
            if let Some(entry) = &previous {
                self.blocked_through.insert(e.symbol.into(), entry.last_bar);
            }
            return Read {
                state: if previous.is_some() {
                    "CONDICIONES PERDIDAS"
                } else {
                    "NO CONFIRMADO"
                }
                .into(),
                reason: "IDENTIDAD, SESION O PRECIO SIN VERIFICAR".into(),
                ..Default::default()
            };
        }
        // A fresh signal requires a valid trigger geometry; maintained signals
        // use their original invalidation, even if this candle has no new trigger.
        let geometry = e
            .trigger
            .reference_price
            .zip(e.trigger.invalidation)
            .is_some_and(|(p, stop)| {
                p.is_finite()
                    && p > 0.0
                    && stop.is_finite()
                    && stop > 0.0
                    && (p - e.close).abs() <= 0.000_051
                    && if e.bullish {
                        stop < p && p > e.vwap
                    } else {
                        stop > p && p < e.vwap
                    }
            });
        let mut trigger = e.trigger.clone();
        trigger.ready &= geometry
            && self
                .blocked_through
                .get(e.symbol)
                .is_none_or(|bar| e.bar_ms > *bar);
        if geometry && e.trigger.ready && !trigger.ready {
            trigger.reason = "ESPERANDO UN NUEVO TRIGGER 2M TRAS INVALIDACION".into();
        }
        let mut entry = self.entries.remove(e.symbol);
        let previous_bar = entry.as_ref().map(|entry| entry.last_bar);
        let result = transition(
            &mut entry,
            &Evidence {
                trigger: &trigger,
                ..*e
            },
            now,
        );
        if let Some(entry) = entry {
            self.entries.insert(e.symbol.into(), entry);
        } else if let Some(previous) = previous_bar {
            self.blocked_through
                .insert(e.symbol.into(), previous.max(e.bar_ms));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_memory_layer_matches_720_scenarios() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-scanner-memory.json"))
                .unwrap();
        let cases = fixtures["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 720);
        let origin = calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        for (index, case) in cases.iter().enumerate() {
            let i = &case["input"];
            let bullish = i["bullish"].as_bool().unwrap();
            let mut entry = i["hasMemory"].as_bool().unwrap().then_some(Entry {
                targets: vec![110.],
                bullish,
                last_bar: origin,
                reference: Some(100.),
                invalidation: i["invalidation"].as_f64(),
            });
            let bar = (origin as i64 + i["gap"].as_i64().unwrap()) as u64;
            let trigger = scanner_trigger::Read {
                ready: i["triggerReady"].as_bool().unwrap(),
                reference_price: i["close"].as_f64(),
                invalidation: Some(if bullish { 99. } else { 101. }),
                ..Default::default()
            };
            let r = transition(
                &mut entry,
                &Evidence {
                    symbol: "QQQ",
                    bar_ms: bar,
                    close: i["close"].as_f64().unwrap(),
                    vwap: 100.,
                    bullish,
                    verified: true,
                    all_gates: i["structure"].as_bool().unwrap(),
                    trigger: &trigger,
                    targets: &[],
                },
                bar.max(origin),
            );
            assert_eq!(
                serde_json::json!({"state":r.state,"signal":r.signal_active,"retained":entry.is_some()}),
                case["expected"],
                "case {index}"
            );
            assert!(!r.entry_confirmed);
        }
    }
    #[test]
    fn repeated_observations_preserve_original_targets_and_accept_display_rounding() {
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let trigger = scanner_trigger::Read {
            ready: true,
            reference_price: Some(102.123456),
            invalidation: Some(99.),
            ..Default::default()
        };
        let mut memory = Memory::default();
        memory.reconcile(&["QQQ".into()], now);
        let mut e = Evidence {
            symbol: "QQQ",
            bar_ms: now,
            close: 102.1235,
            vwap: 100.,
            bullish: true,
            verified: true,
            all_gates: true,
            trigger: &trigger,
            targets: &[104., 106.],
        };
        assert!(memory.evaluate(&e, now).signal_active);
        e.targets = &[110.];
        e.close = 103.;
        e.bar_ms = now + 120_000;
        let kept = memory.evaluate(&e, now + 120_000);
        assert!(kept.signal_active);
        assert_eq!(kept.reference, Some(102.123456));
        assert_eq!(kept.targets, vec![104., 106.]);
        e.all_gates = false;
        let lost = memory.evaluate(&e, now + 120_000);
        assert!(!lost.signal_active);
        assert!(lost.targets.is_empty());
    }
    #[test]
    fn membership_session_and_invalid_evidence_cannot_retain_signals() {
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let trigger = scanner_trigger::Read {
            ready: true,
            reference_price: Some(102.),
            invalidation: Some(99.),
            ..Default::default()
        };
        let mut memory = Memory::default();
        memory.reconcile(&["QQQ".into(), "SPY".into()], now);
        let mut e = Evidence {
            symbol: "QQQ",
            bar_ms: now,
            close: 102.,
            vwap: 100.,
            bullish: true,
            verified: true,
            all_gates: true,
            trigger: &trigger,
            targets: &[],
        };
        assert!(memory.evaluate(&e, now).signal_active);
        e.symbol = "SPY";
        assert!(memory.evaluate(&e, now).signal_active);
        e.symbol = "QQQ";
        e.verified = false;
        assert_eq!(memory.evaluate(&e, now).state, "CONDICIONES PERDIDAS");
        assert!(memory.entries.contains_key("SPY"));
        memory.reconcile(&["QQQ".into()], now);
        assert!(memory.entries.is_empty());
        e.verified = true;
        assert!(!memory.evaluate(&e, now).signal_active);
        e.bar_ms = now + 120_000;
        assert!(memory.evaluate(&e, now + 120_000).signal_active);
        assert!(!memory.evaluate(&e, now + 240_001).signal_active);
        assert!(memory.entries.is_empty());
        assert!(!memory.evaluate(&e, now + 120_000).signal_active);
        e.bar_ms = now + 240_000;
        assert!(memory.evaluate(&e, now + 240_000).signal_active);
        memory.reconcile(&["QQQ".into()], now + 86_400_000);
        assert!(memory.entries.is_empty());
        assert!(!memory.evaluate(&e, now + 86_400_000).signal_active);
        memory.reconcile(&vec!["QQQ".into(); 6], now);
        assert!(memory.members.is_empty());
    }
}
