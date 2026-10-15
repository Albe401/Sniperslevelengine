#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use massive_feed::{credentials::Credentials, live};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::Manager;
mod alpaca;
mod bot_position_policy;
mod bot_thesis;
mod decision_review;
mod historical;
mod investment;
mod news_coordinator;
mod company_research;
mod option_print_scanner;
mod option_research;
mod bot_option_activity;
mod bot_pullback_learning;
mod bot_asset_history;
mod block_alerts;
mod analyst_targets;
mod learning;
mod learning_v2;
mod data_connection;
mod iv_history;
mod structure_study;
mod gamma_learning;
mod legacy_archive;
mod manual_scanner;
mod route_4h;
mod etf_scanner;
mod paper_bot;
mod neuro_trader;
mod neuro_learning;
use legacy_archive::{get_legacy_journal, import_legacy_database, preview_legacy_database};
mod selection_storage;
mod research_observations;
use historical::{start_historical_search, stop_historical_search};

#[tauri::command]
async fn get_snapshot_status()->Result<serde_json::Value,String>{massive_feed::snapshot_bridge::request("status",None).await}
#[tauri::command]
async fn get_snapshot_bundle()->Result<serde_json::Value,String>{massive_feed::snapshot_bridge::request("latest",None).await}
#[tauri::command]
async fn record_snapshot_section(command:String,args:Option<serde_json::Value>,value:serde_json::Value)->Result<(),String>{
    massive_feed::snapshot_bridge::request("observe",Some(serde_json::json!({"command":command,"args":args.unwrap_or(serde_json::json!({})),"value":value}))).await.map(|_|())
}

#[tauri::command]
async fn get_learning_dashboard(
    app: tauri::AppHandle,
    task: tauri::State<'_, LiveTask>,
    symbol: Option<String>,
) -> Result<serde_json::Value, String> {
    let connected = task
        .handle
        .lock()
        .map_err(|_| "Estado no disponible")?
        .is_some();
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta local no disponible")?;
    tauri::async_runtime::spawn_blocking(move || {
        learning::dashboard(&directory, connected, symbol.as_deref(), live::now_ms())
    })
    .await
    .map_err(|_| "No se pudo consultar el aprendizaje".to_owned())?
}

#[tauri::command]
async fn get_system_clock_diagnostic() -> Result<serde_json::Value, String> {
    static BUSY: AtomicBool = AtomicBool::new(false);
    if BUSY.swap(true, Ordering::SeqCst) {
        return Err("Ya hay un diagnóstico de reloj en curso".into());
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::SeqCst);
        }
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = Guard;
        let report = massive_feed::clock_diagnostic::inspect()?;
        serde_json::to_value(report).map_err(|_| "No se pudo presentar el diagnóstico".to_owned())
    })
    .await
    .map_err(|_| "No se pudo completar el diagnóstico".to_owned())?
}

#[tauri::command]
async fn get_paper_journal(
    app: tauri::AppHandle,
    before_id: Option<i64>,
) -> Result<serde_json::Value, String> {
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta local no disponible")?;
    Ok(tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&directory).map_err(|_| "No se pudo abrir la carpeta local")?;
        let ledger = snipers_store::Ledger::open(&directory.join("snipers-v1.sqlite"))
            .map_err(|_| "No se pudo abrir el diario local")?;
        ledger
            .journal_report(before_id, 25)
            .map_err(|_| "No se pudo recuperar el diario completo; vuelve a intentarlo")
    })
    .await
    .map_err(|_| "No se pudo consultar el diario")??)
}
#[tauri::command]
async fn record_gamma_learning(app:tauri::AppHandle, task:tauri::State<'_,LiveTask>,payload:serde_json::Value)->Result<(),String>{
 if task.remote.load(Ordering::SeqCst){
  let directory=app.path().app_local_data_dir().map_err(|_|"Carpeta no disponible")?;
  return tauri::async_runtime::spawn_blocking(move||{
   let value=massive_feed::service_ipc::request(serde_json::json!({"method":"learning_observation","snapshotMs":payload["snapshotMs"]}))?;
   let comparable=value["gammaComparable"]==true;let o:snipers_store::learning::Observation=serde_json::from_value(value).map_err(|_|"Observación inválida")?;
   gamma_learning::record_observation(&directory,o,payload,comparable,live::now_ms())
  }).await.map_err(|_|"Registro gamma no disponible")?;
 }

 if task.handle.lock().map_err(|_|"Conexión no disponible")?.is_none(){return Err("Mercado desconectado".into());}
 let frame=task.published.lock().map_err(|_|"Estado no disponible")?.clone().ok_or("Esperando lectura publicada")?;
 let directory=app.path().app_local_data_dir().map_err(|_|"Carpeta no disponible")?;
 tauri::async_runtime::spawn_blocking(move||gamma_learning::record(&directory,&frame,payload,live::now_ms()))
  .await.map_err(|_|"Registro gamma no disponible".to_owned())?
}
#[derive(Default)]
struct LiveTask {
    remote: AtomicBool,
    remote_published: std::sync::Arc<Mutex<Option<serde_json::Value>>>,
    etf_scanner_enabled: AtomicBool,
    maintenance: std::sync::Arc<AtomicBool>,
    epoch: AtomicU64,
    handle: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    state: Mutex<Option<live::SharedState>>,
    // Passive consumers reuse the exact published reading without recalculating
    // the scanner or taking the market-event lock. Arc keeps reads inexpensive.
    published: std::sync::Arc<Mutex<Option<std::sync::Arc<live::LiveState>>>>,
}

#[tauri::command]
async fn get_analyst_context(
    task: tauri::State<'_, LiveTask>,
) -> Result<serde_json::Value, String> {
    if task.remote.load(Ordering::SeqCst){return tauri::async_runtime::spawn_blocking(||massive_feed::service_ipc::request(serde_json::json!({"method":"analyst"}))).await.map_err(|_|"Motor independiente no disponible")?;}

    let epoch = task.epoch.load(Ordering::SeqCst);
    if task
        .handle
        .lock()
        .map_err(|_| "Conexión no disponible")?
        .is_none()
    {
        return Err("Conecta el monitor para preparar el contexto local".into());
    }
    let shared = task
        .state
        .lock()
        .map_err(|_| "Estado no disponible")?
        .clone()
        .ok_or("Estado no disponible")?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let snapshot = live::snapshot(&shared);
        massive_feed::analyst_context::prepare(&snapshot, live::now_ms()).map_err(|error| {
            match error {
                snipers_core::analyst::Error::Stale => {
                    "El contexto requiere un precio y una conexión vigentes"
                }
                snipers_core::analyst::Error::Budget => {
                    "El contexto supera el límite; se conserva íntegro el control de decisiones"
                }
                _ => "No se pudo preparar un contexto verificable",
            }
            .to_owned()
        })
    })
    .await
    .map_err(|_| "No se pudo preparar el contexto local")?;
    if task.epoch.load(Ordering::SeqCst) != epoch {
        return Err("La sesión cambió durante la preparación".into());
    }
    result
}

#[tauri::command]
async fn start_live(
    symbol: String,
    connection_mode: Option<String>,
    entries_enabled: Option<bool>,
    price_alerts: Option<Vec<massive_feed::price_alerts::Alert>>,
    resume_legacy: Option<bool>,
    app: tauri::AppHandle,
    on_message: tauri::ipc::Channel<serde_json::Value>,
    task: tauri::State<'_, LiveTask>,
) -> Result<(), String> {
    if task.handle.lock().map_err(|_|"Estado ocupado")?.is_some(){return Err("Desconecta la vista anterior antes de cambiar de conexión".into());}
    let mode=connection_mode.as_deref().unwrap_or("auto");
    if !["auto","receiver","direct"].contains(&mode){return Err("Modo de conexión inválido".into());}
    if mode=="direct"{tauri::async_runtime::spawn_blocking(data_connection::stop_receiver).await.map_err(|_|"No se pudo transferir la conexión")??;}
    if mode=="receiver"{let selected=symbol.clone();tauri::async_runtime::spawn_blocking(move||data_connection::ensure_receiver(&selected)).await.map_err(|_|"No se pudo iniciar el receptor")??;}
    if mode!="direct" && massive_feed::service_ipc::present() {
        if entries_enabled==Some(true) {return Err("El receptor compartido es solo lectura; ejecución no habilitada".into());}
        let requested=symbol.clone();let etf=task.etf_scanner_enabled.load(Ordering::SeqCst);let shared_alerts=price_alerts.clone();
        tauri::async_runtime::spawn_blocking(move||{
            massive_feed::service_ipc::request(serde_json::json!({"method":"select","symbol":requested}))?;
            massive_feed::service_ipc::request(serde_json::json!({"method":"etf_enabled","enabled":etf}))?;
            massive_feed::service_ipc::request(serde_json::json!({"method":"direction_enabled","enabled":true}))?;
            if let Some(alerts)=shared_alerts{massive_feed::service_ipc::request(serde_json::json!({"method":"alerts","alerts":alerts}))?;}
            Ok::<(),String>(())
        }).await.map_err(|_|"Motor independiente no disponible")??;
        let mut running=task.handle.lock().map_err(|_|"Estado ocupado")?;
        if running.is_some(){return Err("Desconecta la vista anterior antes de conectar otra".into());}
        task.remote.store(true,Ordering::SeqCst);let published=task.remote_published.clone();let wanted=symbol.clone();
        *running=Some(tauri::async_runtime::spawn(async move {
            let mut timer=tokio::time::interval(std::time::Duration::from_secs(1));
            loop {timer.tick().await;let response=tauri::async_runtime::spawn_blocking(||massive_feed::service_ipc::request(serde_json::json!({"method":"snapshot"}))).await;
                match response {
                    Ok(Ok(frame)) if frame["symbol"]==wanted=>{*published.lock().unwrap()=Some(frame.clone());if on_message.send(frame).is_err(){break;}},
                    Ok(Ok(_))=>{},
                    _=>{let previous=published.lock().unwrap().clone();if let Some(mut frame)=previous{for market in ["stocks","options"]{frame[market]["phase"]=serde_json::json!("disconnected");frame[market]["detail"]=serde_json::json!("Motor independiente no disponible; no se abre otra conexión");}frame["cvd"]["available"]=serde_json::json!(false);if on_message.send(frame).is_err(){break;}}}
                }
            }
        }));return Ok(());
    }
    task.remote.store(false,Ordering::SeqCst);
    if task.maintenance.load(Ordering::SeqCst) {
        return Err("Espera a que termine la importación del diario".into());
    }
    let epoch = task.epoch.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
    let state = live::new_state_for(&symbol).map_err(str::to_owned)?;
    state.lock().map_err(|_|"Estado ocupado")?.direction_enabled=true;
    state.lock().map_err(|_|"Estado ocupado")?.etf_scanner_enabled=task.etf_scanner_enabled.load(Ordering::SeqCst);
    // Preserve background tapes and portfolio membership across monitor changes.
    if let Some(old)=task.state.lock().map_err(|_|"Estado ocupado")?.clone(){
        let previous=old.lock().map_err(|_|"Estado ocupado")?;
        let mut next=state.lock().map_err(|_|"Estado ocupado")?;
        next.scanner_store=previous.scanner_store.clone();next.scanner_options=previous.scanner_options.clone();
        next.snapshot_lanes=previous.snapshot_lanes.clone();
        next.price_alert_engine=previous.price_alert_engine.clone();next.price_alert_engine.reset();
        next.bot_ticks=previous.bot_ticks.clone();next.bot_held=previous.bot_held.clone();
        next.qqq_ui_enabled=previous.qqq_ui_enabled;next.qqq_bot_demand=previous.qqq_bot_demand;next.qqq_components=previous.qqq_components.clone();next.direction_enabled=previous.direction_enabled;next.treasury=previous.treasury.clone();
    }
    if let Some(alerts)=price_alerts {state.lock().map_err(|_|"Estado ocupado")?.price_alert_engine.configure(alerts)?;}
    state.lock().map_err(|_|"Estado ocupado")?.bot_enabled=paper_bot::background_enabled(&app);
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta local no disponible")?;
    let mut storage = selection_storage::SelectionStorage::open(directory.clone(), &state).await?;
    let entries_enabled = entries_enabled.unwrap_or(false);
    if entries_enabled && symbol != "QQQ" {
        return Err("Las nuevas entradas simuladas solo están habilitadas para QQQ".into());
    }
    let mut reviewer = decision_review::Reviewer::open(
        directory.clone(),
        entries_enabled,
        resume_legacy.unwrap_or(false),
    )
    .await?;
    let held_symbol = reviewer.held_symbol().await?;
    let legacy_symbols = reviewer.resume_legacy().await?;
    state
        .lock()
        .map_err(|_| "Estado no disponible")?
        .legacy_symbols = legacy_symbols;
    if let Some(held) = &held_symbol {
        // Reuse the same supported asset policy; never interpolate arbitrary DB
        // text into a subscription request.
        live::new_state_for(held).map_err(str::to_owned)?;
    }
    state
        .lock()
        .map_err(|_| "Estado de posición no disponible")?
        .held_symbol = held_symbol;
    let mut running = task
        .handle
        .lock()
        .map_err(|_| "Estado de conexión no disponible")?;
    if task.epoch.load(Ordering::SeqCst) != epoch {
        return Err("Inicio de conexión cancelado".into());
    }
    if running.is_some() {
        return Err("Desconecta la sesión anterior antes de iniciar otra".into());
    }
    if task.maintenance.load(Ordering::SeqCst) {
        return Err("Espera a que termine la importación del diario".into());
    }
    let key = Credentials::load().map_err(str::to_owned)?;
    state
        .lock()
        .map_err(|_| "Estado no disponible")?
        .oi_directory = Some(
        app.path()
            .app_local_data_dir()
            .map_err(|_| "Carpeta local no disponible")?
            .join("oi-observations"),
    );
    {
        let mut current=task.state.lock().map_err(|_|"Estado de registro no disponible")?;
        state.lock().map_err(|_|"Estado ocupado")?.set_etf_scanner_enabled(task.etf_scanner_enabled.load(Ordering::SeqCst));
        *current=Some(state.clone());
    }
    let published = task.published.clone();
    *published.lock().map_err(|_| "Estado publicado no disponible")? = None;
    *running = Some(tauri::async_runtime::spawn(async move {
        // Observe the existing direct feed without creating another Massive connection.
        let _direct_observer = std::net::TcpListener::bind(massive_feed::service_ipc::DIRECT_READONLY_ADDRESS)
            .and_then(|listener| massive_feed::service_ipc::serve_readonly(listener, std::sync::Arc::new(std::sync::Mutex::new(state.clone()))))
            .map_err(|_| eprintln!("Observador directo de solo lectura no disponible"));
        let (learning_tx, learning_rx) = tokio::sync::mpsc::channel(1);
        let (learning_v2_tx, learning_v2_rx) = tokio::sync::mpsc::channel(1);
        let publish = async {
            let mut next_learning_ms = 0;
            // Feed workers keep every market event; publish one complete UI snapshot per second.
            let mut timer = tokio::time::interval(std::time::Duration::from_millis(1000));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                timer.tick().await;
                storage.before_snapshot(&state).await;
                let mut snapshot = live::snapshot(&state);
                storage.save(&mut snapshot).await;
                reviewer.update(&mut snapshot).await;
                let now = live::now_ms();
                if now >= next_learning_ms {
                    let _ = learning_tx.try_send(snapshot.clone());
                    let _ = learning_v2_tx.try_send(snapshot.clone());
                    next_learning_ms = now.saturating_add(2_000);
                }
                if let Ok(mut live_state) = state.lock() {
                    live_state.decision_review = snapshot.decision_review.clone();
                }
                *published.lock().unwrap() = Some(std::sync::Arc::new(snapshot.clone()));
                if on_message.send(serde_json::to_value(snapshot).unwrap_or(serde_json::Value::Null)).is_err() {
                    break;
                }
            }
        };
        let neuro = async {
            if neuro_trader::open_neuro_trader(app.clone()).await.is_ok() {
                massive_feed::neuro_context::run(state.clone()).await;
            }
            std::future::pending::<()>().await;
        };
        tokio::select! { _ = live::run(key, state.clone()) => {}, _ = publish => {}, _ = neuro => {},
        _ = massive_feed::qqq_component_reading::run(state.clone(),directory.clone()) => {},
        _ = massive_feed::swing_observer::run(state.clone(),directory.clone()) => {},
        _ = learning::run(directory.clone(), learning_rx) => {},
        _ = learning_v2::run(directory.clone(), state.clone(), learning_v2_rx) => {} }
        let snapshot = live::snapshot(&state);
        let _ = on_message.send(serde_json::to_value(snapshot).unwrap_or(serde_json::Value::Null));
    }));
    Ok(())
}

#[tauri::command]
fn set_price_alerts(alerts:Vec<massive_feed::price_alerts::Alert>,task:tauri::State<'_,LiveTask>)->Result<(),String>{
 if task.remote.load(Ordering::SeqCst){massive_feed::service_ipc::request(serde_json::json!({"method":"alerts","alerts":alerts}))?;return Ok(());}

 let state=task.state.lock().map_err(|_|"Estado ocupado")?.clone().ok_or("Conecta Massive para vigilar alarmas")?;
 state.lock().map_err(|_|"Estado ocupado")?.price_alert_engine.configure(alerts)?;
 Ok(())
}

#[tauri::command]
async fn stop_live(task: tauri::State<'_, LiveTask>) -> Result<(), String> {
    task.remote.store(false,Ordering::SeqCst);
    *task.remote_published.lock().map_err(|_|"Estado ocupado")?=None;
    task.epoch.fetch_add(1, Ordering::SeqCst);
    let handle = task
        .handle
        .lock()
        .map_err(|_| "Estado de conexión no disponible")?
        .take();
    if let Some(handle) = handle {
        handle.abort();
        let _ = handle.await;
    }
    *task.published.lock().map_err(|_| "Estado publicado no disponible")? = None;
    Ok(())
}

#[tauri::command]
async fn export_live_recording(
    app: tauri::AppHandle,
    task: tauri::State<'_, LiveTask>,
) -> Result<String, String> {
    if task.remote.load(Ordering::SeqCst){
      let directory=app.path().app_local_data_dir().map_err(|_|"Carpeta no disponible")?.join("recordings");
      return tauri::async_runtime::spawn_blocking(move||{
       let value=massive_feed::service_ipc::request(serde_json::json!({"method":"recording"}))?;
       let recording:massive_feed::recording::Recording=serde_json::from_value(value).map_err(|_|"Registro inválido")?;
       recording.save(&directory).map(|p|p.to_string_lossy().into_owned()).map_err(str::to_owned)
      }).await.map_err(|_|"Exportación no disponible")?;
    }

    let state = task
        .state
        .lock()
        .map_err(|_| "Estado de registro no disponible")?
        .clone()
        .ok_or("Conecta antes de exportar un registro")?;
    let recording = {
        let state = state.lock().map_err(|_| "Registro no disponible")?;
        massive_feed::recording::Recording::capture(&state.symbol, &state.telemetry)
    };
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta local no disponible")?
        .join("recordings");
    tauri::async_runtime::spawn_blocking(move || {
        recording
            .save(&directory)
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "No se pudo completar la exportación".to_owned())?
}

#[tauri::command]
fn get_live_assets() -> std::collections::BTreeMap<String, snipers_core::universe::AssetConfig> {
    snipers_core::universe::assets().clone()
}

/// Passive scanner inputs: own evidence only, no subscription or bot activation.
#[tauri::command]
fn set_direction_scanner(enabled:bool,task:tauri::State<'_,LiveTask>)->Result<(),String>{
 if task.remote.load(Ordering::SeqCst){massive_feed::service_ipc::request(serde_json::json!({"method":"direction_enabled","enabled":enabled}))?;return Ok(());}

    let shared=task.state.lock().map_err(|_|"Estado no disponible")?.clone().ok_or("Conecta Mercado en vivo antes de iniciar el escáner")?;
    let mut state=shared.lock().map_err(|_|"Estado ocupado")?;
    state.direction_enabled=enabled;
    state.scanner_options_notify.notify_one();
    Ok(())
}
#[tauri::command]
fn get_direction_scanner_inputs(task: tauri::State<'_, LiveTask>) -> Result<serde_json::Value,String> {
 if task.remote.load(Ordering::SeqCst){return massive_feed::service_ipc::request(serde_json::json!({"method":"direction"}));}

    let connected=task.handle.lock().map_err(|_|"Conexión no disponible")?.is_some();
    if !connected {return Ok(serde_json::json!({"connected":false,"frames":[]}));}
    let published=task.published.lock().map_err(|_|"Lectura no disponible")?.clone();
    let Some(s)=published else {return Ok(serde_json::json!({"connected":true,"frames":[]}));};
    let now=live::now_ms();
    let mut frames=vec![serde_json::json!({"symbol":s.symbol,"startedMs":s.started_ms,"publishedMs":s.published_ms,
        "stocks":s.stocks,"options":s.options,"ticks":s.ticks,"metrics":s.metrics,
        "treasury":s.treasury,"directionOi":s.direction_oi,"minutes":s.minutes,"cvd":s.cvd,"chartHistory":s.chart_history,"flow":s.flow,"optionSnapshotStatus":s.option_snapshot_status})];
    let shared=task.state.lock().map_err(|_|"Estado no disponible")?.clone();
    if let Some(shared)=shared {
        let symbols={let state=shared.lock().map_err(|_|"Estado ocupado")?;
            let mut symbols=state.scanner_options.symbols();
            symbols.truncate(20);symbols};
        for symbol in symbols {if symbol!=s.symbol {
            let frame=live::bot_frame(&shared,&symbol,now);
            if frame["ticks"][&symbol].is_object(){frames.push(frame);}
        }}
    }
    for frame in &mut frames {if let Some(minutes)=frame["chartHistory"]["minutes"].as_array_mut(){if minutes.len()>30 {minutes.drain(..minutes.len()-30);}}}
    let enabled=shared_direction_enabled(&task);
    Ok(serde_json::json!({"connected":true,"enabled":enabled,"frames":frames}))
}
#[tauri::command]
async fn get_qqq_component_inputs(enabled:bool,task:tauri::State<'_,LiveTask>)->Result<serde_json::Value,String>{
 if task.remote.load(Ordering::SeqCst){return tauri::async_runtime::spawn_blocking(move||massive_feed::service_ipc::request(serde_json::json!({"method":"qqq","enabled":enabled}))).await.map_err(|_|"Motor independiente no disponible")?;}

    let shared=task.state.lock().map_err(|_|"Estado no disponible")?.clone();
    let Some(shared)=shared else {return Ok(serde_json::json!({"connected":false,"frames":[]}));};
    let mut state=shared.lock().map_err(|_|"Estado ocupado")?;
    state.qqq_ui_enabled=enabled;
    Ok(serde_json::json!({"connected":state.stocks.phase=="streaming","report":state.qqq_report}))
}

#[tauri::command]
async fn get_extended_chart_history(symbol: String) -> Result<massive_feed::chart_history::ChartHistory, String> {
    if snipers_core::universe::stock_asset(&symbol).is_none() {
        return Err("Activo no habilitado para historial de gráfica".into());
    }
    let key = Credentials::load().map_err(str::to_owned)?;
    massive_feed::history::fetch_chart_history(&key, &symbol, live::now_ms())
        .await
        .map_err(str::to_owned)
}
fn shared_direction_enabled(task:&LiveTask)->bool{task.state.lock().ok().and_then(|s|s.clone()).and_then(|s|s.lock().ok().map(|s|s.direction_enabled)).unwrap_or(false)}

#[tauri::command]
fn refresh_option_snapshot(task: tauri::State<'_, LiveTask>) -> Result<(), String> {
 if task.remote.load(Ordering::SeqCst){massive_feed::service_ipc::request(serde_json::json!({"method":"refresh_options"}))?;return Ok(());}

    if task
        .handle
        .lock()
        .map_err(|_| "Conexión no disponible")?
        .is_none()
    {
        return Err("Conecta antes de consultar snapshots".into());
    }
    let state = task
        .state
        .lock()
        .map_err(|_| "Estado no disponible")?
        .clone()
        .ok_or("Estado no disponible")?;
    let state = state.lock().map_err(|_| "Estado no disponible")?;
    if state.option_snapshot_status.phase == "loading" {
        return Ok(());
    }
    if state
        .option_snapshot_status
        .observed_ms
        .is_some_and(|t| live::now_ms().saturating_sub(t) < 10_000)
    {
        return Err("Espera diez segundos antes de repetir la consulta".into());
    }
    state.snapshot_notify.notify_one();
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(LiveTask::default())
        .manage(option_research::Task::default())
        .manage(neuro_trader::Task::default())
        .manage(paper_bot::Task::default())
        .manage(research_observations::Task::default())
        .setup(|app| {
            if massive_feed::snapshot_bridge::start().is_err(){eprintln!("Servicio snapshot pendiente de configuración");}
            option_research::start(app.handle().clone());
            paper_bot::start(app.handle().clone());
            block_alerts::start(app.handle().clone());
            research_observations::start(app.handle().clone());
            // Refresh the daily targets independently of the selected UI view.
            let _ = analyst_targets::get_fmp_targets(app.handle().clone(), app.state::<analyst_targets::Task>());
            Ok(())
        })
        .manage(historical::Task::default())
        .manage(investment::Task::default())
        .manage(block_alerts::Task::default())
        .manage(analyst_targets::Task::default())
        .manage(iv_history::Task::default())
        .manage(structure_study::Task::default())
        .invoke_handler(tauri::generate_handler![
            get_snapshot_status,get_snapshot_bundle,record_snapshot_section,
            option_research::get_option_research_status,
            option_research::get_option_control_frame,
            option_research::record_option_formula_baseline,
            get_extended_chart_history,
            research_observations::set_research_watchlist,
            research_observations::get_research_capture_status,
            research_observations::get_baseline_evaluation,
            route_4h::preview_route_4h, route_4h::study_route_4h,
            iv_history::get_iv_history,
            structure_study::structure_study,
            structure_study::structure_queue,
            etf_scanner::get_etf_scanner,
            etf_scanner::get_contract_inventory,
            etf_scanner::get_weekly_range_snapshot,
            etf_scanner::set_etf_scanner_enabled,

            analyst_targets::get_fmp_targets,
            neuro_trader::open_neuro_trader,
            neuro_learning::neuro_confirmation,
            investment::analyze_investment_individual,
            investment::get_investment_individual,
            investment::get_benzinga_news,
            investment::record_news_alerts,
            investment::research_investment_company,
            investment::search_daily_purchases,
            investment::get_saved_daily_purchases,
            investment::get_saved_overnight_purchases,
            investment::get_overnight_closing_bar,
            investment::get_overnight_qqq_holdings,
            investment::save_overnight_observation,
            investment::get_overnight_observations,
            investment::get_daily_purchases_universe,
            block_alerts::get_block_alerts,
            block_alerts::set_block_alerts,
            block_alerts::mark_block_alerts_read,
            option_print_scanner::scan_option_prints,
            option_print_scanner::get_option_prints,
            investment::start_investment_radar,
            investment::get_investment_radar,
            company_research::get_company_research_catalog,
            company_research::get_company_research_detail,
            company_research::start_company_research,
            company_research::register_company_research,
            investment::stop_investment_radar,
            paper_bot::get_paper_bot_state,
            paper_bot::get_contract_sensitivities,
            paper_bot::get_paper_bot_history,
            paper_bot::get_paper_bot_decisions,
            paper_bot::arm_paper_bot,
            paper_bot::connect_top_five_bot,
            paper_bot::set_paper_bot_asset_mode,
            paper_bot::pause_paper_bot,
            paper_bot::close_paper_bot,
            paper_bot::check_paper_bot,
            alpaca::get_alpaca_paper_snapshot,
            alpaca::reconcile_alpaca_paper_orders,
            alpaca::get_alpaca_paper_outbox,
            alpaca::get_alpaca_option_quote,
            alpaca::get_alpaca_option_contracts,
            alpaca::get_alpaca_paper_order,
            alpaca::submit_alpaca_paper_order,
            alpaca::cancel_alpaca_paper_order,
            get_system_clock_diagnostic,
            get_learning_dashboard,
            record_gamma_learning,
            get_paper_journal,
            get_analyst_context,
            preview_legacy_database,
            get_legacy_journal,
            import_legacy_database,
            manual_scanner::search_top_five_now,
            manual_scanner::get_manual_scanner_profiles,
            start_historical_search,
            stop_historical_search,
            get_live_assets,
            get_direction_scanner_inputs,
            get_qqq_component_inputs,
            set_direction_scanner,
            start_live,
            set_price_alerts,
            stop_live,
            export_live_recording,
            refresh_option_snapshot
        ])
        .build(tauri::generate_context!())
        .expect("Unable to start Snipers desktop")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<neuro_trader::Task>().stop();
            }
        });
}



