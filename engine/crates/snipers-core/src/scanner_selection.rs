//! Session selection by activity, separate from options confirmation and entries.
use crate::{
    calendar::{self, Day},
    scanner::{self, Input},
};
use serde::Serialize;

#[derive(Clone, Default)]
pub struct Selection {
    date: String,
    symbols: Vec<String>,
    locked_at: Option<u64>,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub session: String,
    pub phase: String,
    pub symbols: Vec<String>,
    pub locked_at: Option<u64>,
    pub locked: bool,
    pub entry_confirmed: bool,
}
impl Selection {
    pub fn restore(&mut self, symbols: Vec<String>, locked_ms: u64, now: u64) -> bool {
        let Some(date) = calendar::date_at(now) else {
            return false;
        };
        let Day::Open(day) = calendar::schedule(date) else {
            return false;
        };
        let mut seen = std::collections::BTreeSet::new();
        if locked_ms > now
            || locked_ms < day.open_ms + 900_000
            || locked_ms >= day.close_ms
            || symbols.is_empty()
            || symbols.len() > 5
            || symbols
                .iter()
                .any(|s| crate::universe::stock_asset(s).is_none() || !seen.insert(s))
        {
            return false;
        }
        self.date = day.date;
        self.symbols = symbols;
        self.locked_at = Some(locked_ms);
        true
    }
    pub fn update(&mut self, rows: &[Input], now: u64, universe_ready: bool) -> Status {
        let Some(date) = calendar::date_at(now) else {
            return Status {
                phase: "CALENDARIO NO DISPONIBLE".into(),
                ..Default::default()
            };
        };
        let label = date.to_string();
        if label != self.date {
            self.date = label.clone();
            self.symbols.clear();
            self.locked_at = None;
        }
        let phase = match calendar::schedule(date) {
            Day::Unknown => "CALENDARIO NO DISPONIBLE",
            Day::Closed => "SESION CERRADA",
            Day::Open(day) => {
                if now < day.open_ms - 900_000 {
                    "ESPERANDO 09:15 ET"
                } else if now < day.open_ms {
                    "PREMARKET · SIN SELECCION RTH"
                } else if now >= day.close_ms {
                    "SESION CERRADA"
                } else if now < day.open_ms + 900_000 {
                    "APERTURA · SELECCION DINAMICA"
                } else {
                    if self.locked_at.is_none() && universe_ready {
                        // Revalidate current bars across the full universe before filtering
                        // thresholds; truncating first could exclude an eligible sixth row.
                        self.symbols = scanner::rank_all(rows, now)
                            .into_iter()
                            .filter(|r| {
                                r.volume_2m >= 25_000.0
                                    && r.notional_2m >= 5_000_000.0
                                    && now >= r.bar_end_ms.saturating_add(2_000)
                            })
                            .take(5)
                            .map(|r| r.symbol)
                            .collect();
                        if !self.symbols.is_empty() {
                            self.locked_at = Some(now);
                        }
                    }
                    if self.locked_at.is_some() {
                        "TOP5 FIJO PROVISIONAL"
                    } else if !universe_ready {
                        "ESPERANDO RECUPERACION DEL UNIVERSO"
                    } else {
                        "SIN CANDIDATOS RECIENTES CON ACTIVIDAD SUFICIENTE"
                    }
                }
            }
        };
        Status {
            session: label,
            phase: phase.into(),
            symbols: self.symbols.clone(),
            locked_at: self.locked_at,
            locked: self.locked_at.is_some(),
            entry_confirmed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn open(date: &str) -> u64 {
        let Day::Open(day) =
            calendar::schedule(calendar::date_at(calendar::day_bounds(date).unwrap().0).unwrap())
        else {
            panic!()
        };
        day.open_ms
    }
    #[test]
    fn restored_membership_is_session_scoped_and_cannot_be_replaced_by_new_ranking() {
        let now = open("2026-09-24") + 1_800_000;
        let mut selection = Selection::default();
        assert!(!selection.restore(vec!["SPX".into()], now - 1000, now));
        assert!(!selection.restore(vec!["QQQ".into()], now + 1, now));
        assert!(!selection.restore(vec!["QQQ".into()], open("2026-09-23") + 1_800_000, now));
        assert!(selection.restore(vec!["QQQ".into(), "SPY".into()], now - 1000, now));
        let restored = selection.update(&[], now, true);
        assert!(restored.locked);
        assert_eq!(restored.symbols, ["QQQ", "SPY"]);
        assert_eq!(restored.locked_at, Some(now - 1000));
        assert!(selection
            .update(&[], open("2026-09-25"), true)
            .symbols
            .is_empty());
    }
    #[test]
    fn activity_threshold_is_applied_before_top_five_truncation() {
        let start = open("2026-09-24");
        let mut rows: Vec<_> = crate::universe::assets()
            .keys()
            .filter(|s| crate::universe::stock_asset(s).is_some())
            .take(6)
            .map(|s| Input {
                symbol: s.clone(),
                bar_end_ms: start + 840_000,
                price: 1000.0,
                volume_2m: 24_999.0,
                vwap: Some(999.0),
                rvol: Some(1.0),
                source_verified: true,
            })
            .collect();
        rows[5].volume_2m = 25_000.0;
        rows[5].price = 200.0;
        let mut selection = Selection::default();
        assert_eq!(
            selection.update(&rows, start + 900_000, true).symbols,
            [rows[5].symbol.clone()]
        );
    }
    #[test]
    fn morning_lock_requires_recovery_activity_and_fresh_bars_and_resets_by_session() {
        let start = open("2026-09-24");
        let mut selection = Selection::default();
        let mut row = Input {
            symbol: "QQQ".into(),
            bar_end_ms: start + 840_000,
            price: 200.0,
            volume_2m: 25_000.0,
            vwap: Some(199.0),
            rvol: Some(1.0),
            source_verified: true,
        };
        for (time, phase) in [
            (start - 900_001, "ESPERANDO"),
            (start - 900_000, "PREMARKET"),
            (start, "APERTURA"),
        ] {
            let status = selection.update(&[row.clone()], time, true);
            assert!(status.phase.starts_with(phase));
            assert!(!status.locked);
        }
        assert!(
            !selection
                .update(&[row.clone()], start + 900_000, false)
                .locked
        );
        row.volume_2m = 24_999.0;
        assert!(
            !selection
                .update(&[row.clone()], start + 900_000, true)
                .locked
        );
        row.volume_2m = 25_000.0;
        row.bar_end_ms = start + 600_000;
        assert!(
            !selection
                .update(&[row.clone()], start + 900_000, true)
                .locked
        );
        row.bar_end_ms = start + 840_000;
        let status = selection.update(&[row], start + 900_000, true);
        assert!(status.locked);
        assert_eq!(status.symbols, ["QQQ"]);
        assert!(!status.entry_confirmed);
        // Disconnection cannot silently select replacement symbols or erase session identity.
        assert_eq!(
            selection.update(&[], start + 1_000_000, false).symbols,
            ["QQQ"]
        );
        assert!(selection
            .update(&[], open("2026-09-25"), true)
            .symbols
            .is_empty());
        assert!(
            !selection
                .update(&[], open("2026-11-27") + 12_600_000, true)
                .locked
        );
    }
}
