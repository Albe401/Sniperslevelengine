use serde::{Deserialize, Serialize};
use snipers_core::block_alerts::{Config, Engine, Scope, StoredAlert};
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    enabled: bool,
    #[serde(default)]
    armed_date: String,
    #[serde(default)]
    armed_ms: u64,
    config: Option<Config>,
    watchlist: Vec<String>,
    engine: Engine,
    scanned: usize,
    total: usize,
    last_symbol: String,
    last_scan_ms: u64,
    error: String,
}

#[derive(Default)]
pub struct Task {
    inner: Arc<Mutex<Saved>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    enabled: bool,
    config: Option<Config>,
    scanned: usize,
    total: usize,
    last_symbol: String,
    last_scan_ms: u64,
    error: String,
    unread: usize,
    history: Vec<StoredAlert>,
    morning_window: bool,
}

fn morning_window(now: u64) -> bool {
    let Some(date) = snipers_core::calendar::date_at(now) else { return false };
    let begin = snipers_core::calendar::legacy_eastern_ms(&format!("{date} 04:00:00"));
    let end = snipers_core::calendar::legacy_eastern_ms(&format!("{date} 16:00:00"));
    begin.zip(end).is_some_and(|(begin, end)| now >= begin && now < end)
}

fn path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app.path().app_local_data_dir().map_err(|_| "Carpeta local no disponible")?.join("block-alerts.json"))
}

fn save(app: &tauri::AppHandle, value: &Saved) -> Result<(), String> {
    let file = path(app)?;
    std::fs::create_dir_all(file.parent().ok_or("Ruta local inválida")?)
        .map_err(|_| "No se pudo preparar el historial de alarmas")?;
    let bytes = serde_json::to_vec(value).map_err(|_| "No se pudieron serializar las alarmas")?;
    if bytes.len() > 10_000_000 { return Err("Historial de alarmas demasiado grande".into()) }
    std::fs::write(file, bytes).map_err(|_| "No se pudieron guardar las alarmas".into())
}

fn read(app: &tauri::AppHandle) -> Saved {
    path(app).ok().and_then(|p| std::fs::read(p).ok())
        .and_then(|bytes| serde_json::from_slice::<Saved>(&bytes).ok())
        .unwrap_or_default()
}

fn status(saved: &Saved) -> Status {
    let today = snipers_core::calendar::date_at(massive_feed::live::now_ms()).map(|date| date.to_string());
    Status {
        enabled: saved.enabled && today.as_deref() == Some(saved.armed_date.as_str()),
        config: saved.config.clone(),
        scanned: saved.scanned,
        total: saved.total,
        last_symbol: saved.last_symbol.clone(),
        last_scan_ms: saved.last_scan_ms,
        error: saved.error.clone(),
        unread: saved.engine.unread_count(),
        history: saved.engine.history().iter().rev().take(500).cloned().collect(),
        morning_window: morning_window(massive_feed::live::now_ms()),
    }
}

pub fn start(app: tauri::AppHandle) {
    let task = app.state::<Task>().inner.clone();
    if let Ok(mut guard) = task.lock() { *guard = read(&app); }
    tauri::async_runtime::spawn(async move {
        loop {
            let now = massive_feed::live::now_ms();
            let next = {
                let Ok(mut guard) = task.lock() else { break };
                if !guard.enabled || guard.armed_date != snipers_core::calendar::date_at(now).map(|date| date.to_string()).unwrap_or_default() || !morning_window(now) { None }
                else if let Some(config) = guard.config.clone() {
                    let watchlist = guard.watchlist.clone();
                    match guard.engine.next_batch(&config, &watchlist) {
                        Ok(batch) => {
                            guard.total = match &config.scope { Scope::Selected(v) => v.len(), Scope::Watchlist => watchlist.len() };
                            Some((config, watchlist, batch))
                        }
                        Err(error) => { guard.error = error; None }
                    }
                } else { None }
            };
            if let Some((config, watchlist, batch)) = next {
                for symbol in batch {
                    if !task.lock().is_ok_and(|g| g.enabled && g.armed_date == snipers_core::calendar::date_at(massive_feed::live::now_ms()).map(|date| date.to_string()).unwrap_or_default() && morning_window(massive_feed::live::now_ms())) { break }
                    let result = massive_feed::investment::daily_purchases(&symbol, config.minimum_notional).await;
                    let at = massive_feed::live::now_ms();
                    if let Ok(mut guard) = task.lock() {
                        if guard.enabled {
                            match result {
                                Ok(mut report) => {
                                    if let Some(problem) = report.coverage.iter().find(|line| line.contains(": error ")) {
                                        guard.error = problem.clone();
                                    } else {
                                        report.rows.retain(|row| row.block.execution_ms.is_some_and(|time| time >= guard.armed_ms));
                                        match massive_feed::block_alerts::observe_report(&mut guard.engine, &config, &watchlist, &report, at) {
                                            Ok(_) => guard.error.clear(),
                                            Err(error) => guard.error = error,
                                        }
                                    }
                                }
                                Err(error) => guard.error = format!("{symbol}: {error}"),
                            }
                            guard.scanned += 1;
                            if guard.scanned >= guard.total { guard.scanned = 0; }
                            guard.last_symbol = symbol;
                            guard.last_scan_ms = at;
                            if let Err(error) = save(&app, &guard) { guard.error = error; }
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
}

#[tauri::command]
pub fn get_block_alerts(task: tauri::State<'_, Task>) -> Result<Status, String> {
    let guard = task.inner.lock().map_err(|_| "Alarmas no disponibles")?;
    Ok(status(&guard))
}

#[tauri::command]
pub fn set_block_alerts(
    app: tauri::AppHandle,
    task: tauri::State<'_, Task>,
    enabled: bool,
    config: Config,
    watchlist: Vec<String>,
) -> Result<Status, String> {
    if enabled && !morning_window(massive_feed::live::now_ms()) {
        return Err("La búsqueda matutina se activa de 04:00 a 16:00 ET".into());
    }
    Engine::validate(&config, &watchlist)?;
    let mut guard = task.inner.lock().map_err(|_| "Alarmas no disponibles")?;
    let before = guard.clone();
    guard.enabled = enabled;
    guard.armed_date = if enabled { snipers_core::calendar::date_at(massive_feed::live::now_ms()).ok_or("Fecha ET no disponible")?.to_string() } else { String::new() };
    guard.armed_ms = if enabled { massive_feed::live::now_ms() } else { 0 };
    guard.config = Some(config);
    guard.watchlist = watchlist;
    guard.scanned = 0;
    guard.total = 0;
    guard.error.clear();
    if let Err(error) = save(&app, &guard) { *guard = before; return Err(error) }
    Ok(status(&guard))
}

#[tauri::command]
pub fn mark_block_alerts_read(
    app: tauri::AppHandle,
    task: tauri::State<'_, Task>,
    key: Option<String>,
) -> Result<Status, String> {
    let mut guard = task.inner.lock().map_err(|_| "Alarmas no disponibles")?;
    let before = guard.clone();
    if let Some(key) = key { guard.engine.mark_read(&key); }
    else { guard.engine.mark_all_read(); }
    if let Err(error) = save(&app, &guard) { *guard = before; return Err(error) }
    Ok(status(&guard))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn morning_bounds_are_eastern_and_stop_at_four_pm() {
        let t = |hour| snipers_core::calendar::legacy_eastern_ms(&format!("2026-10-08 {hour}:00:00")).unwrap();
        assert!(!morning_window(t("03")));
        assert!(morning_window(t("04")));
        assert!(morning_window(t("15")));
        assert!(!morning_window(t("16")));
    }
}
