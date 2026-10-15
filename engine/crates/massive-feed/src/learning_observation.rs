use snipers_store::learning::LearningStore;
use std::path::PathBuf;
use crate::{analyst_context,live};
use serde_json::json;
pub fn observation_value(s: &live::LiveState, now: u64, build: &str) -> serde_json::Value {
    let pattern_read=crate::intraday_strategy::read_paper(&crate::intraday_strategy::snapshot_input(s),now);
    let tick = s.ticks.get(&s.symbol).filter(|t| t.symbol == s.symbol);
    let fresh = s.stocks.phase == "streaming"
        && tick.is_some_and(|t| {
            t.trade_ms.is_some_and(|ms| ms <= now.saturating_add(1000) && now.saturating_sub(ms) <= 15_000)
                && t.price.is_some_and(|p| p.is_finite() && p > 0.0)
        });
    // Normalize only the collector's copy; diagnostics keep the original provider time.
    let mut normalized = s.clone();
    if fresh {
        if let Some(t) = normalized.ticks.get_mut(&s.symbol) {
            t.trade_ms = t.trade_ms.map(|ms| ms.min(now));
        }
    }
    let prepared_result = analyst_context::prepare(&normalized, now);
    let preparation_error = prepared_result.as_ref().err().map(|e| format!("{e:?}"));
    let prepared = prepared_result.ok();
    let day = s
        .option_snapshot_status
        .horizons
        .iter()
        .find(|h| h.mode == "DAY");
    let structure_ready = day.is_some_and(|h| {
        h.structure.complete
            && h.structure.reading.available
            && h.observed_ms <= now
            && now - h.observed_ms <= 300_000
    });
    let in_rth = snipers_core::calendar::date_at(now).is_some_and(|d|
        matches!(snipers_core::calendar::schedule(d),snipers_core::calendar::Day::Open(s) if now>=s.open_ms && now<s.close_ms));
    let context_ready = prepared.as_ref().is_some_and(|p| {
        p["quick_read"]["stale_data"] == false
            && p["market_interpretation"]["day_or_next_session"]["call_score"].is_number()
            && p["market_interpretation"]["day_or_next_session"]["put_score"].is_number()
    });
    let eligible = in_rth && fresh && context_ready && structure_ready;
    let quality = if !in_rth {
        "OUTSIDE_REGULAR_SESSION"
    } else if !fresh {
        "NO_FRESH_PRICE"
    } else if !structure_ready {
        "INCOMPLETE_INTRADAY_STRUCTURE"
    } else if !context_ready {
        "CONTEXT_UNAVAILABLE"
    } else {
        "READY"
    };
    json!({
        "symbol": s.symbol.clone(),
        "observed_ms": now,
        "price": if fresh {
            tick.and_then(|t| t.price)
        } else {
            None
        },
        "price_ms": if fresh {
            tick.and_then(|t| t.trade_ms).map(|ms| ms.min(now))
        } else {
            None
        },
        "eligible":eligible,
        "quality": quality,
        "context": json!({"schema":"snipers-observation-v1","engine_version":env!("CARGO_PKG_VERSION"),
            "engine_build":build,
            "collector_version":"passive-v1","captured_ms":now,"analyst":prepared,
            "level_dynamics":s.level_dynamics,
            "magnet_reclaim":pattern_read.magnet_reclaim,"paper_entry_blockers":pattern_read.blockers,
            "structural_entry":pattern_read.structural_entry,"entry_basis":pattern_read.entry_basis,"entry_side":pattern_read.entry_side,
            "fvg":pattern_read.quick_context.get("fvg").cloned().unwrap_or_else(||crate::fvg::read(&crate::intraday_strategy::snapshot_input(s),&s.symbol,tick.and_then(|t|t.price).unwrap_or(0.),pattern_read.next,&pattern_read.entry_side,now)),
            "preparation_error":preparation_error,
            "price_diagnostics":{"tick_present":tick.is_some(),"source_ms":tick.and_then(|t|t.trade_ms),
                "price_present":tick.and_then(|t|t.price).is_some(),
                "age_ms":tick.and_then(|t|t.trade_ms).filter(|t|*t<=now).map(|t|now-t),
                "ahead_ms":tick.and_then(|t|t.trade_ms).filter(|t|*t>now).map(|t|t-now),
                "published_ms":s.published_ms},
            "stocks_phase":s.stocks.phase,"options_phase":s.options.phase,
            "snapshot_phase":s.option_snapshot_status.phase,
            "scope":"ACTIVE_SYMBOL_BACKEND_READING","automatic_model_updates":false}),
    })
}

pub fn write_json(
    directory: &std::path::Path,
    name: &str,
    value: &serde_json::Value,
) -> std::io::Result<()> {
    let temp = directory.join(format!("{name}.tmp"));
    std::fs::write(&temp, serde_json::to_vec(value)?)?;
    std::fs::rename(temp, directory.join(name))
}

pub async fn run(directory: PathBuf, mut frames: tokio::sync::mpsc::Receiver<live::LiveState>) {
    let mut next_record_ms = 0;
    while let Some(snapshot) = frames.recv().await {
        let now = live::now_ms();
        let record = now >= next_record_ms;
        if record {
            next_record_ms = now.saturating_add(30_000);
        }
        let directory = directory.clone();
        // SQLite and serialization never block the UI publisher or feed reader.
        let _ = tokio::task::spawn_blocking(move || {
            let o:snipers_store::learning::Observation=serde_json::from_value(observation_value(&snapshot,live::now_ms(),"independent-data-service")).expect("Observation schema");
            let bridge = json!({"captured_ms":o.observed_ms,"symbol":o.symbol,"quality":o.quality,
                "price_ms":o.price_ms,"context":o.context,"read_only":true});
            if write_json(&directory, "analysis-bridge.json", &bridge).is_err() {
                eprintln!("ANALYSIS_BRIDGE_UNAVAILABLE");
            }
            if !record {
                return;
            }
            let result = LearningStore::open(&directory.join("snipers-learning-v1.sqlite"))
                .and_then(|mut store| store.record(&o));
            let status = json!({"observed_ms":o.observed_ms,"symbol":o.symbol,
                "recorded":result.is_ok(),"eligible":o.eligible,"quality":o.quality,
                "error":if result.is_err(){Some("LEARNING_STORAGE_UNAVAILABLE")}else{None}});
            if result.is_err() {
                eprintln!("LEARNING_STORAGE_UNAVAILABLE");
            }
            let _ = write_json(&directory, "learning-status.json", &status);
        })
        .await;
    }
}

