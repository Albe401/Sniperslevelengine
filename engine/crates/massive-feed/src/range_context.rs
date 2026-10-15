//! RTH bands use the first complete 09:30 minute, never a truncated chart window.
use crate::live::LiveState;
use serde::Serialize;
use snipers_core::{
    calendar::{self, Day},
    reading::Alignment,
};

#[derive(Clone, Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub ready: bool,
    pub symbol: String,
    pub detail: String,
    pub date: String,
    pub anchor: Option<f64>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub remaining_call: Option<f64>,
    pub remaining_put: Option<f64>,
    pub extension_call: Option<f64>,
    pub extension_put: Option<f64>,
    pub used_call_pct: Option<f64>,
    pub used_put_pct: Option<f64>,
    pub range_state: String,
    pub alignment_range_state: String,
    pub extremes_complete: bool,
    pub price_ms: Option<u64>,
}

pub fn calculate(s: &LiveState, now: u64, alignment: Option<&Alignment>) -> Context {
    let mut r = Context {
        symbol: s.symbol.clone(),
        detail: "Esperando apertura RTH y datos vigentes".into(),
        ..Default::default()
    };
    let Some(date) = calendar::date_at(now) else {
        return r;
    };
    r.date = date.to_string();
    let Day::Open(session) = calendar::schedule(date) else {
        return r;
    };
    if now < session.open_ms + 60_000 || now >= session.close_ms || s.stocks.phase != "streaming" {
        return r;
    }
    let snapshot = &s.option_snapshot_status;
    if snapshot.phase != "ready"
        || !snapshot
            .observed_ms
            .is_some_and(|t| t <= now && now - t <= 900_000 && calendar::date_at(t) == Some(date))
    {
        return r;
    }
    let Some(model) = snapshot.daily_range.as_ref().filter(|m| m.ready) else {
        return r;
    };
    let Some(movement) = model
        .expected_move_dollars
        .filter(|x| x.is_finite() && *x > 0.)
    else {
        return r;
    };
    let Some(tick) = s.ticks.get(&s.symbol).filter(|t| t.symbol == s.symbol) else {
        return r;
    };
    let Some(price_ms) = tick
        .trade_ms
        .filter(|t| *t >= session.open_ms && *t <= now && now - *t <= 15_000)
    else {
        return r;
    };
    let Some(spot) = tick.price.filter(|x| x.is_finite() && *x > 0.) else {
        return r;
    };
    // Missing 09:30 data is unknown; never label a later minute as the open.
    let Some(first) = s.minutes.get(&session.open_ms) else {
        return r;
    };
    let valid = |b: &crate::live::Bar| {
        [b.open, b.high, b.low, b.close]
            .iter()
            .all(|x| x.is_finite() && *x > 0.)
            && b.high >= b.open.max(b.close)
            && b.low <= b.open.min(b.close)
            && b.high >= b.low
    };
    if first.timestamp_ms != session.open_ms || !valid(first) {
        return r;
    }
    let (anchor, lower, upper) = (first.open, first.open - movement, first.open + movement);
    if !lower.is_finite() || lower <= 0. || !upper.is_finite() {
        return r;
    }
    let remaining_call = upper - spot;
    let remaining_put = spot - lower;
    r.range_state = if spot > upper {
        "EXTENDIDO ARRIBA"
    } else if spot < lower {
        "EXTENDIDO ABAJO"
    } else if remaining_call <= movement * 0.15 {
        "CERCA DEL LÍMITE CALL"
    } else if remaining_put <= movement * 0.15 {
        "CERCA DEL LÍMITE PUT"
    } else {
        "DENTRO DEL RANGO"
    }
    .into();
    r.alignment_range_state = alignment.map_or_else(
        || "ESTRUCTURA PENDIENTE".into(),
        |a| {
            if a.status.contains("FULL") && matches!(a.side.as_str(), "CALL" | "PUT") {
                let (extended, remaining) = if a.side == "CALL" {
                    (spot > upper, remaining_call)
                } else {
                    (spot < lower, remaining_put)
                };
                format!(
                    "FULL ALIGNMENT {} · RANGO {} {}",
                    a.side,
                    a.side,
                    if extended {
                        "EXTENDIDO"
                    } else if remaining <= movement * 0.15 {
                        "CASI CONSUMIDO"
                    } else {
                        "DISPONIBLE"
                    }
                )
            } else {
                format!("{} {}", a.status, a.side)
            }
        },
    );
    // Complete-minute extremes only; missing minutes suppress consumption, not the anchor.
    let end = now / 60_000 * 60_000;
    let mut high = anchor;
    let mut low = anchor;
    let mut complete = s.history.phase == "ready";
    for t in (session.open_ms..end).step_by(60_000) {
        match s
            .minutes
            .get(&t)
            .filter(|b| b.timestamp_ms == t && valid(b))
        {
            Some(b) => {
                high = high.max(b.high);
                low = low.min(b.low)
            }
            None => complete = false,
        }
    }
    r.ready = true;
    r.anchor = Some(anchor);
    r.lower = Some(lower);
    r.upper = Some(upper);
    r.price_ms = Some(price_ms);
    r.remaining_call = Some(remaining_call.max(0.));
    r.remaining_put = Some(remaining_put.max(0.));
    r.extension_call = Some((spot - upper).max(0.));
    r.extension_put = Some((lower - spot).max(0.));
    let used_call = ((high - anchor) / movement * 100.).max(0.);
    let used_put = ((anchor - low) / movement * 100.).max(0.);
    complete &= used_call.is_finite() && used_put.is_finite();
    r.extremes_complete = complete;
    if complete {
        r.used_call_pct = Some(used_call);
        r.used_put_pct = Some(used_put);
    }
    r.detail=if complete{"Ancla: apertura de vela 09:30 ET. Recorrido: minutos completos observados."}else{"Ancla: apertura de vela 09:30 ET. Recorrido sin certificar: histórico parcial o minutos ausentes."}.into();
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{Bar, Tick};
    fn state(date: &str) -> (LiveState, u64) {
        let Day::Open(day) = calendar::schedule(date.parse().unwrap()) else {
            panic!()
        };
        let now = day.open_ms + 120_000;
        let mut s = LiveState::default();
        s.symbol = "QQQ".into();
        s.stocks.phase = "streaming".into();
        s.history.phase = "ready".into();
        s.option_snapshot_status.phase = "ready".into();
        s.option_snapshot_status.observed_ms = Some(now);
        s.option_snapshot_status.daily_range = Some(snipers_core::daily_range::Range {
            ready: true,
            expected_move_dollars: Some(10.),
            ..Default::default()
        });
        s.ticks.insert(
            "QQQ".into(),
            Tick {
                symbol: "QQQ".into(),
                price: Some(509.),
                trade_ms: Some(now),
                bid: None,
                ask: None,
                trade_sequence: None,
                quote_ms: None,
            },
        );
        for t in [day.open_ms, day.open_ms + 60_000] {
            s.minutes.insert(
                t,
                Bar {
                    timestamp_ms: t,
                    open: 500.,
                    high: 506.,
                    low: 497.,
                    close: 502.,
                    volume: 100.,
                    vwap: None,
                },
            );
        }
        (s, now)
    }
    #[test]
    fn bands_preserve_legacy_thresholds_and_never_use_a_later_or_foreign_day_open() {
        for date in ["2026-09-24", "2026-11-27"] {
            let (mut s, now) = state(date);
            let r = calculate(&s, now, None);
            assert!(r.ready && r.extremes_complete);
            assert_eq!(r.anchor, Some(500.));
            assert_eq!((r.lower, r.upper), (Some(490.), Some(510.)));
            assert_eq!((r.used_call_pct, r.used_put_pct), (Some(60.), Some(30.)));
            assert_eq!(r.range_state, "CERCA DEL LÍMITE CALL");
            s.ticks.get_mut("QQQ").unwrap().price = Some(511.);
            assert_eq!(calculate(&s, now, None).extension_call, Some(1.));
            s.history.phase = "loading".into();
            assert_eq!(calculate(&s, now, None).used_call_pct, None);
            s.minutes.remove(&(now - 120_000));
            assert!(!calculate(&s, now, None).ready);
        }
    }
    #[test]
    fn stale_disconnect_new_date_closed_session_or_bad_geometry_revokes_context() {
        let (mut s, now) = state("2026-09-24");
        assert!(!calculate(&s, now + 15_001, None).ready);
        assert!(!calculate(&s, now + 86_400_000, None).ready);
        let (mut early, early_now) = state("2026-11-27");
        let close = early_now - 120_000 + 210 * 60_000;
        early.ticks.get_mut("QQQ").unwrap().trade_ms = Some(close);
        early.option_snapshot_status.observed_ms = Some(close);
        assert!(!calculate(&early, close, None).ready);
        s.stocks.phase = "reconnecting".into();
        assert!(!calculate(&s, now, None).ready);
        s.stocks.phase = "streaming".into();
        s.minutes.get_mut(&(now - 120_000)).unwrap().open = f64::NAN;
        assert!(!calculate(&s, now, None).ready);
    }
    #[test]
    fn legacy_dashboard_twenty_one_contexts_preserve_ranges_and_alignment() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-range-context.json"))
                .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 21);
        for c in cases {
            let (mut s, now) = state("2026-09-24");
            s.ticks.get_mut("QQQ").unwrap().price = c["spot"].as_f64();
            let alignment = snipers_core::reading::align([c["side"].as_str().unwrap(); 3]);
            let alignment = Alignment {
                status: c["status"].as_str().unwrap().into(),
                side: c["side"].as_str().unwrap().into(),
                ..alignment
            };
            let r = calculate(&s, now, Some(&alignment));
            let expected = &c["expected"];
            for (key, value) in [
                ("anchor", r.anchor),
                ("lower", r.lower),
                ("upper", r.upper),
                ("remaining_call", r.remaining_call),
                ("remaining_put", r.remaining_put),
                ("extension_call", r.extension_call),
                ("extension_put", r.extension_put),
                ("used_call_pct", r.used_call_pct),
                ("used_put_pct", r.used_put_pct),
            ] {
                assert_eq!(value, expected[key].as_f64(), "{key}: {c}");
            }
            assert_eq!(r.range_state, expected["range_state"].as_str().unwrap());
            assert_eq!(
                r.alignment_range_state,
                expected["alignment_range_state"].as_str().unwrap()
            );
        }
    }
}
