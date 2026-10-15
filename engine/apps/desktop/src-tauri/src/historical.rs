use massive_feed::{
    credentials::Credentials,
    historical_search::{self, Report},
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use tauri::Manager;
#[derive(Default)]
pub struct Task {
    epoch: Arc<AtomicU64>,
    handle: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}
#[tauri::command]
pub async fn start_historical_search(
    date: String,
    include_rvol: bool,
    on_message: tauri::ipc::Channel<Report>,
    app: tauri::AppHandle,
    task: tauri::State<'_, Task>,
) -> Result<(), String> {
    snipers_core::historical::session(&date, massive_feed::live::now_ms())
        .map_err(str::to_owned)?;
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta de reportes no disponible")?
        .join("historical-reports");
    let key = Credentials::load().map_err(str::to_owned)?;
    let epoch = task.epoch.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
    let old = task
        .handle
        .lock()
        .map_err(|_| "Búsqueda no disponible")?
        .take();
    if let Some(old) = old {
        old.abort();
        let _ = old.await;
    }
    let mut handle = task.handle.lock().map_err(|_| "Búsqueda no disponible")?;
    if task.epoch.load(Ordering::SeqCst) != epoch {
        return Err("Consulta cancelada".into());
    }
    let generation = task.epoch.clone();
    *handle = Some(tauri::async_runtime::spawn(async move {
        let result = historical_search::run(&key, &date, include_rvol, |report| {
            generation.load(Ordering::SeqCst) == epoch && on_message.send(report.clone()).is_ok()
        })
        .await;
        let mut report = match result {
            Ok(report) => report,
            Err(e) => Report {
                schema: 1,
                date,
                phase: "error".into(),
                total: 0,
                processed: 0,
                rows: vec![],
                top: vec![],
                generated_ms: massive_feed::live::now_ms(),
                error: Some(e.into()),
                saved_path: None,
                entry_confirmed: false,
            },
        };
        if generation.load(Ordering::SeqCst) != epoch {
            return;
        }
        let copy = report.clone();
        match tauri::async_runtime::spawn_blocking(move || {
            historical_search::save(&directory, &copy)
        })
        .await
        {
            Ok(Ok(path)) => report.saved_path = Some(path.to_string_lossy().into_owned()),
            _ => {
                report.error = Some(format!(
                    "{} No se pudo guardar el reporte local.",
                    report.error.as_deref().unwrap_or("")
                ))
            }
        }
        if generation.load(Ordering::SeqCst) == epoch {
            let _ = on_message.send(report);
        }
    }));
    Ok(())
}
#[tauri::command]
pub async fn stop_historical_search(task: tauri::State<'_, Task>) -> Result<(), String> {
    task.epoch.fetch_add(1, Ordering::SeqCst);
    let old = task
        .handle
        .lock()
        .map_err(|_| "Búsqueda no disponible")?
        .take();
    if let Some(old) = old {
        old.abort();
        let _ = old.await;
    }
    Ok(())
}
