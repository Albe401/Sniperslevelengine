//! Three closed-candle price gaps. OHLC structure, never resting orders or probability.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use snipers_core::calendar::{date_at, schedule, Day};
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
struct Bar {
    t: u64,
    end: u64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    pub id: String,
    pub symbol: String,
    pub timeframe: u64,
    pub side: String,
    pub created_ms: u64,
    pub low: f64,
    pub midpoint: f64,
    pub high: f64,
    pub state: String,
    pub filled_fraction: f64,
    pub first_touch_ms: Option<u64>,
    pub filled_ms: Option<u64>,
    pub crossed_ms: Option<u64>,
    pub tracking_complete: bool,
    pub displacement: bool,
    pub confluence: Vec<String>,
    pub strength: String,
    pub distance: f64,
    pub seeking_fill: bool,
    pub support_retest: bool,
}
fn number(v: &Value) -> Option<f64> {
    v.as_f64().filter(|v| v.is_finite())
}
fn minute(v: &Value) -> Option<Bar> {
    let t = v["timestampMs"].as_u64()?;
    let (open, high, low, close, volume) = (
        number(&v["open"])?,
        number(&v["high"])?,
        number(&v["low"])?,
        number(&v["close"])?,
        number(&v["volume"])?,
    );
    (low > 0. && high >= open.max(close) && low <= open.min(close) && volume >= 0.).then_some(Bar {
        t,
        end: t.checked_add(60000)?,
        open,
        high,
        low,
        close,
    })
}
fn aggregate(rows: &BTreeMap<u64, Option<Bar>>, start: u64, end: u64, tf: u64) -> Vec<Bar> {
    let step = tf * 60000;
    let mut out = vec![];
    let mut t = start;
    while t.saturating_add(step) <= end {
        let group: Option<Vec<_>> = (0..tf)
            .map(|i| rows.get(&(t + i * 60000)).and_then(Option::as_ref))
            .collect();
        if let Some(g) = group {
            out.push(Bar {
                t,
                end: t + step,
                open: g[0].open,
                close: g.last().unwrap().close,
                high: g.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max),
                low: g.iter().map(|b| b.low).fold(f64::INFINITY, f64::min),
            });
        }
        t += step;
    }
    out
}
fn zones(
    symbol: &str,
    tf: u64,
    bars: &[Bar],
    rows: &BTreeMap<u64, Option<Bar>>,
    end: u64,
    spot: f64,
    refs: &[(String, f64)],
    last_two: Option<&Bar>,
) -> Vec<Zone> {
    let mut out = vec![];
    for triple in bars.windows(3) {
        let (a, b, c) = (&triple[0], &triple[1], &triple[2]);
        if a.end != b.t || b.end != c.t {
            continue;
        }
        let (side, low, high) = if c.low > a.high {
            ("BULLISH", a.high, c.low)
        } else if c.high < a.low {
            ("BEARISH", c.high, a.low)
        } else {
            continue;
        };
        let width = high - low;
        let sign = if side == "BULLISH" { 1. } else { -1. };
        // A gap without a directional middle candle remains weak context.
        let displacement = (b.close - b.open) * sign > 0.
            && (b.close - b.open).abs() >= (b.high - b.low) * 0.6
            && (b.close - b.open).abs() >= width;
        let mut z = Zone {
            id: format!("fvg-v1:{symbol}:{tf}:{}:{side}", c.end),
            symbol: symbol.into(),
            timeframe: tf,
            side: side.into(),
            created_ms: c.end,
            low,
            midpoint: (low + high) / 2.,
            high,
            state: "NEW".into(),
            filled_fraction: 0.,
            first_touch_ms: None,
            filled_ms: None,
            crossed_ms: None,
            tracking_complete: true,
            displacement,
            confluence: vec![],
            strength: "WEAK".into(),
            distance: if spot < low {
                low - spot
            } else if spot > high {
                spot - high
            } else {
                0.
            },
            seeking_fill: false,
            support_retest: false,
        };
        for t in (c.end..end).step_by(60000) {
            let Some(m) = rows.get(&t).and_then(Option::as_ref) else {
                z.tracking_complete = false;
                break;
            };
            let penetration = if sign > 0. {
                high - m.low
            } else {
                m.high - low
            };
            if penetration > 0. {
                z.first_touch_ms.get_or_insert(m.end);
                z.filled_fraction = z.filled_fraction.max((penetration / width).clamp(0., 1.));
            }
            if z.filled_fraction >= 1. {
                z.filled_ms.get_or_insert(m.end);
            }
            if (m.close - if sign > 0. { low } else { high }) * sign < 0. {
                z.crossed_ms.get_or_insert(m.end);
            }
        }
        z.state = if z.crossed_ms.is_some() {
            "CROSSED"
        } else if z.filled_ms.is_some() {
            "FILLED"
        } else if !z.tracking_complete {
            "UNKNOWN"
        } else if z.first_touch_ms.is_some() {
            "PARTIAL"
        } else {
            "NEW"
        }
        .into();
        let tolerance = spot * 0.0003;
        z.confluence = refs
            .iter()
            .filter(|(_, p)| *p >= low - tolerance && *p <= high + tolerance)
            .map(|(name, _)| name.clone())
            .collect();
        z.strength = if displacement && !z.confluence.is_empty() {
            "STRONG"
        } else if displacement || !z.confluence.is_empty() {
            "MODERATE"
        } else {
            "WEAK"
        }
        .into();
        if let Some(last) =
            last_two.filter(|last| last.t >= c.end && end.saturating_sub(last.end) <= 120000)
        {
            z.seeking_fill = matches!(z.state.as_str(), "NEW" | "PARTIAL")
                && (last.close - last.open) * sign < 0.
                && (spot - z.midpoint) * sign > 0.;
            z.support_retest = z.state == "PARTIAL"
                && if sign > 0. {
                    last.low < high && last.close > high && spot >= high
                } else {
                    last.high > low && last.close < low && spot <= low
                };
        }
        out.push(z);
    }
    // Prefer the latest evidence, bounded independently per timeframe.
    out.into_iter().rev().take(20).collect()
}
pub fn read(
    s: &Value,
    symbol: &str,
    spot: f64,
    target: Option<f64>,
    side: &str,
    now: u64,
) -> Value {
    let pending = || json!({"version":"fvg-v1","symbol":symbol,"at":now,"available":false,"zones":[],"alignment":"pending","probabilityClaim":false,"executionEnabled":false});
    if s["symbol"] != symbol || !spot.is_finite() || spot <= 0. {
        return pending();
    }
    let Some(date) = date_at(now) else {
        return pending();
    };
    let Day::Open(day) = schedule(date) else {
        return pending();
    };
    if now < day.open_ms || now >= day.close_ms {
        return pending();
    }
    let end = now / 60000 * 60000;
    let mut rows = BTreeMap::new();
    for v in s["chartHistory"]["minutes"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let Some(t) = v["timestampMs"].as_u64() else {
            continue;
        };
        if t < day.open_ms || t >= end || (t - day.open_ms) % 60000 != 0 {
            continue;
        }
        if rows.insert(t, minute(v)).is_some() {
            rows.insert(t, None);
        }
    }
    let two = aggregate(&rows, day.open_ms, end, 2);
    let mut refs: Vec<(String, f64)> = vec![];
    // Options references are present-day context, never attributed to the historical gap's creation.
    let snapshot = &s["optionSnapshotStatus"];
    if snapshot["symbol"].as_str().is_none_or(|own| own == symbol)
        && snapshot["phase"] == "ready"
        && snapshot["observedMs"]
            .as_u64()
            .is_some_and(|t| t <= now && now - t <= 900000)
    {
        if let Some(h) = snapshot["horizons"].as_array().and_then(|hs| {
            hs.iter().find(|h| {
                h["mode"] == "DAY"
                    && h["observedMs"] == snapshot["observedMs"]
                    && h["expirations"] == json!([day.date])
                    && h["structure"]["complete"] == true
            })
        }) {
            let st = &h["structure"];
            for (name, v) in [
                ("Put Wall", &st["putWall"]["strike"]),
                ("Call Wall", &st["callWall"]["strike"]),
                ("Main Magnet", &st["selected"]["M1"]),
            ] {
                if let Some(p) = number(v).filter(|p| *p > 0.) {
                    refs.push((name.into(), p));
                }
            }
            for r in st["routes"].as_array().into_iter().flatten() {
                if r["structuralReady"] == true {
                    if let Some(p) = number(&r["barrier"]).filter(|p| *p > 0.) {
                        refs.push(("Liquidity Barrier".into(), p));
                    }
                }
            }
            for r in st["top"].as_array().into_iter().flatten() {
                if st["gexAvailable"] == true && number(&r["score"]).is_some_and(|v| v >= 60.) {
                    if let Some(p) = number(&r["strike"]).filter(|p| *p > 0.) {
                        refs.push(("GEX reference".into(), p));
                    }
                }
            }
        }
    }
    if s["metrics"]["session"] == day.date
        && s["metrics"]["asofMs"]
            .as_u64()
            .is_some_and(|t| t <= now && now - t <= 180000)
    {
        if let Some(v) = number(&s["metrics"]["vwap"]).filter(|p| *p > 0.) {
            refs.push(("VWAP".into(), v));
        }
    }
    for p in two.windows(5) {
        let b = &p[2];
        if p.windows(2).all(|pair| pair[0].end == pair[1].t) {
            if p.iter().enumerate().all(|(i, a)| i == 2 || a.high < b.high) {
                refs.push(("Confirmed pivot".into(), b.high));
            }
            if p.iter().enumerate().all(|(i, a)| i == 2 || a.low > b.low) {
                refs.push(("Confirmed pivot".into(), b.low));
            }
        }
    }
    let mut all = zones(symbol, 2, &two, &rows, end, spot, &refs, two.last());
    all.extend(zones(
        symbol,
        15,
        &aggregate(&rows, day.open_ms, end, 15),
        &rows,
        end,
        spot,
        &refs,
        two.last(),
    ));
    let available = s["stocks"]["phase"] == "streaming"
        && s["publishedMs"]
            .as_u64()
            .is_some_and(|t| t <= now && now - t <= 3000)
        && s["ticks"][symbol]["tradeMs"]
            .as_u64()
            .is_some_and(|t| t <= now.saturating_add(1000) && now.saturating_sub(t) <= 15000);
    let sign = if side == "CALL" {
        1.
    } else if side == "PUT" {
        -1.
    } else {
        0.
    };
    let obstacle = all
        .iter()
        .filter(|z| {
            available
                && sign != 0.
                && z.strength == "STRONG"
                && matches!(z.state.as_str(), "NEW" | "PARTIAL")
                && ((sign > 0. && z.side == "BEARISH") || (sign < 0. && z.side == "BULLISH"))
        })
        .filter(|z| {
            target.is_some_and(|t| {
                t.is_finite()
                    && (t - spot) * sign > 0.
                    && if sign > 0. {
                        z.high > spot && z.low < t
                    } else {
                        z.low < spot && z.high > t
                    }
            })
        })
        .min_by(|a, b| a.distance.total_cmp(&b.distance));
    let support = all
        .iter()
        .filter(|z| {
            available
                && z.support_retest
                && ((sign > 0. && z.side == "BULLISH") || (sign < 0. && z.side == "BEARISH"))
        })
        .min_by(|a, b| a.distance.total_cmp(&b.distance));
    let selected = obstacle.or(support).or_else(|| {
        all.iter()
            .filter(|z| matches!(z.state.as_str(), "NEW" | "PARTIAL"))
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    });
    json!({"version":"fvg-v1","symbol":symbol,"at":now,"available":available,"zones":all,"selected":selected,"alignment":if !available{"pending"}else if obstacle.is_some(){"opposing-path"}else if support.is_some(){"supports-pullback"}else{"context"},"pathBlocked":obstacle.is_some(),"confluenceAt":now,"probabilityClaim":false,"executionEnabled":false,"changesRiskLevels":false})
}
pub fn apply(s: &Value, r: &mut crate::intraday_strategy::Reading, now: u64) {
    let context = read(s, &r.symbol, r.spot, r.next, &r.entry_side, now);
    if r.ready && r.eligible && context["pathBlocked"] == true {
        r.eligible = false;
        r.blockers.push(
            "FVG contrario fuerte entre precio y objetivo: esperar despeje y nueva confirmación 2M"
                .into(),
        );
    }
    r.quick_context["fvg"] = context;
}
#[cfg(test)]
#[path = "fvg_tests.rs"]
mod tests;
