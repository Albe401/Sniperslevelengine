//! Massive protocol foundation. Transport is opt-in via the diagnostic example.
//! Production reconnection, replay recovery and desktop live mode are separate work.
use serde::{Deserialize, Serialize};
pub mod alpaca;
pub mod analyst_context;
pub mod investment_fmp;
pub mod analyst_targets;
pub mod neuro_context;
pub mod broker_quotes;
pub mod bot_ema;
pub mod bot_ema9_rebound;
pub mod chart_history;
pub mod daily_atr;
pub mod gap_engine;
pub mod clock_diagnostic;
pub mod cme;
pub mod cme_bridge;
pub mod cme_cache;
pub mod cme_data;
pub mod cme_oi;
pub mod cme_reference;
pub mod cme_runtime;
pub mod credentials;
pub mod news_translation;
pub mod etf_scanner;
pub mod treasury;
pub mod qqq_components;
pub mod overnight_data;
pub mod qqq_component_reading;
pub mod direction_context;
pub mod dex_store;
pub mod historical_search;
pub mod history;
pub mod interpretation;
pub mod intraday_strategy;
pub mod swing_context;
pub mod swing_observer;
pub mod route_4h;
pub mod route_4h_history;
pub mod route_4h_projection;
pub mod bot_route_context;
pub mod bot_chart_route;
pub mod quick_bot_context;
pub mod fvg;
pub mod level_dynamics;
pub mod live;
pub mod auction_live;
pub mod auction_profile_partitions;
pub mod trade_units;
pub mod auction_session;
pub mod auction_reconciliation;
pub mod manual_scanner;
pub mod metrics;
pub mod movement_room;
pub mod off_exchange;
pub mod oi_store;
pub mod option_quote_diagnostic;
pub mod option_print_scanner;
pub mod option_activity_history;
pub mod option_activity_dataset;
pub mod option_activity_observer;
pub mod option_premium_dataset;
pub mod option_premium_audit;
pub mod option_volume_audit;
pub mod option_joint_audit;
pub mod option_future_outcomes;
pub mod option_outcome_prices;
pub mod option_formula_baseline;
pub mod option_trial_units;
pub mod option_capture_preflight;
pub mod option_trade_identity_probe;
pub mod option_premium_reference_accumulator;
pub mod block_alerts;
pub mod option_time_queue;
pub mod option_snapshot;
pub mod paper_expiry;
pub mod paper_readiness;
pub mod range_context;
pub mod recording;
pub mod comparison;
pub mod background_capture;
pub mod snapshot_lanes;
pub mod snapshot_bridge;
pub mod service_ipc;
pub mod learning_observation;
pub mod passive_learning_v2;
pub mod scanner;
pub mod scanner_options;
pub mod spx_reading;
pub mod spx_synthetic;
pub mod structural_path;
pub mod telemetry;
mod top_five_metrics;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Market {
    Stocks,
    Options,
}

#[derive(Debug, Clone, Copy)]
pub enum Feed {
    Realtime,
    Delayed,
}

pub fn endpoint(market: Market, feed: Feed) -> &'static str {
    match (market, feed) {
        (Market::Stocks, Feed::Realtime) => "wss://socket.massive.com/stocks",
        (Market::Stocks, Feed::Delayed) => "wss://delayed.massive.com/stocks",
        (Market::Options, Feed::Realtime) => "wss://socket.massive.com/options",
        (Market::Options, Feed::Delayed) => "wss://delayed.massive.com/options",
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "ev")]
pub enum Event {
    #[serde(rename = "status")]
    Status {
        status: String,
        #[serde(default)]
        message: String,
    },
    #[serde(rename = "T")]
    Trade {
        sym: String,
        p: f64,
        s: f64,
        t: u64,
        #[serde(default)]
        q: Option<u64>,
        #[serde(flatten)]
        metadata: TradeMetadata,
    },
    #[serde(rename = "Q")]
    Quote {
        sym: String,
        bp: f64,
        ap: f64,
        bs: f64,
        #[serde(rename = "as")]
        ask_size: f64,
        t: u64,
    },
    #[serde(rename = "A")]
    SecondBar {
        sym: String,
        o: f64,
        h: f64,
        l: f64,
        c: f64,
        v: f64,
        s: u64,
        e: u64,
        #[serde(default)]
        vw: Option<f64>,
    },
    #[serde(rename = "AM")]
    MinuteBar {
        sym: String,
        o: f64,
        h: f64,
        l: f64,
        c: f64,
        v: f64,
        s: u64,
        e: u64,
        #[serde(default)]
        vw: Option<f64>,
    },
    #[serde(other)]
    Unknown,
}

/// Exchange/condition fields retained for replay and the subsequent flow/TRF audit.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct TradeMetadata {
    #[serde(default)]
    pub i: Option<String>,
    #[serde(default)]
    pub x: Option<u32>,
    #[serde(default)]
    pub z: Option<u32>,
    #[serde(default)]
    pub c: Vec<u32>,
    #[serde(default)]
    pub ds: Option<String>,
    #[serde(default)]
    pub pt: Option<u64>,
    #[serde(default)]
    pub trfi: Option<u32>,
    #[serde(default)]
    pub trft: Option<u64>,
}

/// Massive may send several events in a single JSON array.
pub fn decode_batch(payload: &str) -> Result<Vec<Event>, serde_json::Error> {
    serde_json::from_str(payload)
}

#[derive(Debug, thiserror::Error)]
pub enum SubscriptionError {
    #[error("Select at least one explicit channel; wildcards are disabled in this foundation")]
    Invalid,
    #[error("Options quotes allow at most 1,000 distinct contracts per connection")]
    QuoteLimit,
}

pub fn subscription(market: Market, channels: &[String]) -> Result<String, SubscriptionError> {
    if channels.is_empty() {
        return Err(SubscriptionError::Invalid);
    }
    let mut unique = std::collections::BTreeSet::new();
    let mut quotes = std::collections::BTreeSet::new();
    for channel in channels {
        let (kind, symbol) = channel.split_once('.').ok_or(SubscriptionError::Invalid)?;
        if !["T", "Q", "A", "AM"].contains(&kind)
            || symbol.is_empty()
            || !symbol
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b":.-".contains(&c))
        {
            return Err(SubscriptionError::Invalid);
        }
        match market {
            Market::Options if !symbol.starts_with("O:") || symbol.len() <= 2 => {
                return Err(SubscriptionError::Invalid)
            }
            Market::Stocks if symbol.contains(':') => return Err(SubscriptionError::Invalid),
            _ => {}
        }
        if kind == "Q" {
            quotes.insert(symbol);
        }
        unique.insert(channel.as_str());
    }
    if matches!(market, Market::Options) && quotes.len() > 1000 {
        return Err(SubscriptionError::QuoteLimit);
    }
    Ok(serde_json::json!({"action": "subscribe", "params": unique.into_iter().collect::<Vec<_>>().join(",")}).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batches_keep_millisecond_timestamps_and_ignore_new_event_types() {
        let events = decode_batch(r#"[{"ev":"status","status":"auth_success"},{"ev":"T","sym":"QQQ","p":500.1,"s":10,"t":1780000000123},{"ev":"NEW"}]"#).unwrap();
        assert!(matches!(
            &events[1],
            Event::Trade {
                t: 1780000000123,
                ..
            }
        ));
        assert!(matches!(&events[2], Event::Unknown));
        assert!(decode_batch("invalid").is_err());
    }
    #[test]
    fn market_mismatch_and_wildcards_are_rejected() {
        assert!(subscription(Market::Options, &["Q.QQQ".into()]).is_err());
        assert!(subscription(Market::Stocks, &["T.O:QQQ".into()]).is_err());
        assert!(subscription(Market::Stocks, &["T.*".into()]).is_err());
        assert!(subscription(Market::Stocks, &["A.QQQ".into()]).is_ok());
    }
    #[test]
    fn enforces_quote_contract_limit() {
        let channels: Vec<_> = (0..1001).map(|i| format!("Q.O:QQQ{i}")).collect();
        assert!(matches!(
            subscription(Market::Options, &channels),
            Err(SubscriptionError::QuoteLimit)
        ));
        assert!(subscription(Market::Options, &channels[..1000]).is_ok());
    }
}

pub mod investment;

pub mod bot_universe;

pub mod price_alerts;

pub mod gap_bot_context;
pub mod smc;
pub mod smc_bot;
pub mod smc_observer;
pub mod smc_study;
mod smc_patterns;

pub mod auction_rejection_live;
pub mod news_intelligence;
