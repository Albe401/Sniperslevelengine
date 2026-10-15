//! Causal, tick-based auction profile. Descriptive hypotheses, never calibrated forecasts.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const VERSION: &str = "auction-map-ticks-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    pub symbol: String,
    pub at: u64,
    /// Unique provider identity within this symbol/window, including venue if needed.
    pub id: String,
    pub price: f64,
    pub volume: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub symbol: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub as_of_ms: u64,
    pub bin_size: f64,
    pub bracket_ms: u64,
    pub value_fraction: f64,
    /// Explicit transport/backfill attestation, not inferred from the number of trades.
    pub complete_window: bool,
    /// Adapter must provide eligible, corrected underlying trades, not option prints.
    pub trades: Vec<Trade>,
    /// Closed one-minute underlying candles for observed reaction only.
    pub bars: Vec<crate::Candle>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bin {
    pub low: f64,
    pub high: f64,
    pub volume: f64,
    pub brackets: usize,
    pub volume_ratio: f64,
    pub time_ratio: f64,
    pub kind: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    pub direction: i32,
    pub destination_low: f64,
    pub destination_high: f64,
    pub trigger: String,
    pub invalidation: String,
    pub invalidation_level: f64,
    pub invalidation_close_direction: i32,
    pub probability: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub version: String,
    pub symbol: String,
    pub as_of_ms: u64,
    pub provisional: bool,
    pub accepted_trades: usize,
    pub duplicate_trades: usize,
    pub excluded_trades: usize,
    pub total_volume: f64,
    pub vwap: f64,
    pub poc_low: f64,
    pub poc_high: f64,
    pub val: f64,
    pub vah: f64,
    pub actual_value_fraction: f64,
    pub bins: Vec<Bin>,
    pub reaction: String,
    pub routes: Vec<Route>,
    pub calibrated: bool,
}
/// Window is [start,end); future events are excluded, never used to build today's profile.
pub fn read(input: &Input) -> Result<Reading, String> {
    let c = input;
    if c.symbol.trim().is_empty()
        || c.start_ms >= c.end_ms
        || c.as_of_ms < c.start_ms
        || !c.bin_size.is_finite()
        || c.bin_size <= 0.0
        || c.bracket_ms == 0
        || !c.value_fraction.is_finite()
        || !(0.0..=1.0).contains(&c.value_fraction)
        || c.value_fraction == 0.0
    {
        return Err("Configuración o ventana inválida".into());
    }
    let mut ids = BTreeMap::new();
    let mut bins: BTreeMap<i64, (f64, BTreeSet<u64>)> = BTreeMap::new();
    let (mut count, mut duplicates, mut excluded) = (0, 0, 0);
    let (mut total, mut weighted) = (0.0, 0.0);
    let mut ordered: Vec<_> = c.trades.iter().collect();
    ordered.sort_by(|a, b| a.at.cmp(&b.at).then(a.id.cmp(&b.id)));
    for t in ordered {
        if t.symbol != c.symbol || t.at < c.start_ms || t.at >= c.end_ms || t.at > c.as_of_ms {
            excluded += 1;
            continue;
        }
        if t.id.is_empty()
            || !t.price.is_finite()
            || t.price <= 0.0
            || !t.volume.is_finite()
            || t.volume <= 0.0
        {
            return Err("Operación inválida: no se publica un perfil con datos corruptos".into());
        }
        if let Some(previous) = ids.insert(&t.id, (t.at, t.price, t.volume)) {
            if previous != (t.at, t.price, t.volume) {
                return Err("Identidad duplicada con datos incompatibles".into());
            }
            duplicates += 1;
            continue;
        }
        let raw = (t.price / c.bin_size + 1e-9).floor();
        if raw > i64::MAX as f64 || raw < 0.0 {
            return Err("Precio fuera de rango".into());
        }
        let b = bins.entry(raw as i64).or_default();
        b.0 += t.volume;
        b.1.insert((t.at - c.start_ms) / c.bracket_ms);
        total += t.volume;
        weighted += t.price * t.volume;
        count += 1;
    }
    if count == 0 || !total.is_finite() || !weighted.is_finite() {
        return Err("Sin operaciones válidas o acumulación fuera de rango".into());
    }
    let low = *bins.first_key_value().unwrap().0;
    let high = *bins.last_key_value().unwrap().0;
    if high - low > 20_000 {
        return Err("Demasiados niveles: ampliar binSize".into());
    }
    let rows: Vec<_> = (low..=high)
        .map(|k| bins.get(&k).cloned().unwrap_or_default())
        .collect();
    let poc = rows
        .iter()
        .enumerate()
        .fold(0, |best, (i, b)| if b.0 > rows[best].0 { i } else { best });
    let (mut left, mut right, mut value) = (poc, poc, rows[poc].0);
    // Contiguous expansion to AT LEAST requested volume. Ties choose lower price.
    while value < c.value_fraction * total && (left > 0 || right + 1 < rows.len()) {
        if left > 0 && (right + 1 == rows.len() || rows[left - 1].0 >= rows[right + 1].0) {
            left -= 1;
            value += rows[left].0;
        } else {
            right += 1;
            value += rows[right].0;
        }
    }
    let max_time = rows.iter().map(|r| r.1.len()).max().unwrap_or(1) as f64;
    let output: Vec<Bin> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let v = r.0 / rows[poc].0;
            let t = r.1.len() as f64 / max_time;
            // Explicit research thresholds; scores are descriptive, not probabilities.
            let kind = if v >= 0.5 && t >= 0.5 && r.1.len() >= 2 {
                "acceptance"
            } else if r.0 == 0.0 {
                "untraded"
            } else if v <= 0.2 && t <= 0.5 {
                "low-participation"
            } else {
                "transition"
            };
            let low_price = (low + i as i64) as f64 * c.bin_size;
            Bin {
                low: low_price,
                high: low_price + c.bin_size,
                volume: r.0,
                brackets: r.1.len(),
                volume_ratio: v,
                time_ratio: t,
                kind: kind.into(),
            }
        })
        .collect();
    let val = output[left].low;
    let vah = output[right].high;
    let mut bars: Vec<_> = c
        .bars
        .iter()
        .filter(|b| {
            b.timestamp_ms >= c.start_ms
                && b.timestamp_ms
                    .checked_add(60_000)
                    .is_some_and(|end| end <= c.as_of_ms && end <= c.end_ms)
        })
        .collect();
    bars.sort_by_key(|b| b.timestamp_ms);
    for (i, b) in bars.iter().enumerate() {
        if [b.open, b.high, b.low, b.close]
            .iter()
            .any(|p| !p.is_finite() || *p <= 0.0)
            || b.high < b.open.max(b.close)
            || b.low > b.open.min(b.close)
            || b.low > b.high
            || (i > 0 && b.timestamp_ms <= bars[i - 1].timestamp_ms)
        {
            return Err("Velas cerradas inválidas o duplicadas".into());
        }
    }
    let mut reaction = "pending";
    let mut direction = 0;
    if bars.len() >= 3 {
        let n = bars.len();
        let (a, b, d) = (bars[n - 3], bars[n - 2], bars[n - 1]);
        // Never bridge a data gap or use stale closes as current confirmation.
        if b.timestamp_ms - a.timestamp_ms == 60_000
            && d.timestamp_ms - b.timestamp_ms == 60_000
            && c.as_of_ms.saturating_sub(d.timestamp_ms + 60_000) <= 60_000
        {
            if a.high >= vah && b.close < vah && d.close < vah && d.close >= val {
                reaction = "upper-rejection";
                direction = -1;
            } else if a.low < val && b.close >= val && d.close >= val && d.close < vah {
                reaction = "lower-rejection";
                direction = 1;
            } else if b.close >= vah && d.close >= vah {
                reaction = "upper-breakout-provisional";
                direction = 1;
            } else if b.close < val && d.close < val {
                reaction = "lower-breakout-provisional";
                direction = -1;
            } else {
                reaction = "inside-value";
            }
        }
    }
    let mut routes = Vec::new();
    if direction != 0 {
        let spot = bars.last().unwrap().close;
        let mut candidates: Vec<_> = output
            .iter()
            .filter(|b| {
                b.kind == "acceptance"
                    && if direction > 0 {
                        b.low > spot
                    } else {
                        b.high < spot
                    }
            })
            .collect();
        candidates.sort_by(|a, b| {
            ((a.low + a.high) / 2.0 - spot)
                .abs()
                .total_cmp(&((b.low + b.high) / 2.0 - spot).abs())
        });
        if let Some(target) = candidates.first() {
            routes.push(Route {
                direction,
                destination_low: target.low,
                destination_high: target.high,
                trigger: format!(
                    "{}: mantener el desplazamiento hacia la siguiente zona observada",
                    reaction
                ),
                invalidation: if direction > 0 {
                    "Anular con cierre bajo el nivel de invalidación".into()
                } else {
                    "Anular con cierre sobre el nivel de invalidación".into()
                },
                invalidation_level: match reaction {
                    "lower-rejection" => val,
                    "upper-rejection" => vah,
                    "upper-breakout-provisional" => vah,
                    _ => val,
                },
                invalidation_close_direction: -direction,
                probability: None,
            });
        }
    }
    Ok(Reading {
        version: VERSION.into(),
        symbol: c.symbol.clone(),
        as_of_ms: c.as_of_ms,
        provisional: !c.complete_window,
        accepted_trades: count,
        duplicate_trades: duplicates,
        excluded_trades: excluded,
        total_volume: total,
        vwap: weighted / total,
        poc_low: output[poc].low,
        poc_high: output[poc].high,
        val,
        vah,
        actual_value_fraction: value / total,
        bins: output,
        reaction: reaction.into(),
        routes,
        calibrated: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Input {
        Input {
            symbol: "QQQ".into(),
            start_ms: 0,
            end_ms: 600_000,
            as_of_ms: 300_000,
            bin_size: 1.0,
            bracket_ms: 60_000,
            value_fraction: 0.7,
            complete_window: false,
            trades: [(100.0, 10.0), (101.0, 100.0), (101.0, 100.0), (102.0, 30.0)]
                .iter()
                .enumerate()
                .map(|(i, &(price, volume))| Trade {
                    symbol: "QQQ".into(),
                    at: i as u64 * 60_000,
                    id: i.to_string(),
                    price,
                    volume,
                })
                .collect(),
            bars: vec![],
        }
    }
    #[test]
    fn volume_conservation_value_area_and_time() {
        let r = read(&fixture()).unwrap();
        assert_eq!(r.total_volume, 240.0);
        assert_eq!(r.bins.iter().map(|b| b.volume).sum::<f64>(), r.total_volume);
        assert_eq!(r.poc_low, 101.0);
        assert!(r.actual_value_fraction >= 0.7);
        assert_eq!(r.bins[1].brackets, 2);
        assert_eq!(r.bins[1].kind, "acceptance");
        assert!(r.provisional && !r.calibrated);
    }
    #[test]
    fn future_and_other_assets_never_change_profile() {
        let mut c = fixture();
        let r = read(&c).unwrap();
        let mut t = c.trades[0].clone();
        t.id = "future".into();
        t.at = 400_000;
        t.volume = 1e9;
        c.trades.push(t.clone());
        t.at = 200_000;
        t.symbol = "SPY".into();
        c.trades.push(t);
        let s = read(&c).unwrap();
        assert_eq!(s.bins, r.bins);
        assert_eq!(s.vwap, r.vwap);
        assert_eq!(s.excluded_trades, 2);
    }
    #[test]
    fn duplicate_dedup_and_conflicting_id_fails() {
        let mut c = fixture();
        c.trades.push(c.trades[0].clone());
        assert_eq!(read(&c).unwrap().total_volume, 240.0);
        c.trades.last_mut().unwrap().volume = 999.0;
        assert!(read(&c).is_err());
    }
    #[test]
    fn gaps_are_untraded_not_observed_rejection() {
        let mut c = fixture();
        c.trades[3].price = 104.0;
        let r = read(&c).unwrap();
        assert_eq!(r.bins[2].kind, "untraded");
        assert_eq!(r.reaction, "pending");
        assert!(r.routes.is_empty());
    }
    #[test]
    fn invalid_data_and_excessive_bins_fail() {
        let mut c = fixture();
        c.bin_size = f64::NAN;
        assert!(read(&c).is_err());
        c = fixture();
        c.trades[0].volume = -1.0;
        assert!(read(&c).is_err());
        c = fixture();
        c.trades[0].price = 1e8;
        assert!(read(&c).is_err());
    }
    #[test]
    fn ordering_and_serialization_are_stable() {
        let c = fixture();
        let first = read(&c).unwrap();
        let mut reversed = c.clone();
        reversed.trades.reverse();
        assert_eq!(read(&reversed).unwrap().bins, first.bins);
        let json = serde_json::to_string(&first).unwrap();
        let restored = serde_json::from_str::<Reading>(&json).unwrap();
        assert!((restored.vwap - first.vwap).abs() < 1e-10);
        assert_eq!(restored.bins, first.bins);
    }
    #[test]
    fn route_uses_observed_zone_and_explicit_invalidation() {
        let mut c = fixture();
        c.trades.extend([
            Trade {
                symbol: "QQQ".into(),
                at: 60_000,
                id: "a".into(),
                price: 104.0,
                volume: 110.0,
            },
            Trade {
                symbol: "QQQ".into(),
                at: 120_000,
                id: "b".into(),
                price: 104.0,
                volume: 110.0,
            },
        ]);
        c.value_fraction = 0.4;
        // POC is at 104, its lower-edge rejection points to that observed acceptance bin.
        c.bars = vec![
            bar(120_000, 103.5),
            bar(180_000, 104.1),
            bar(240_000, 104.2),
        ];
        let r = read(&c).unwrap();
        assert_eq!(r.reaction, "lower-rejection");
        // Already within destination: no invented additional target.
        assert!(r.routes.is_empty());
        c.trades[4].volume = 60.0;
        c.trades[5].volume = 60.0;
        c.bars = vec![
            bar(120_000, 100.5),
            bar(180_000, 101.1),
            bar(240_000, 101.2),
        ];
        let r = read(&c).unwrap();
        assert_eq!(r.routes.len(), 1);
        assert_eq!(r.routes[0].destination_low, 104.0);
        assert_eq!(r.routes[0].invalidation_level, 101.0);
        assert_eq!(r.routes[0].probability, None);
    }
    fn bar(at: u64, close: f64) -> crate::Candle {
        crate::Candle {
            timestamp_ms: at,
            open: close,
            high: close + 0.2,
            low: close - 0.2,
            close,
            volume: 1,
        }
    }
    #[test]
    fn observed_rejection_requires_closed_contiguous_fresh_bars() {
        let mut c = fixture();
        c.bars = vec![
            bar(120_000, 100.5),
            bar(180_000, 101.1),
            bar(240_000, 101.2),
        ];
        assert_eq!(read(&c).unwrap().reaction, "lower-rejection");
        c.bars[1].timestamp_ms = 190_000;
        assert_eq!(read(&c).unwrap().reaction, "pending");
        c.bars[1].timestamp_ms = 180_000;
        c.as_of_ms = 290_000;
        assert_eq!(read(&c).unwrap().reaction, "pending");
    }
}
