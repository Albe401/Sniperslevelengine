//! Bounded chart transport: the live stream stays at five sessions; extended history is requested separately.
use crate::live::Bar;
use serde::Serialize;
use snipers_core::calendar::{self, Day, Session};
use std::collections::BTreeMap;
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartHistory {
    pub minutes: Vec<Bar>,
    pub sessions: Vec<Session>,
}
pub fn collect(input: &BTreeMap<u64, Bar>, now: u64) -> ChartHistory {
    collect_with_limit(input, now, 5)
}
pub fn collect_with_limit(input: &BTreeMap<u64, Bar>, now: u64, limit: usize) -> ChartHistory {
    let mut sessions = BTreeMap::new();
    if let Some(today) = calendar::date_at(now) {
        if let Day::Open(s) = calendar::schedule(today) {
            sessions.insert(s.date.clone(), s);
        }
    }
    let mut minutes = Vec::new();
    for bar in input.values().rev() {
        if bar.timestamp_ms > now {
            continue;
        }
        let Some(date) = calendar::date_at(bar.timestamp_ms) else {
            continue;
        };
        let Day::Open(session) = calendar::schedule(date) else {
            continue;
        };
        if bar.timestamp_ms < session.open_ms || bar.timestamp_ms >= session.close_ms {
            continue;
        }
        if !sessions.contains_key(&session.date) && sessions.len() >= limit {
            break;
        }
        sessions.insert(session.date.clone(), session);
        minutes.push(bar.clone());
    }
    minutes.reverse();
    ChartHistory {
        minutes,
        sessions: sessions.into_values().collect(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn five_sessions_exclude_extended_hours_and_future() {
        let now = calendar::rfc3339_ms("2026-09-24T16:00:00-04:00").unwrap();
        let mut input = BTreeMap::new();
        for day in [16, 17, 18, 21, 22, 23, 24] {
            for hour in [8, 10, 17] {
                let t =
                    calendar::rfc3339_ms(&format!("2026-09-{day}T{hour:02}:00:00-04:00")).unwrap();
                input.insert(
                    t,
                    Bar {
                        timestamp_ms: t,
                        open: 100.,
                        high: 101.,
                        low: 99.,
                        close: 100.,
                        volume: 10.,
                        vwap: Some(100.),
                    },
                );
            }
        }
        let result = collect(&input, now);
        assert_eq!(result.sessions.len(), 5);
        assert_eq!(result.minutes.len(), 5);
        assert_eq!(result.sessions[0].date, "2026-09-18");
        assert!(result.minutes.iter().all(|b| b.timestamp_ms <= now));
    }
    #[test]
    fn extended_chart_keeps_thirty_real_sessions() {
        let now = calendar::rfc3339_ms("2026-10-07T16:00:00-04:00").unwrap();
        let mut input = BTreeMap::new();
        let mut day = calendar::date_at(now).unwrap();
        let mut inserted = 0;
        while inserted < 35 {
            if let Day::Open(session) = calendar::schedule(day) {
                input.insert(session.open_ms, Bar { timestamp_ms:session.open_ms, open:100.,high:101.,low:99.,close:100.,volume:10.,vwap:Some(100.) });
                inserted += 1;
            }
            day = day.pred_opt().unwrap();
        }
        let result = collect_with_limit(&input, now, 30);
        assert_eq!(result.sessions.len(), 30);
        assert_eq!(result.minutes.len(), 30);
        assert_eq!(collect(&input, now).sessions.len(), 5);
    }
}
