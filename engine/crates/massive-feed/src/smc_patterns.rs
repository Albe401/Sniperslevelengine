//! Operational OHLC definitions; hypotheses, not evidence of institutional orders.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bar {
    pub start: u64,
    pub end: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pivot {
    pub kind: String,
    pub price: f64,
    pub at: u64,
    pub available_at: u64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub kind: String,
    pub price: f64,
    pub side: i8,
    pub available_at: u64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub kind: String,
    pub side: i8,
    pub at: u64,
    pub level: f64,
    pub displacement: bool,
    pub atr: Option<f64>,
    pub relative_volume: Option<f64>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderBlock {
    pub id: String,
    pub side: i8,
    pub lower: f64,
    pub upper: f64,
    pub origin_at: u64,
    pub available_at: u64,
    pub state: String,
    pub first_touch: Option<u64>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setup {
    pub id: String,
    pub kind: String,
    pub side: i8,
    pub break_at: u64,
    pub sweep_at: Option<u64>,
    pub level: f64,
    pub lower: f64,
    pub upper: f64,
    pub invalidation: f64,
    pub state: String,
    pub retest_at: Option<u64>,
    pub order_block_id: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub timeframe: u64,
    pub segment_start: Option<u64>,
    pub trend: i8,
    pub pivots: Vec<Pivot>,
    pub events: Vec<Event>,
    pub order_blocks: Vec<OrderBlock>,
    pub setups: Vec<Setup>,
    pub last_closed: Option<u64>,
}
fn atr(bars: &[Bar]) -> Option<f64> {
    if bars.len() < 15 {
        return None;
    }
    let rows = &bars[bars.len() - 15..];
    Some(
        rows.windows(2)
            .map(|p| {
                (p[1].high - p[1].low)
                    .max((p[1].high - p[0].close).abs())
                    .max((p[1].low - p[0].close).abs())
            })
            .sum::<f64>()
            / 14.,
    )
}
pub fn analyze(symbol: &str, timeframe: u64, bars: &[Bar], refs: &[Reference]) -> Layer {
    let mut out = Layer {
        timeframe,
        segment_start: bars.first().map(|b| b.start),
        trend: 0,
        pivots: vec![],
        events: vec![],
        order_blocks: vec![],
        setups: vec![],
        last_closed: bars.last().map(|b| b.end),
    };
    let mut broken = BTreeSet::new();
    let mut swept = BTreeSet::new();
    for (i, b) in bars.iter().enumerate() {
        // Strict continuity also breaks pending sequences across missing provider minutes.
        if i > 0 && bars[i - 1].end != b.start {
            out.segment_start = Some(b.start);
            out.trend = 0;
            out.pivots.clear();
            broken.clear();
            swept.clear();
            for s in &mut out.setups {
                if matches!(s.state.as_str(), "WAIT_RETEST" | "CONFIRMED") {
                    s.state = "DATA_GAP".into();
                }
            }
            for ob in &mut out.order_blocks {
                if matches!(ob.state.as_str(), "FRESH" | "TOUCHED") {
                    ob.state = "DATA_GAP".into();
                }
            }
        }
        // Only an unbroken trailing window may establish magnitude and participation.
        let trailing = bars[..i]
            .iter()
            .rev()
            .take_while({
                let mut next = b.start;
                move |p| {
                    let ok = p.end == next;
                    next = p.start;
                    ok
                }
            })
            .count();
        let history = &bars[i - trailing..i];
        let magnitude = atr(history).filter(|a| *a > 0.);
        let rv = (history.len() >= 10)
            .then(|| {
                let n = history.len().min(20);
                let mean = history[history.len() - n..]
                    .iter()
                    .map(|p| p.volume)
                    .sum::<f64>()
                    / n as f64;
                if mean > 0. {
                    Some(b.volume / mean)
                } else {
                    None
                }
            })
            .flatten();
        let tolerance = magnitude
            .map(|a| a * 0.05)
            .unwrap_or(b.close * 0.0001)
            .max(0.01);
        // Confirm a five-bar pivot only now. It is eligible starting with a later bar.
        if i >= 4 && bars[i - 4..=i].windows(2).all(|p| p[0].end == p[1].start) {
            let p = &bars[i - 2];
            for (kind, price, is_pivot) in [
                (
                    "high",
                    p.high,
                    bars[i - 4..=i].iter().enumerate().all(|(j, q)| {
                        j == 2
                            || if j < 2 {
                                q.high < p.high
                            } else {
                                q.high <= p.high
                            }
                    }),
                ),
                (
                    "low",
                    p.low,
                    bars[i - 4..=i]
                        .iter()
                        .enumerate()
                        .all(|(j, q)| j == 2 || if j < 2 { q.low > p.low } else { q.low >= p.low }),
                ),
            ] {
                if is_pivot {
                    out.pivots.push(Pivot {
                        kind: kind.into(),
                        price,
                        at: p.start,
                        available_at: b.end,
                    });
                }
            }
        }
        let highs: Vec<_> = out
            .pivots
            .iter()
            .filter(|p| p.kind == "high" && p.available_at <= b.start)
            .collect();
        let lows: Vec<_> = out
            .pivots
            .iter()
            .filter(|p| p.kind == "low" && p.available_at <= b.start)
            .collect();
        if out.trend == 0 && highs.len() >= 2 && lows.len() >= 2 {
            let h = highs[highs.len() - 1].price - highs[highs.len() - 2].price;
            let l = lows[lows.len() - 1].price - lows[lows.len() - 2].price;
            out.trend = if h > tolerance && l > tolerance {
                1
            } else if h < -tolerance && l < -tolerance {
                -1
            } else {
                0
            };
        }
        let mut levels: Vec<Reference> = refs
            .iter()
            .filter(|r| r.available_at <= b.start)
            .cloned()
            .collect();
        for ps in [&highs, &lows] {
            if let Some(p) = ps.last() {
                let equal = ps.len() >= 2 && (p.price - ps[ps.len() - 2].price).abs() <= tolerance;
                levels.push(Reference {
                    kind: if equal {
                        format!("equal-{}", p.kind)
                    } else {
                        format!("pivot-{}", p.kind)
                    },
                    price: p.price,
                    side: if p.kind == "high" { -1 } else { 1 },
                    available_at: p.available_at,
                });
            }
        }
        for r in levels {
            let key = format!("{}:{}:{}", r.kind, r.price.to_bits(), r.available_at);
            let cross = if r.side == 1 {
                b.low < r.price - tolerance
                    && b.open >= r.price - tolerance
                    && b.close > r.price + tolerance
            } else {
                b.high > r.price + tolerance
                    && b.open <= r.price + tolerance
                    && b.close < r.price - tolerance
            };
            if cross && swept.insert(key) {
                out.events.push(Event {
                    id: format!(
                        "smc-v1:{symbol}:{timeframe}:sweep:{}:{}:{}:{}",
                        b.end,
                        r.side,
                        r.kind,
                        r.price.to_bits()
                    ),
                    kind: "SWEEP".into(),
                    side: r.side,
                    at: b.end,
                    level: r.price,
                    displacement: false,
                    atr: magnitude,
                    relative_volume: rv,
                });
            }
        }
        // Lifecycle before new creation: the displacement bar cannot retest its own block.
        for ob in &mut out.order_blocks {
            if !matches!(ob.state.as_str(), "FRESH" | "TOUCHED") || ob.available_at > b.start {
                continue;
            }
            if b.end.saturating_sub(ob.available_at) > 12 * timeframe * 60000 {
                ob.state = "EXPIRED".into();
                continue;
            }
            if (b.close - if ob.side == 1 { ob.lower } else { ob.upper }) * (ob.side as f64) < 0. {
                ob.state = "INVALIDATED".into();
            } else if b.low <= ob.upper && b.high >= ob.lower {
                ob.state = "TOUCHED".into();
                ob.first_touch.get_or_insert(b.end);
            }
        }
        for s in &mut out.setups {
            if !matches!(s.state.as_str(), "WAIT_RETEST" | "CONFIRMED") {
                continue;
            }
            if (b.close - s.invalidation) * (s.side as f64) < 0. {
                s.state = "INVALIDATED".into();
                continue;
            }
            if s.state == "WAIT_RETEST" && b.end.saturating_sub(s.break_at) > 12 * timeframe * 60000
            {
                s.state = "EXPIRED".into();
                continue;
            }
            if s.state == "WAIT_RETEST"
                && s.break_at <= b.start
                && b.low <= s.upper + tolerance
                && b.high >= s.lower - tolerance
                && (b.close - s.level) * (s.side as f64) > tolerance
                && (b.close - b.open) * (s.side as f64) > 0.
            {
                s.state = "CONFIRMED".into();
                s.retest_at = Some(b.end);
            }
        }
        for (side, pivot) in [(1, highs.last().copied()), (-1, lows.last().copied())] {
            let Some(p) = pivot else {
                continue;
            };
            let key = (side, p.at);
            if (b.close - p.price) * (side as f64) <= tolerance || !broken.insert(key) {
                continue;
            }
            let kind = if out.trend != 0 && out.trend != side {
                "CHOCH"
            } else {
                "BOS"
            };
            let displacement = magnitude
                .is_some_and(|a| (b.close - b.open) * (side as f64) >= 0.8 * a)
                && (b.close - b.open).abs() >= (b.high - b.low) * 0.6
                && rv.is_some_and(|v| v >= 1.2);
            let id = format!("smc-v1:{symbol}:{timeframe}:{kind}:{}:{side}", b.end);
            out.events.push(Event {
                id: id.clone(),
                kind: kind.into(),
                side,
                at: b.end,
                level: p.price,
                displacement,
                atr: magnitude,
                relative_volume: rv,
            });
            let sweep = out
                .events
                .iter()
                .rev()
                .find(|e| {
                    e.kind == "SWEEP"
                        && e.side == side
                        && e.at < b.end
                        && b.end - e.at <= 6 * timeframe * 60000
                })
                .map(|e| e.at);
            if displacement && (kind == "BOS" || sweep.is_some()) {
                let ob = history
                    .iter()
                    .rev()
                    .take(5)
                    .find(|q| (q.close - q.open) * (side as f64) < 0.);
                let (lower, upper, invalidation, ob_id) = if let Some(q) = ob {
                    let oid = format!("{id}:ob");
                    out.order_blocks.push(OrderBlock {
                        id: oid.clone(),
                        side,
                        lower: q.low,
                        upper: q.high,
                        origin_at: q.start,
                        available_at: b.end,
                        state: "FRESH".into(),
                        first_touch: None,
                    });
                    (
                        q.low,
                        q.high,
                        if side == 1 { q.low } else { q.high },
                        Some(oid),
                    )
                } else {
                    (
                        p.price,
                        p.price,
                        if side == 1 { b.low } else { b.high },
                        None,
                    )
                };
                out.setups.push(Setup {
                    id,
                    kind: if kind == "CHOCH" {
                        "REVERSAL"
                    } else {
                        "CONTINUATION"
                    }
                    .into(),
                    side,
                    break_at: b.end,
                    sweep_at: sweep,
                    level: p.price,
                    lower,
                    upper,
                    invalidation,
                    state: "WAIT_RETEST".into(),
                    retest_at: None,
                    order_block_id: ob_id,
                });
            }
            out.trend = side;
        }
    }
    // Keep last confirmed pivots plus bounded evidence, never unlimited history per tick.
    if out.pivots.len() > 64 {
        out.pivots.drain(..out.pivots.len() - 64);
    }
    if out.events.len() > 64 {
        out.events.drain(..out.events.len() - 64);
    }
    if out.order_blocks.len() > 32 {
        out.order_blocks.drain(..out.order_blocks.len() - 32);
    }
    if out.setups.len() > 32 {
        out.setups.drain(..out.setups.len() - 32);
    }
    out
}
