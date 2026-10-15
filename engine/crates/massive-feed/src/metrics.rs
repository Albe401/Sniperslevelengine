use crate::live::Bar;
use serde::Serialize;
use snipers_core::{
    bars::{self, Minute},
    calendar::{self, Day},
};
use std::collections::BTreeMap;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetrics {
    pub session: String,
    pub phase: String,
    pub vwap: Option<f64>,
    pub rvol: Option<f64>,
    pub volume: Option<f64>,
    pub baseline_sessions: usize,
    pub asof_ms: Option<u64>,
}

pub fn calculate(input: &BTreeMap<u64, Bar>, now: u64) -> SessionMetrics {
    let mut out = SessionMetrics::default();
    let Some(today) = calendar::date_at(now) else {
        out.phase = "Calendario no disponible".into();
        return out;
    };
    out.session = today.to_string();
    let schedule = calendar::schedule(today);
    let session = match schedule {
        Day::Open(s) => s,
        Day::Closed => {
            out.phase = "Sesión cerrada".into();
            return out;
        }
        Day::Unknown => {
            out.phase = "Fecha fuera de cobertura del calendario".into();
            return out;
        }
    };
    let elapsed = (now.min(session.close_ms).saturating_sub(session.open_ms)) / 60_000;
    if elapsed < 2 {
        out.phase = "Esperando primera vela RTH completa".into();
        return out;
    }
    let slot = (elapsed / 2 * 2 - 2) as u32;
    let mut sessions: BTreeMap<String, BTreeMap<u32, Minute>> = BTreeMap::new();
    for b in input.values() {
        let Some(date) = calendar::date_at(b.timestamp_ms) else {
            continue;
        };
        if date > today {
            continue;
        }
        let Day::Open(day) = calendar::schedule(date) else {
            continue;
        };
        if b.timestamp_ms < day.open_ms
            || b.timestamp_ms >= day.close_ms
            || b.timestamp_ms + 60_000 > now
        {
            continue;
        }
        sessions.entry(day.date).or_default().insert(
            ((b.timestamp_ms - day.open_ms) / 60_000) as u32,
            Minute {
                t: b.timestamp_ms,
                o: b.open,
                h: b.high,
                l: b.low,
                c: b.close,
                v: b.volume,
                vw: b.vwap,
            },
        );
    }
    let target = sessions.remove(&session.date).unwrap_or_default();
    let history: Vec<_> = sessions.into_values().rev().collect();
    out.baseline_sessions = history
        .iter()
        .filter(|m| bars::cumulative_at_slot(m, slot).is_some())
        .take(5)
        .count();
    out.vwap = bars::vwap_at_slot(&target, slot);
    out.volume = bars::cumulative_at_slot(&target, slot);
    out.rvol = bars::relative_volume(&target, &history, slot).map(|r| r.value);
    out.asof_ms = Some(session.open_ms + u64::from(slot + 2) * 60_000);
    out.phase = if now >= session.close_ms {
        "Cierre RTH"
    } else {
        "Sesión RTH"
    }
    .into();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_vwap_is_invented_from_ohlc_when_provider_omits_it() {
        let day = calendar::date_at(1_790_163_000_000).unwrap();
        let Day::Open(s) = calendar::schedule(day) else {
            panic!()
        };
        let mut input = BTreeMap::new();
        for i in 0..30 {
            let t = s.open_ms + i * 60_000;
            input.insert(
                t,
                Bar {
                    timestamp_ms: t,
                    open: 10.0,
                    high: 12.0,
                    low: 9.0,
                    close: 11.0,
                    volume: 20.0,
                    vwap: Some(10.5),
                },
            );
        }
        let now = s.open_ms + 30 * 60_000;
        let result = calculate(&input, now);
        assert_eq!(result.vwap, Some(10.5));
        assert!(result.rvol.is_none());
        input.get_mut(&s.open_ms).unwrap().vwap = None;
        assert!(calculate(&input, now).vwap.is_none());
    }
}
