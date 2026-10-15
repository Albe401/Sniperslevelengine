//! Autonomous QQQ observation bridge. Never changes decisions or sends orders.
use massive_feed::{
    live, option_activity_observer as observer, option_joint_audit, option_outcome_prices,
};
use serde_json::{json, Value};
use snipers_core::calendar::{self, Day};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::Manager;
const DATE: &str = "2026-10-12";
const VOLUME:&str=include_str!("../../../../docs/audits/option-activity-baseline-breadth-60/baseline-1791644419099-23852-540.json");
const PREMIUM:&str=include_str!("../../../../docs/audits/option-premium-breadth-certified-60/premium-baseline-1791653186106-33700-1.json");
const POLICY: &str =
    include_str!("../../../../docs/audits/2026-10-10-option-future-outcomes-policy.json");
pub struct Task {
    status: Mutex<Value>,
    last_packet: Mutex<u64>,
}
impl Default for Task {
    fn default() -> Self {
        Self {
            status: Mutex::new(
                json!({"status":"WAITING_CONNECTION","formulaChanged":false,"executionEnabled":false}),
            ),
            last_packet: Mutex::new(0),
        }
    }
}
fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta de observaciones no disponible")?
        .join("option-engines")
        .join(DATE))
}
fn once(path: &Path, text: &str) -> Result<(), String> {
    if !path.exists() {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| "No se pudo crear evidencia")?;
        f.write_all(text.as_bytes())
            .map_err(|_| "No se pudo guardar evidencia")?;
    }
    Ok(())
}
fn publish(app: &tauri::AppHandle, value: Value) {
    if let Ok(mut s) = app.state::<Task>().status.lock() {
        *s = value;
    }
}
fn action(now: u64, open: u64, close: u64) -> Option<(&'static str, u64)> {
    if now < open + 60000 || now >= close + 120000 {
        return None;
    }
    let offset = now.saturating_sub(open) % 300000;
    if now < close && offset >= 240000 {
        Some(("arm", open + (now - open) / 300000 * 300000 + 300000))
    } else if offset <= 120000 && now >= open + 600000 {
        Some(("capture", open + (now - open) / 300000 * 300000))
    } else {
        None
    }
}
async fn tick(
    root: &Path,
    now: u64,
    attempts: &mut BTreeMap<String, usize>,
) -> Result<Value, String> {
    let Day::Open(session) = calendar::schedule(calendar::date_at(now).ok_or("Reloj inválido")?)
    else {
        return Ok(json!({"status":"WAITING_SESSION"}));
    };
    if session.date != DATE {
        return Ok(json!({"status":"WAITING_CONFIGURED_SESSION","targetDate":DATE}));
    }
    if now < session.open_ms + 60000 {
        return Ok(json!({"status":"WAITING_FIRST_CLOSED_MINUTE"}));
    }
    std::fs::create_dir_all(root).map_err(|_| "No se pudo crear diario")?;
    let volume = root.join("volume");
    let premium = root.join("premium");
    once(&root.join("volume-reference.json"), VOLUME)?;
    once(&root.join("premium-reference.json"), PREMIUM)?;
    once(&root.join("outcome-policy.json"), POLICY)?;
    let mut operations = vec![];
    for (mode, dir) in [(false, &volume), (true, &premium)] {
        if !dir.join("frozen-session.json").exists() && now < session.close_ms {
            let key = format!("prepare-{mode}");
            let count = attempts.entry(key).or_default();
            if *count < 6 {
                *count += 1;
                let result = if mode {
                    observer::prepare_premium(&root.join("premium-reference.json"), "QQQ", dir)
                        .await
                } else {
                    observer::prepare(&root.join("volume-reference.json"), "QQQ", dir).await
                };
                operations.push(json!({"engine":if mode{"premium"}else{"volume"},"operation":"prepare","result":result}));
            }
        }
    }
    if let Some((op, boundary)) = action(now, session.open_ms, session.close_ms) {
        let mut jobs = vec![];
        for (mode, dir) in [(false, &volume), (true, &premium)] {
            if !dir.join("frozen-session.json").exists() {
                continue;
            }
            let file = if op == "arm" {
                dir.join(format!("context-{boundary}.json"))
            } else {
                dir.join(format!(
                    "{}window-{}.json",
                    if mode { "premium-" } else { "" },
                    boundary - 300000
                ))
            };
            if file.exists() {
                continue;
            }
            let key = format!("{op}-{mode}-{boundary}");
            let count = attempts.entry(key).or_default();
            if *count >= 3 {
                continue;
            }
            *count += 1;
            jobs.push(async move {
                let result = if op == "arm" {
                    observer::arm(dir).await
                } else if mode {
                    observer::capture_premium(dir).await
                } else {
                    observer::capture(dir).await
                };
                json!({"engine":if mode{"premium"}else{"volume"},"operation":op,"result":result})
            });
        }
        operations.extend(futures_util::future::join_all(jobs).await);
    }
    let audit = option_joint_audit::report(&[volume.clone()], &[premium.clone()], live::now_ms())?;
    let mut followup = Value::Null;
    if now >= session.close_ms + 120000 && !root.join("outcomes-complete.json").exists() {
        let count = attempts.entry("outcomes".into()).or_default();
        if *count < 3 {
            *count += 1;
            match option_outcome_prices::collect(
                &[volume],
                &[premium],
                &root.join("outcome-policy.json"),
                &root.join("outcomes"),
            )
            .await
            {
                Ok(v) => {
                    if v["summary"]["status"] == "PRICE_EVIDENCE_ARCHIVED" {
                        let manifest: Value = serde_json::from_slice(
                            &std::fs::read(
                                v["summary"]["outcomeManifestFile"]
                                    .as_str()
                                    .ok_or("Manifiesto ausente")?,
                            )
                            .map_err(|_| "Manifiesto inaccesible")?,
                        )
                        .map_err(|_| "Manifiesto inválido")?;
                        let mut prices = vec![];
                        for file in manifest["priceEvidenceFiles"]
                            .as_array()
                            .ok_or("Precios ausentes")?
                        {
                            prices.push(
                                serde_json::from_slice::<
                                    massive_feed::option_future_outcomes::PriceEvidence,
                                >(
                                    &std::fs::read(file.as_str().ok_or("Ruta inválida")?)
                                        .map_err(|_| "Precios inaccesibles")?,
                                )
                                .map_err(|_| "Precios inválidos")?,
                            );
                        }
                        let policy =
                            serde_json::from_str(POLICY).map_err(|_| "Política inválida")?;
                        let labels = massive_feed::option_future_outcomes::build(
                            &[root.join("volume")],
                            &[root.join("premium")],
                            &prices,
                            &policy,
                            live::now_ms(),
                        )?;
                        once(&root.join("future-outcomes.json"), &labels.to_string())?;
                        if root.join("formula-baseline.jsonl").exists() {
                            let raw = std::fs::read_to_string(root.join("formula-baseline.jsonl"))
                                .map_err(|_| "Diario inaccesible")?;
                            let rows: Vec<Value> = raw
                                .lines()
                                .map(serde_json::from_str)
                                .collect::<Result<_, _>>()
                                .map_err(|_| "Diario incompleto")?;
                            once(&root.join("formula-journal.json"),&json!({"schema":"route-observation-v1","formulaBaseline":{"version":"option-formula-baseline-archive-v1","rows":rows,"dropped":0}}).to_string())?;
                        }
                        once(&root.join("outcomes-complete.json"), &v.to_string())?;
                    }
                    followup = v;
                }
                Err(e) => followup = json!({"status":"PENDING_OUTCOMES","reason":e}),
            }
        }
    }
    Ok(
        json!({"status":"OBSERVING","targetDate":DATE,"directory":root,"operations":operations,"audit":audit,"followup":followup,"formulaChanged":false,"executionEnabled":false,"ordersSent":0}),
    )
}
pub fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut attempts = BTreeMap::new();
        loop {
            let now = live::now_ms();
            let connected = {
                let task = app.state::<crate::LiveTask>();
                let frame = task.published.lock().ok().and_then(|p| p.clone());
                let enabled = task.handle.lock().is_ok_and(|h| h.is_some());
                let remote=task.remote_published.lock().ok().and_then(|f|f.clone());
                let remote_current=task.remote.load(std::sync::atomic::Ordering::SeqCst)&&remote.as_ref().is_some_and(|r|r["symbol"]=="QQQ"&&r["publishedMs"].as_u64().is_some_and(|t|t<=now&&now-t<=15000));
                let selected = remote_current || frame.is_some_and(|f| {
                        f.symbol == "QQQ" && f.published_ms <= now && now - f.published_ms <= 15000
                    });
                // QQQ can be monitored by the bot while another asset is selected in the UI.
                let shared = task.state.lock().ok().and_then(|s|s.clone());
                let background = enabled && !selected && shared.is_some_and(|s| {
                    let qqq = live::bot_frame(&s,"QQQ",now);
                    qqq["symbol"]=="QQQ" && qqq["stocks"]["phase"]=="streaming"
                        && qqq["publishedMs"].as_u64().is_some_and(|t|t<=now&&now-t<=15000)
                });
                enabled && (selected || background)
            };
            let value = if connected {
                match root(&app) {
                    Ok(r) => tick(&r, now, &mut attempts)
                        .await
                        .unwrap_or_else(|e| json!({"status":"PENDING","reason":e})),
                    Err(e) => json!({"status":"PENDING","reason":e}),
                }
            } else {
                json!({"status":"WAITING_QQQ_CONNECTION","targetDate":DATE})
            };
            publish(&app, value);
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });
}
#[tauri::command]
pub fn get_option_research_status(task: tauri::State<'_, Task>) -> Result<Value, String> {
    task.status
        .lock()
        .map(|v| v.clone())
        .map_err(|_| "Estado ocupado".into())
}
/// Own-symbol inputs for the autonomous control; does not activate trading or subscribe.
#[tauri::command]
pub fn get_option_control_frame(task:tauri::State<'_,crate::LiveTask>)->Result<Value,String>{
    if !task.handle.lock().map_err(|_|"Conexión ocupada")?.is_some(){return Ok(json!({"connected":false}));}
    let now=live::now_ms();
    if task.remote.load(std::sync::atomic::Ordering::SeqCst){return Ok(task.remote_published.lock().map_err(|_|"Publicación ocupada")?.clone().map_or_else(||json!({"connected":false}),|frame|control_input(frame,now)));}
    if let Some(frame)=task.published.lock().map_err(|_|"Publicación ocupada")?.as_ref().filter(|f|f.symbol=="QQQ") {
        return Ok(control_input(serde_json::to_value(frame).map_err(|_|"Frame no disponible")?,now));
    }
    let shared=task.state.lock().map_err(|_|"Estado ocupado")?.clone();
    Ok(shared.map_or_else(||json!({"connected":false}),|s|control_input(live::bot_frame(&s,"QQQ",now),now)))
}
fn control_input(frame:Value,now:u64)->Value {
    let current=frame["symbol"]=="QQQ"&&frame["stocks"]["phase"]=="streaming"
        &&frame["publishedMs"].as_u64().is_some_and(|t|t<=now&&now-t<=3000)
        &&frame["ticks"]["QQQ"]["tradeMs"].as_u64().is_some_and(|t|t<=now&&now-t<=3000)
        &&frame["flow"].is_object()&&frame["metrics"].is_object();
    if current {json!({"connected":true,"frame":frame,"source":"own-qqq-shared-feed"})}
    else {json!({"connected":false,"reason":"Esperando datos propios y vigentes de QQQ"})}
}
pub fn bot_status(app: &tauri::AppHandle) -> Value {
    app.state::<Task>().status.lock().map(|v|v.clone()).unwrap_or_else(|_|json!({"status":"PENDING"}))
}
#[tauri::command]
pub fn record_option_formula_baseline(
    app: tauri::AppHandle,
    task: tauri::State<'_, Task>,
    payload: Value,
) -> Result<(), String> {
    let now = live::now_ms();
    let at = payload["observedMs"].as_u64().ok_or("Reloj ausente")?;
    let published = payload["publishedMs"]
        .as_u64()
        .ok_or("Publicación ausente")?;
    if payload["version"] != "option-formula-baseline-v1"
        || payload["source"] != "desktop-live"
        || payload["symbol"] != "QQQ"
        || payload["session"] != DATE
        || at > now
        || now - at > 3000
        || published > at
        || at - published > 3000
    {
        return Err("Snapshot no contemporáneo".into());
    }
    let live_task = app.state::<crate::LiveTask>();
    if !live_task.handle.lock().is_ok_and(|h| h.is_some()) {
        return Err("Monitor desconectado".into());
    }
    let mut last = task.last_packet.lock().map_err(|_| "Registro ocupado")?;
    if at / 1000 <= *last {
        return Ok(());
    }
    let dir = root(&app)?;
    std::fs::create_dir_all(&dir).map_err(|_| "Carpeta no disponible")?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("formula-baseline.jsonl"))
        .map_err(|_| "Diario no disponible")?;
    writeln!(f, "{payload}").map_err(|_| "No se pudo guardar snapshot")?;
    *last = at / 1000;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_control_rejects_foreign_stale_future_and_missing_inputs(){
        let now=100000;
        let frame=json!({"symbol":"QQQ","stocks":{"phase":"streaming"},"publishedMs":now,"ticks":{"QQQ":{"tradeMs":now}},"metrics":{},"flow":{}});
        assert_eq!(control_input(frame.clone(),now)["connected"],true);
        for kind in 0..5 {let mut f=frame.clone();match kind {0=>f["symbol"]=json!("AAPL"),1=>f["publishedMs"]=json!(now-3001),2=>f["ticks"]["QQQ"]["tradeMs"]=json!(now+1),3=>f["flow"]=Value::Null,_=>f["stocks"]["phase"]=json!("disconnected")};assert_eq!(control_input(f,now)["connected"],false);}
    }
    #[tokio::test]
    async fn closed_market_does_not_create_capture_sources() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("session");
        let now = calendar::legacy_eastern_ms("2026-10-10 10:00:00").unwrap();
        let result = tick(&directory, now, &mut BTreeMap::new()).await.unwrap();
        assert_eq!(result["status"], "WAITING_SESSION");
        assert!(!directory.exists());
    }
    #[tokio::test]
    async fn other_dates_and_early_monday_do_not_request_data() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("session");
        for (time, status) in [
            ("2026-10-13 10:00:00", "WAITING_CONFIGURED_SESSION"),
            ("2026-10-12 09:30:30", "WAITING_FIRST_CLOSED_MINUTE"),
        ] {
            let result = tick(
                &directory,
                calendar::legacy_eastern_ms(time).unwrap(),
                &mut BTreeMap::new(),
            )
            .await
            .unwrap();
            assert_eq!(result["status"], status);
            assert!(!directory.exists());
        }
    }
    #[test]
    fn original_evidence_is_not_replaced() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("evidence.json");
        once(&file, "original").unwrap();
        once(&file, "changed").unwrap();
        assert_eq!(std::fs::read_to_string(file).unwrap(), "original");
    }
    #[test]
    fn embedded_references_are_prior_to_monday() {
        for raw in [VOLUME, PREMIUM] {
            let b: snipers_core::option_activity_baseline::Baseline =
                serde_json::from_str(raw).unwrap();
            assert!(b.available_before_cutoff && b.built_ms < b.cutoff_ms && !b.execution_enabled);
            assert!(b
                .cohorts
                .iter()
                .all(|c| c.dates.iter().all(|d| d.as_str() < DATE)));
            assert_eq!(
                b.cutoff_ms,
                calendar::legacy_eastern_ms("2026-10-12 09:30:00").unwrap()
            );
        }
    }
    #[test]
    fn cadence_does_not_arm_or_capture_late() {
        let open = 1000000;
        let close = open + 23400000;
        assert_eq!(action(open + 59000, open, close), None);
        assert_eq!(
            action(open + 240000, open, close),
            Some(("arm", open + 300000))
        );
        assert_eq!(
            action(open + 600000, open, close),
            Some(("capture", open + 600000))
        );
        assert_eq!(action(open + 721000, open, close), None);
        assert_eq!(action(close + 120000, open, close), None);
    }
}
