//! Backend review adapter. No visible panel or changes to trade admission.
use massive_feed::route_4h_projection::{self, Sample};
use massive_feed::{live, route_4h};
use std::sync::{Mutex, OnceLock};
fn historical_samples(directory: Option<&std::path::Path>) -> &'static Mutex<Vec<Sample>> {
    static LIBRARY: OnceLock<Mutex<Vec<Sample>>> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let mut samples: Vec<Sample> = serde_json::from_str::<std::collections::BTreeMap<String,Vec<Sample>>>(include_str!("../../../../data/price-route-history/bot-projections.json"))
            .expect("validated own-symbol projections").into_values().flatten().collect();
        if let Some(directory) = directory {
            if let Ok(files) = std::fs::read_dir(directory) {
                for entry in files.flatten().take(500) {
                    let path = entry.path();
                    if !path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.ends_with("-study-v1.json"))
                    {
                        continue;
                    }
                    if !entry
                        .metadata()
                        .is_ok_and(|m| m.is_file() && m.len() < 16 * 1024 * 1024)
                    {
                        continue;
                    }
                    if let Ok(raw) = std::fs::read(path) {
                        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&raw) {
                            if v["version"] != route_4h::VERSION {
                                continue;
                            }
                            if let Ok(rows) = serde_json::from_value::<Vec<Sample>>(
                                v["projectionSamples"].clone(),
                            ) {
                                samples.extend(rows);
                            }
                        }
                    }
                }
            }
        }
        Mutex::new(samples)
    })
}
fn frozen_projections(
    s: &route_4h::Scenario,
    directory: Option<&std::path::Path>,
) -> [route_4h_projection::Projection; 2] {
    use std::collections::BTreeMap;
    static FROZEN: OnceLock<Mutex<BTreeMap<String, [route_4h_projection::Projection; 2]>>> =
        OnceLock::new();
    let mut frozen = FROZEN
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(p) = frozen.get(&s.id) {
        return p.clone();
    }
    let library = historical_samples(directory)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let p = [
        route_4h_projection::project(s, &library, "session"),
        route_4h_projection::project(s, &library, "swing2"),
    ];
    if frozen.len() >= 256 {
        frozen.clear();
    }
    frozen.insert(s.id.clone(), p.clone());
    p
}
#[tauri::command]
pub async fn preview_route_4h(
    app: tauri::AppHandle,
    task: tauri::State<'_, crate::LiveTask>,
    symbol: String,
) -> Result<serde_json::Value, String> {
    if snipers_core::universe::stock_asset(&symbol).is_none() {
        return Err("Activo no admitido".into());
    }
    let live = task
        .state
        .lock()
        .map_err(|_| "Estado no disponible")?
        .clone()
        .ok_or("Conecta el activo para revisar su contexto")?;
    let input = {
        let s = live.lock().map_err(|_| "Estado no disponible")?;
        if s.symbol != symbol {
            return Err("El activo seleccionado cambió".into());
        }
        s.minutes
            .values()
            .map(route_4h::Minute::from)
            .collect::<Vec<_>>()
    };
    use tauri::Manager;
    let directory = app
        .path()
        .app_local_data_dir()
        .ok()
        .map(|p| p.join("route-4h-history-v1"));
    tauri::async_runtime::spawn_blocking(move || {
        let frame = serde_json::to_value(live::snapshot(&live)).unwrap_or_default();
        // Snapshot construction can receive newer trades while acquiring the
        // shared lock. Validate against a clock captured after that snapshot,
        // otherwise an ordinary fresh trade appears to come from the future.
        let now = live::now_ms();
        let local = massive_feed::intraday_strategy::read(&frame, now);
        calculate_review(&symbol, &input, &frame, &local, directory.as_deref(), now)
    })
    .await
    .map_err(|_| "No se pudo calcular la estructura 4H".into())
}
fn calculate_review(
    symbol: &str,
    input: &[route_4h::Minute],
    frame: &serde_json::Value,
    local: &massive_feed::intraday_strategy::Reading,
    directory: Option<&std::path::Path>,
    now: u64,
) -> serde_json::Value {
    let mut reading = route_4h::read(symbol, input, now);
    let structural_target = reading.scenario.as_ref().map(|s| s.target);
    let projections = reading
        .scenario
        .as_ref()
        .map(|s| frozen_projections(s, directory));
    if let (Some(s), Some(ps)) = (reading.scenario.as_mut(), projections.as_ref()) {
        if let Some(target) = ps[1].typical {
            s.target = target;
            s.target_source = "historical-median".into();
        }
    }
    let evidence = massive_feed::swing_context::intraday_evidence(frame, local, now);
    let side = reading
        .scenario
        .as_ref()
        .map(|s| if s.side == 1 { "CALL" } else { "PUT" })
        .unwrap_or("PENDIENTE");
    let confirmation = massive_feed::swing_context::confirmation(side, symbol, &evidence, now);
    let observation = reading
        .scenario
        .as_ref()
        .map(|s| route_4h::monitor(s, input, now));
    let fresh = frame["ticks"][symbol]["tradeMs"]
        .as_u64()
        // Match the existing display-only price clock tolerance. The bot's
        // admission gate below remains strict and rejects future timestamps.
        .is_some_and(|t| t <= now.saturating_add(1000) && now.saturating_sub(t) <= 15000);
    serde_json::json!({"reading":reading,"projections":projections,"structuralTarget":structural_target,"confirmation":confirmation,"observation":observation,"mode":"review-only","available":frame["symbol"]==symbol&&frame["stocks"]["phase"]=="streaming"&&fresh,"executionEnabled":false})
}
/// Independent of the visible chart and its buttons. Reuses the same native 4H model.
pub fn bot_review(
    shared: &live::SharedState,
    frame: &serde_json::Value,
    local: &massive_feed::intraday_strategy::Reading,
    app: &tauri::AppHandle,
    now: u64,
) -> serde_json::Value {
    use std::collections::BTreeMap;
    use tauri::Manager;
    type Cache = BTreeMap<String, (u64, serde_json::Value)>;
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let symbol = &local.symbol;
    let valid = frame["symbol"] == *symbol
        && frame["stocks"]["phase"] == "streaming"
        && frame["publishedMs"]
            .as_u64()
            .is_some_and(|t| t <= now && now - t <= 3000)
        && frame["ticks"][symbol]["tradeMs"]
            .as_u64()
            .is_some_and(|t| t <= now && now - t <= 15000);
    if !valid {
        return serde_json::Value::Null;
    }
    let key = format!("{}:{}:{}", symbol, frame["startedMs"], local.session);
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some((at, review)) = cache
        .get(&key)
        .filter(|(at, _)| *at <= now && now - *at < 10000)
    {
        let _ = at;
        return review.clone();
    }
    let input = {
        let s = shared.lock().unwrap_or_else(|e| e.into_inner());
        if s.symbol == *symbol {
            s.minutes.values().map(route_4h::Minute::from).collect()
        } else {
            frame["chartHistory"]["minutes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| serde_json::from_value::<route_4h::Minute>(v.clone()).ok())
                .collect::<Vec<_>>()
        }
    };
    let directory = app
        .path()
        .app_local_data_dir()
        .ok()
        .map(|p| p.join("route-4h-history-v1"));
    let review = calculate_review(symbol, &input, frame, local, directory.as_deref(), now);
    if cache.len() >= 32 {
        cache.clear();
    }
    cache.insert(key, (now, review.clone()));
    review
}
#[tauri::command]
pub async fn study_route_4h(
    app: tauri::AppHandle,
    symbol: String,
    from: String,
    to: String,
) -> Result<route_4h::Study, String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use tauri::Manager;
    static BUSY: AtomicBool = AtomicBool::new(false);
    if BUSY.swap(true, Ordering::SeqCst) {
        return Err("Estudio histórico ya en curso".into());
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::SeqCst);
        }
    }
    let _guard = Guard;
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta no disponible")?
        .join("route-4h-history-v1");
    let report = massive_feed::route_4h_history::study(&symbol, &from, &to, &dir).await?;
    historical_samples(Some(&dir))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .extend(report.projection_samples.clone());
    let path = dir.join(format!("{symbol}-{from}-{to}-study-v1.json"));
    let bytes = serde_json::to_vec_pretty(&report).map_err(|_| "No se pudo serializar estudio")?;
    tauri::async_runtime::spawn_blocking(move || std::fs::write(path, bytes))
        .await
        .map_err(|_| "No se pudo guardar estudio")?
        .map_err(|_| "No se pudo guardar estudio")?;
    Ok(report)
}

#[cfg(test)]
mod preview_tests {
    use super::*;
    #[test]
    fn display_clock_lead_is_bounded_and_never_fabricates_a_setup() {
        let now = 1_791_302_400_000u64;
        for (stamp, expected) in [(now, true), (now + 351, true), (now + 1000, true), (now + 1001, false), (now - 15001, false)] {
            let frame = serde_json::json!({"symbol":"QQQ","stocks":{"phase":"streaming"},"ticks":{"QQQ":{"symbol":"QQQ","price":762.,"tradeMs":stamp}}});
            let local = massive_feed::intraday_strategy::read(&frame, now);
            let result = calculate_review("QQQ", &[], &frame, &local, None, now);
            assert_eq!(result["available"], expected);
            assert!(result["reading"]["scenario"].is_null());
            assert_eq!(result["executionEnabled"], false);
        }
    }
}
