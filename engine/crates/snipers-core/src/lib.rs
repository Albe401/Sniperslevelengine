//! Deterministic domain types. No network, UI, credentials, or broker side effects.
use serde::{Deserialize, Serialize};
pub mod auction_map;
pub mod auction_rejection;
pub mod analyst;
pub mod atr;
pub mod bars;
pub mod block_pressure;
pub mod block_alerts;
pub mod calendar;
pub mod cvd;
pub mod stock_volume_policy;
pub mod execution_evidence;
pub mod execution_absorption;
pub mod daily_range;
pub mod decision_prepare;
pub mod decision_score;
pub mod decision_source;
pub mod dex_change;
pub mod exposure;
pub mod flow;
pub mod horizons;
pub mod legacy_levels;
pub mod levels;
pub mod liquidity;
pub mod magnet_confirmation;
pub mod magnet_reclaim;
pub mod magnet_trigger;
pub mod off_exchange;
pub mod off_exchange_tape;
pub mod off_exchange_trade;
pub mod option_liquidity;
pub mod option_activity_baseline;
pub mod option_activity_detector;
pub mod option_premium_detector;
pub mod option_trial_training;
pub mod options;
pub mod option_data_coverage;
pub mod paper;
pub mod paper_performance;
pub mod position_map;
pub mod premarket;
pub mod reading;
pub mod reading_overlay;
pub mod scanner;
pub mod scanner_confirmation;
pub mod scanner_memory;
pub mod scanner_selection;
pub mod scanner_structure;
pub mod scanner_trigger;
pub mod sector_activity;
pub mod session_guard;
pub mod structural_path;
pub mod structure;
pub mod synthetic;
pub mod universe;
pub mod wall_flow;
pub mod wall_pull;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candle {
    pub timestamp_ms: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub label: String,
    pub price: f64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoAsset {
    pub symbol: String,
    pub name: String,
    pub previous_close: f64,
    pub candles: Vec<Candle>,
    pub levels: Vec<Level>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoSnapshot {
    pub schema_version: u32,
    pub mode: String,
    pub session_label: String,
    pub assets: Vec<DemoAsset>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    Stale,
    Missing,
    FutureTimestamp,
}

/// Event age, not socket connectivity, determines freshness.
pub fn freshness(event_ms: Option<u64>, now_ms: u64, max_age_ms: u64) -> Freshness {
    match event_ms {
        None => Freshness::Missing,
        Some(t) if t > now_ms => Freshness::FutureTimestamp,
        Some(t) if now_ms - t > max_age_ms => Freshness::Stale,
        Some(_) => Freshness::Fresh,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum InferredSide {
    Buy,
    Sell,
    Unknown,
}

/// First parity function ported from option_flow_engine.py.
/// This is an inference from a valid quote, never an exchange-provided trade side.
/// Caller must separately check quote age and reject future/out-of-order quotes.
pub fn infer_side(price: f64, bid: f64, ask: f64) -> InferredSide {
    if !price.is_finite()
        || !bid.is_finite()
        || !ask.is_finite()
        || price <= 0.0
        || bid <= 0.0
        || ask < bid
    {
        return InferredSide::Unknown;
    }
    let tolerance = 0.01_f64.max((ask - bid) * 0.05);
    if price >= ask - tolerance {
        InferredSide::Buy
    } else if price <= bid + tolerance {
        InferredSide::Sell
    } else if price > (bid + ask) / 2.0 {
        InferredSide::Buy
    } else if price < (bid + ask) / 2.0 {
        InferredSide::Sell
    } else {
        InferredSide::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_missing_and_future_data_are_distinct() {
        assert_eq!(freshness(None, 200_000, 120_000), Freshness::Missing);
        assert_eq!(freshness(Some(79_999), 200_000, 120_000), Freshness::Stale);
        assert_eq!(freshness(Some(80_000), 200_000, 120_000), Freshness::Fresh);
        assert_eq!(
            freshness(Some(200_001), 200_000, 120_000),
            Freshness::FutureTimestamp
        );
    }

    #[test]
    fn classification_matches_legacy_boundaries() {
        assert_eq!(infer_side(2.20, 2.0, 2.2), InferredSide::Buy);
        assert_eq!(infer_side(2.0, 2.0, 2.2), InferredSide::Sell);
        assert_eq!(infer_side(2.1, 2.0, 2.2), InferredSide::Unknown);
        assert_eq!(infer_side(2.05, 2.0, 2.2), InferredSide::Sell);
        assert_eq!(infer_side(2.15, 2.0, 2.2), InferredSide::Buy);
        assert_eq!(infer_side(2.0, 2.2, 2.0), InferredSide::Unknown);
        assert_eq!(infer_side(f64::NAN, 2.0, 2.2), InferredSide::Unknown);
    }

    #[test]
    fn shared_demo_fixture_is_explicit_and_has_valid_ohlc() {
        let snapshot: DemoSnapshot =
            serde_json::from_str(include_str!("../../../fixtures/demo-market.json")).unwrap();
        assert_eq!(snapshot.mode, "demo");
        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.assets[0].symbol, "QQQ");
        for asset in snapshot.assets {
            assert!(!asset.candles.is_empty());
            for candle in asset.candles {
                assert!(candle.high >= candle.open.max(candle.close));
                assert!(candle.low <= candle.open.min(candle.close));
            }
        }
    }
}

pub mod analyst_compact;
pub mod historical;
pub mod legacy_position;
pub mod local_volume;

pub mod investment;

pub mod investment_structure;

pub mod investment_zones;

pub mod greek_support;
pub mod investment_risk;
pub mod theta_vega;

pub mod opportunity;
pub mod gap_engine;
pub mod level_research;
pub mod arrival_baseline;
pub mod level_dataset;
