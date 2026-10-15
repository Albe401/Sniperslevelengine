//! Off-exchange concentration, not an identification of the executing ATS or investor.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MIN_NOTIONAL: f64 = 100_000.0;
pub const QUOTE_MAX_AGE_MS: u64 = 2_000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Side {
    Buy,
    Sell,
    BuyLean,
    SellLean,
    Neutral,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Print {
    pub price: f64,
    pub shares: f64,
    pub notional: f64,
    pub level: f64,
    pub side: Side,
    pub time_et: String,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Level {
    pub level: f64,
    pub prints: usize,
    pub shares: f64,
    pub notional: f64,
    pub buy_notional: f64,
    pub sell_notional: f64,
    pub neutral_notional: f64,
    pub largest_print: f64,
    pub last_time: String,
    pub pressure_pct: f64,
    pub concentration_pct: f64,
    pub bias: String,
    pub strength: String,
    pub strength_score: f64,
    pub strength_data_sufficient: bool,
    pub strength_prints_pct: f64,
    pub strength_relative_notional: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    pub timestamp_ms: u64,
}
fn positive(v: f64) -> bool {
    v.is_finite() && v > 0.0
}
fn rounded(v: f64, places: usize) -> f64 {
    format!("{v:.places$}")
        .parse()
        .expect("finite numeric result")
}
pub fn bucket(price: f64) -> Option<f64> {
    positive(price)
        .then(|| rounded((price / 0.25).round_ties_even() * 0.25, 4))
        .filter(|v| v.is_finite() && *v > 0.0)
}
pub fn tier(notional: f64) -> Option<&'static str> {
    if !notional.is_finite() || notional < MIN_NOTIONAL {
        None
    } else if notional >= 1_000_000.0 {
        Some("BLOCK_1M_PLUS")
    } else if notional >= 500_000.0 {
        Some("MAJOR_500K_PLUS")
    } else {
        Some("BIG_100K_PLUS")
    }
}
pub fn classify(price: f64, trade_ms: u64, quote: Option<Quote>) -> Side {
    let Some(q) = quote else { return Side::Unknown };
    if !positive(price)
        || !positive(q.bid)
        || !positive(q.ask)
        || q.ask <= q.bid
        || q.timestamp_ms > trade_ms
        || trade_ms - q.timestamp_ms > QUOTE_MAX_AGE_MS
    {
        return Side::Unknown;
    }
    let spread = (q.ask - q.bid).max(0.0001);
    let tolerance = (spread * 0.10).max(0.001);
    let midpoint = q.bid + (q.ask - q.bid) / 2.0;
    if price >= q.ask - tolerance {
        Side::Buy
    } else if price <= q.bid + tolerance {
        Side::Sell
    } else if price > midpoint {
        Side::BuyLean
    } else if price < midpoint {
        Side::SellLean
    } else {
        Side::Neutral
    }
}
pub fn build_levels(events: &[Print]) -> Result<Vec<Level>, &'static str> {
    if events.len() > 500_000
        || events.iter().any(|e| {
            !positive(e.price)
                || !positive(e.shares)
                || !positive(e.notional)
                || !positive(e.level)
                || !(e.price * e.shares).is_finite()
                || ((e.price * e.shares) - e.notional).abs() > 1e-8 * e.notional
                || bucket(e.price) != Some(e.level)
        })
    {
        return Err("Print inválido o capacidad excedida");
    }
    let total: f64 = events.iter().map(|e| e.notional).sum();
    let shares: f64 = events.iter().map(|e| e.shares).sum();
    if !total.is_finite() || !shares.is_finite() {
        return Err("Desbordamiento de nocional/volumen");
    }
    // Preserve arrival order for equal-notional/equal-count legacy ties.
    let mut indices = BTreeMap::new();
    let mut levels: Vec<Level> = vec![];
    for e in events {
        let index = *indices.entry(e.level.to_bits()).or_insert_with(|| {
            levels.push(Level {
                level: e.level,
                ..Default::default()
            });
            levels.len() - 1
        });
        let row = &mut levels[index];
        row.prints += 1;
        row.shares += e.shares;
        row.notional += e.notional;
        row.largest_print = row.largest_print.max(e.notional);
        row.last_time = e.time_et.clone();
        match e.side {
            Side::Buy | Side::BuyLean => row.buy_notional += e.notional,
            Side::Sell | Side::SellLean => row.sell_notional += e.notional,
            _ => row.neutral_notional += e.notional,
        }
    }
    decorate(&mut levels)?;
    Ok(levels)
}
/// Recompute bounded level metrics after an incremental aggregation.
pub fn decorate(levels: &mut [Level]) -> Result<(), &'static str> {
    if levels.is_empty() {
        return Ok(());
    }
    if levels.len() > 20_000
        || levels.iter().any(|r| {
            !positive(r.level)
                || r.prints == 0
                || !positive(r.notional)
                || !positive(r.shares)
                || [
                    r.buy_notional,
                    r.sell_notional,
                    r.neutral_notional,
                    r.largest_print,
                ]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0)
        })
    {
        return Err("Nivel agregado inválido");
    }
    let total: f64 = levels.iter().map(|r| r.notional).sum();
    let prints: f64 = levels.iter().map(|r| r.prints as f64).sum();
    if !total.is_finite() || total <= 0.0 || !prints.is_finite() || prints <= 0.0 {
        return Err("Totales de niveles inválidos");
    }
    let count = levels.len();
    let mut notionals: Vec<_> = levels.iter().map(|r| r.notional).collect();
    notionals.sort_by(f64::total_cmp);
    let median = if count.is_multiple_of(2) {
        notionals[count / 2 - 1] / 2.0 + notionals[count / 2] / 2.0
    } else {
        notionals[count / 2]
    };
    for r in levels.iter_mut() {
        let directional = r.buy_notional + r.sell_notional;
        r.pressure_pct = if directional > 0.0 {
            (r.buy_notional - r.sell_notional) / directional * 100.0
        } else {
            0.0
        };
        r.concentration_pct = r.notional / total * 100.0;
        r.bias = if r.pressure_pct >= 15.0 {
            "BUY_PRESSURE"
        } else if r.pressure_pct <= -15.0 {
            "SELL_PRESSURE"
        } else {
            "BALANCED"
        }
        .into();
        let print_pct = r.prints as f64 / prints * 100.0;
        let relative = if median > 0.0 {
            r.notional / median
        } else {
            0.0
        };
        if !relative.is_finite() {
            return Err("Desbordamiento de concentración relativa");
        }
        let score = 100.0
            * (0.35 * (r.concentration_pct / 40.0).min(1.0)
                + 0.25 * (print_pct / 40.0).min(1.0)
                + 0.40 * ((relative - 1.0).max(0.0) / 4.0).min(1.0));
        r.strength_score = rounded(score.clamp(0.0, 100.0), 1);
        r.strength_data_sufficient = count >= 3 && prints >= 10.0 && r.prints >= 3;
        r.strength = if !r.strength_data_sufficient {
            "INSUFFICIENT_DATA"
        } else if r.strength_score >= 80.0 {
            "EXTREME"
        } else if r.strength_score >= 55.0 {
            "HIGH"
        } else if r.strength_score >= 30.0 {
            "MEDIUM"
        } else {
            "LOW"
        }
        .into();
        r.strength_prints_pct = rounded(print_pct, 2);
        r.strength_relative_notional = rounded(relative, 2);
    }
    levels.sort_by(|a, b| {
        b.notional
            .total_cmp(&a.notional)
            .then(b.prints.cmp(&a.prints))
    });
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    #[test]
    fn legacy_concentration_side_buckets_and_tiers() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-off-exchange.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let events: Vec<Print> = serde_json::from_value(case["events"].clone()).unwrap();
            let actual = serde_json::to_value(build_levels(&events).unwrap()).unwrap();
            let expected = case["levels"].as_array().unwrap();
            assert_eq!(actual.as_array().unwrap().len(), expected.len());
            for (a, e) in actual.as_array().unwrap().iter().zip(expected) {
                for (key, value) in a.as_object().unwrap() {
                    if let Some(n) = value.as_f64() {
                        let wanted = e[key].as_f64().unwrap();
                        assert!(
                            (n - wanted).abs() < 1e-8 * wanted.abs().max(1.0),
                            "{key}: {n} != {wanted}"
                        );
                    } else {
                        assert_eq!(value, &e[key], "{key}");
                    }
                }
            }
        }
        for c in fixture["sides"].as_array().unwrap() {
            let bid = c["bid"].as_f64().unwrap();
            let ask = c["ask"].as_f64().unwrap();
            let actual = classify(
                c["price"].as_f64().unwrap(),
                1000,
                Some(Quote {
                    bid,
                    ask,
                    timestamp_ms: 1000,
                }),
            );
            // A locked NBBO does not identify an aggressor, unlike the old BUY-first branch.
            if bid == ask {
                assert_eq!(actual, Side::Unknown);
            } else {
                assert_eq!(serde_json::to_value(actual).unwrap(), c["result"]);
            }
        }
        for c in fixture["buckets"].as_array().unwrap() {
            assert_eq!(bucket(c["price"].as_f64().unwrap()), c["result"].as_f64());
        }
        for c in fixture["tiers"].as_array().unwrap() {
            assert_eq!(tier(c["notional"].as_f64().unwrap()), c["result"].as_str());
        }
    }
    #[test]
    fn stale_future_missing_or_crossed_quotes_are_unknown() {
        for quote in [
            None,
            Some(Quote {
                bid: 499.0,
                ask: 501.0,
                timestamp_ms: 1001,
            }),
            Some(Quote {
                bid: 499.0,
                ask: 501.0,
                timestamp_ms: 0,
            }),
            Some(Quote {
                bid: 501.0,
                ask: 499.0,
                timestamp_ms: 1000,
            }),
        ] {
            let time = if quote.is_some_and(|q| q.timestamp_ms == 0) {
                2001
            } else {
                1000
            };
            assert_eq!(classify(501.0, time, quote), Side::Unknown);
        }
        assert_eq!(
            classify(
                501.0,
                3000,
                Some(Quote {
                    bid: 499.0,
                    ask: 501.0,
                    timestamp_ms: 1000
                })
            ),
            Side::Buy
        );
        assert_eq!(tier(99_999.99), None);
        assert_eq!(bucket(f64::MAX), None);
        let invalid = Print {
            price: 500.0,
            shares: 200.0,
            notional: 1_000_000.0,
            level: 500.0,
            side: Side::Unknown,
            time_et: "10:00".into(),
        };
        assert!(build_levels(&[invalid]).is_err());
    }
}
