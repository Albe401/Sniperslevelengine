//! Standalone observer. Not called by live, intraday_strategy or paper_bot.
//! SMC labels describe price patterns; they never certify institutional liquidity.
use crate::smc_patterns::{self, Bar, Layer, Reference};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use snipers_core::calendar::{self, Day};
use std::collections::BTreeMap;
pub const VERSION: &str = "smc-observer-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub displacement_atr: f64,
    pub body_fraction: f64,
    pub relative_volume: f64,
    pub pivot_confirming_bars: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            displacement_atr: 0.8,
            body_fraction: 0.6,
            relative_volume: 1.2,
            pivot_confirming_bars: 2,
        }
    }
}
fn finite(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}
fn minute(v: &Value) -> Option<Bar> {
    let start = v["timestampMs"].as_u64()?;
    let (open, high, low, close, volume) = (
        finite(&v["open"])?,
        finite(&v["high"])?,
        finite(&v["low"])?,
        finite(&v["close"])?,
        finite(&v["volume"])?,
    );
    (start % 60000 == 0
        && low > 0.
        && high >= open.max(close)
        && low <= open.min(close)
        && volume >= 0.)
        .then_some(Bar {
            start,
            end: start.checked_add(60000)?,
            open,
            high,
            low,
            close,
            volume,
        })
}
fn aggregate(rows: &BTreeMap<u64, Option<Bar>>, open: u64, end: u64, tf: u64) -> Vec<Bar> {
    let mut bars = vec![];
    let step = tf * 60000;
    let mut t = open;
    while t + step <= end {
        let group: Option<Vec<&Bar>> = (0..tf)
            .map(|i| rows.get(&(t + i * 60000)).and_then(Option::as_ref))
            .collect();
        if let Some(g) = group {
            bars.push(Bar {
                start: t,
                end: t + step,
                open: g[0].open,
                close: g.last().unwrap().close,
                high: g.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max),
                low: g.iter().map(|b| b.low).fold(f64::INFINITY, f64::min),
                volume: g.iter().map(|b| b.volume).sum(),
            });
        }
        t += step;
    }
    bars
}
fn pending(symbol: &str, now: u64, reason: &str) -> Value {
    json!({"version":VERSION,"symbol":symbol,"at":now,"state":"pending","reason":reason,"layers":[],"executionEnabled":false,"strategyConnected":false,"probabilityClaim":false,"changesRiskLevels":false})
}
pub fn read(s: &Value, symbol: &str, now: u64) -> Value {
    let Some(date) = calendar::date_at(now) else {
        return pending(symbol, now, "Fecha inválida");
    };
    let Day::Open(day) = calendar::schedule(date) else {
        return pending(symbol, now, "Fuera de sesión conocida");
    };
    if now < day.open_ms || now >= day.close_ms {
        return pending(symbol, now, "Observador intradía RTH; fuera de sesión");
    }
    let fresh = |v: &Value, limit| {
        v.as_u64()
            .is_some_and(|t| t <= now && now - t <= limit && calendar::date_at(t) == Some(date))
    };
    if s["symbol"] != symbol
        || snipers_core::universe::stock_asset(symbol).is_none()
        || s["ticks"][symbol]["symbol"] != symbol
        || s["stocks"]["phase"] != "streaming"
        || !fresh(&s["publishedMs"], 3000)
        || !fresh(&s["ticks"][symbol]["tradeMs"], 5000)
        || !finite(&s["ticks"][symbol]["price"]).is_some_and(|p| p > 0.)
    {
        return pending(symbol, now, "Precio propio o feed no vigente");
    }
    let Some(values) = s["chartHistory"]["minutes"]
        .as_array()
        .filter(|v| v.len() <= 2400)
    else {
        return pending(symbol, now, "Historial ausente o fuera del límite");
    };
    let end = now.min(day.close_ms) / 60000 * 60000;
    let mut rows: BTreeMap<u64, Option<Bar>> = BTreeMap::new();
    for v in values {
        let Some(t) = v["timestampMs"].as_u64() else {
            continue;
        };
        if t >= end {
            continue;
        }
        if rows.contains_key(&t) {
            rows.insert(t, None);
        } else {
            rows.insert(t, minute(v));
        }
    }
    let mut refs = vec![];
    // A previous-day range is used only with exact full RTH minute coverage.
    if let Some(prior) =
        snipers_core::gap_engine::sessions(now, 1).and_then(|v| v.into_iter().last())
    {
        let complete: Option<Vec<_>> = (prior.open_ms..prior.close_ms)
            .step_by(60000)
            .map(|t| rows.get(&t).and_then(Option::as_ref))
            .collect();
        if let Some(g) = complete {
            refs.push(Reference {
                kind: "previous-day-high".into(),
                price: g.iter().map(|b| b.high).fold(0., f64::max),
                side: -1,
                available_at: day.open_ms,
            });
            refs.push(Reference {
                kind: "previous-day-low".into(),
                price: g.iter().map(|b| b.low).fold(f64::INFINITY, f64::min),
                side: 1,
                available_at: day.open_ms,
            });
        }
    }
    // Premarket is optional. Its absence in the RTH chart transport is explicitly reported.
    let pre = day.open_ms - 330 * 60000;
    let pre_rows: Option<Vec<_>> = (pre..day.open_ms)
        .step_by(60000)
        .map(|t| rows.get(&t).and_then(Option::as_ref))
        .collect();
    let premarket_available = pre_rows.is_some();
    if let Some(g) = pre_rows {
        refs.push(Reference {
            kind: "premarket-high".into(),
            price: g.iter().map(|b| b.high).fold(0., f64::max),
            side: -1,
            available_at: day.open_ms,
        });
        refs.push(Reference {
            kind: "premarket-low".into(),
            price: g.iter().map(|b| b.low).fold(f64::INFINITY, f64::min),
            side: 1,
            available_at: day.open_ms,
        });
    }
    let two = aggregate(&rows, day.open_ms, end, 2);
    let fifteen = aggregate(&rows, day.open_ms, end, 15);
    let required = if end >= day.open_ms + 120000 {
        Some(day.open_ms + (end - day.open_ms) / 120000 * 120000)
    } else {
        None
    };
    let active = required.is_some() && two.last().map(|b| b.end) == required;
    let layers: Vec<Layer> = [(2, two), (15, fifteen)]
        .into_iter()
        .map(|(tf, b)| smc_patterns::analyze(symbol, tf, &b, &refs))
        .collect();
    let fvg = crate::fvg::read(
        s,
        symbol,
        finite(&s["ticks"][symbol]["price"]).unwrap_or(0.),
        None,
        "",
        now,
    );
    let route_input: Vec<_> = values
        .iter()
        .filter_map(|v| serde_json::from_value::<crate::route_4h::Minute>(v.clone()).ok())
        .collect();
    let four_hour = crate::route_4h::read(symbol, &route_input, now);
    let mut links = vec![];
    for layer in &layers {
        for setup in &layer.setups {
            let Some(retest) = setup.retest_at else {
                continue;
            };
            for z in fvg["zones"].as_array().into_iter().flatten() {
                let created = z["createdMs"].as_u64().unwrap_or(u64::MAX);
                let own = z["symbol"] == symbol
                    && z["timeframe"] == layer.timeframe
                    && z["trackingComplete"] == true
                    && created >= setup.break_at
                    && created <= retest.saturating_sub(layer.timeframe * 60000)
                    && z["side"]
                        == if setup.side == 1 {
                            "BULLISH"
                        } else {
                            "BEARISH"
                        };
                if own
                    && finite(&z["low"])
                        .zip(finite(&z["high"]))
                        .is_some_and(|(lo, hi)| lo <= setup.upper && hi >= setup.lower)
                {
                    links.push(json!({"setupId":setup.id,"fvgId":z["id"],"availableAt":created,"role":"overlapping-context","extraVote":false}));
                }
            }
        }
    }
    json!({"version":VERSION,"symbol":symbol,"at":now,"state":if active{"observing"}else{"pending"},"session":day.date,"layers":layers,"fourHour":four_hour,"references":refs,"premarketAvailable":premarket_available,"fvg":fvg,"fvgLinks":links,"parameters":Config::default(),"reason":"Observador separado: patrones confirmados de precio; sin activar entradas o salidas","executionEnabled":false,"strategyConnected":false,"probabilityClaim":false,"changesRiskLevels":false})
}
/// Compare evidence to an existing strategy candidate. Never manufacture eligibility.
pub fn advisory(
    context: &Value,
    reading: &Value,
    anchor: Option<f64>,
    held_side: Option<&str>,
    now: u64,
) -> Value {
    let symbol = reading["symbol"].as_str().unwrap_or("");
    let valid = context["version"] == VERSION
        && context["symbol"] == symbol
        && context["state"] == "observing"
        && context["at"]
            .as_u64()
            .is_some_and(|t| t <= now && now - t <= 3000)
        && reading["at"]
            .as_u64()
            .is_some_and(|t| t <= now && now - t <= 5000);
    let entry = reading["entry_side"].as_str().unwrap_or("");
    let side = if entry == "CALL" {
        1
    } else if entry == "PUT" {
        -1
    } else {
        0
    };
    let mut supports = false;
    let mut opposes = false;
    let mut exit_review = false;
    if valid {
        for layer in context["layers"].as_array().into_iter().flatten() {
            let tf = layer["timeframe"].as_u64().unwrap_or(0);
            if ![2, 15].contains(&tf) {
                continue;
            }
            for s in layer["setups"].as_array().into_iter().flatten() {
                let recent = s["retestAt"]
                    .as_u64()
                    .is_some_and(|t| t <= now && now - t <= 2 * tf * 60000);
                if recent && s["state"] == "CONFIRMED" {
                    supports |= s["side"] == side;
                    opposes |= side != 0 && s["side"] != side;
                }
            }
            let held = held_side
                .map(|s| {
                    if s == "CALL" {
                        1
                    } else if s == "PUT" {
                        -1
                    } else {
                        0
                    }
                })
                .unwrap_or(0);
            if held != 0
                && (reading["management_ready"] == true || reading["ready"] == true)
                && anchor
                    .filter(|a| a.is_finite() && *a > 0.)
                    .zip(finite(&reading["spot"]))
                    .is_some_and(|(a, p)| (p - a) * (held as f64) < 0.)
            {
                exit_review |= layer["events"].as_array().into_iter().flatten().any(|e| {
                    e["at"]
                        .as_u64()
                        .is_some_and(|t| t >= layer["segmentStart"].as_u64().unwrap_or(u64::MAX))
                        && e["kind"] == "CHOCH"
                        && e["side"] == -held
                        && e["at"]
                            .as_u64()
                            .is_some_and(|t| t <= now && now - t <= 2 * tf * 60000)
                });
            }
        }
    }
    let q = &reading["quick_context"];
    let own = q["symbol"] == symbol
        && q["at"]
            .as_u64()
            .is_some_and(|t| t <= now && now - t <= 5000);
    let cvd = if valid && own {
        q["cvd"]["status"]
            .as_str()
            .filter(|s| matches!(*s, "confirms" | "contradicts"))
            .unwrap_or("pending")
    } else {
        "pending"
    };
    let flow = if valid && side != 0 {
        finite(&reading["flow_one"])
            .zip(finite(&reading["flow_five"]))
            .map(|(a, b)| {
                if a * (side as f64) >= 0.15 && b * (side as f64) > 0. {
                    "supports"
                } else if a * (side as f64) < 0. && b * (side as f64) < 0. {
                    "opposes"
                } else {
                    "mixed"
                }
            })
            .unwrap_or("pending")
    } else {
        "pending"
    };
    let four_hour = if valid {
        context["fourHour"]["structure"]
            .as_str()
            .filter(|s| matches!(*s, "up" | "down" | "range"))
            .unwrap_or("pending")
    } else {
        "pending"
    };
    json!({"version":"smc-advisory-v1","symbol":symbol,"at":now,"entryAssessment":if !valid||side==0{"pending"}else if supports&&opposes{"mixed"}else if opposes{"opposes"}else if supports{"supports"}else{"waiting"},"baselineEligible":reading["eligible"],"confirmations":{"cvd":cvd,"flow":flow,"fourHour":four_hour,"fvg":context["fvg"]["available"],"gaps":if own{q["gaps"].clone()}else{Value::Null}},"exitReviewSuggested":exit_review,"executionEnabled":false,"strategyConnected":false,"changesRiskLevels":false,"extraDirectionalVotes":0})
}
#[cfg(test)]
#[path = "smc_tests.rs"]
mod tests;
