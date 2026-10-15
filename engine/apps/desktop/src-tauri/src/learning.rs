//! Passive collection: no broker access and no changes to model parameters.
use massive_feed::{analyst_context, live};
use serde_json::json;
use snipers_store::learning::{LearningStore, Observation};
use std::path::PathBuf;

pub fn dashboard(
    directory: &std::path::Path,
    connected: bool,
    symbol: Option<&str>,
    now: u64,
) -> Result<serde_json::Value, String> {
    let mut data = snipers_store::learning_report::read(
        &directory.join("snipers-learning-v1.sqlite"),
        symbol,
        now,
    )
    .map_err(|_| "No se pudo leer el historial de aprendizaje. Intenta actualizar.".to_owned())?;
    let read = |name: &str| -> Option<serde_json::Value> {
        let path = directory.join(name);
        if std::fs::metadata(&path).ok()?.len() > 524_288 {
            return None;
        }
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()
    };
    data["generatedMs"] = now.into();
    data["connected"] = connected.into();
    data["collector"] = read("learning-status.json").unwrap_or(serde_json::Value::Null);
    data["collectorV2"] = read("learning-v2-status.json").unwrap_or(serde_json::Value::Null);
    // Only publish bounded metadata to this panel, not the full model context.
    data["bridge"]=read("analysis-bridge.json").map(|v|json!({"capturedMs":v["captured_ms"],"symbol":v["symbol"],
        "quality":v["quality"],"hasContext":v["context"]["analyst"].is_object(),"stocksPhase":v["context"]["stocks_phase"],
        "optionsPhase":v["context"]["options_phase"]})).unwrap_or(serde_json::Value::Null);
    data["gammaMap"] = snipers_store::learning_report::read(&directory.join("gamma-map-learning-v1.sqlite"), symbol, now).map_err(|_| "No se pudo leer aprendizaje gamma".to_owned())?;
    data["gammaCollector"] = read("gamma-learning-status.json" ).unwrap_or(serde_json::Value::Null);
    data["bot"] = snipers_store::bot_learning::read(&directory.join("alpaca-paper.sqlite"),symbol.as_deref(),now)
        .unwrap_or_else(|error|json!({"available":false,"error":error}));
    data["qqqComponents"]=massive_feed::qqq_component_reading::journal_summary(&directory.join("qqq-components-v2.jsonl"));
    Ok(data)
}

pub(crate) fn observation(s:&live::LiveState,now:u64)->Observation {
 serde_json::from_value(massive_feed::learning_observation::observation_value(s,now,env!("SNIPERS_ENGINE_BUILD"))).expect("Shared observation schema")
}

pub(crate) fn write_json(
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
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let o = observation(&snapshot, live::now_ms());
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_replaces_atomically_and_worker_persists_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        write_json(dir.path(), "replace.json", &json!({"v":1})).unwrap();
        write_json(dir.path(), "replace.json", &json!({"v":2})).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("replace.json")).unwrap())
                .unwrap();
        assert_eq!(value["v"], 2);
        let state = live::new_state();
        let frame = live::snapshot(&state);
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tx.try_send(frame).unwrap();
        drop(tx);
        tauri::async_runtime::block_on(run(dir.path().to_owned(), rx));
        assert!(dir.path().join("snipers-learning-v1.sqlite").exists());
        let status: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.path().join("learning-status.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(status["recorded"], true);
        assert_eq!(status["eligible"], false);
        let bridge: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.path().join("analysis-bridge.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(bridge["read_only"], true);
        assert!(bridge["context"]["analyst"].is_null());
        assert_eq!(bridge["context"]["fvg"]["version"], "fvg-v1");
        assert_eq!(bridge["context"]["fvg"]["available"], false);
    }
    #[test]
    fn disconnected_context_is_diagnostic_only() {
        let state = live::new_state();
        let s = live::snapshot(&state);
        let o = observation(&s, live::now_ms());
        assert!(!o.eligible);
        assert!(o.price.is_none());
        assert!(o.context["analyst"].is_null());
        assert_eq!(o.context["automatic_model_updates"], false);
    }
}

