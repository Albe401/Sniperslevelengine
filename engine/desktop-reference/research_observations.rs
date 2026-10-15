//! Passive, view-independent evidence capture. No subscriptions, predictions or orders.
use massive_feed::live;
use serde_json::{json, Value};
use snipers_core::calendar::{self, Day};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Mutex,
};
use tauri::Manager;

pub struct Task {
    symbols: Mutex<Vec<String>>,
    status: Mutex<Value>,
    evaluation:Mutex<Value>,
}
impl Default for Task {
    fn default() -> Self {
        Self {
            evaluation:Mutex::new(Value::Null),
            symbols: Mutex::new(
                ["QQQ", "SPY", "GOOGL", "NVDA", "AAPL"]
                    .map(str::to_owned)
                    .to_vec(),
            ),
            status: Mutex::new(
                json!({"version":"research-capture-v1","state":"waiting","predictionEnabled":false}),
            ),
        }
    }
}
fn clean(symbols: Vec<String>) -> Result<Vec<String>, String> {
    if symbols.len() > 5 {
        return Err("El recolector admite hasta cinco activos".into());
    }
    let mut out = Vec::new();
    for symbol in symbols {
        let symbol = symbol.trim().to_uppercase();
        if snipers_core::universe::stock_asset(&symbol).is_none() {
            return Err("Activo no habilitado para observaciones".into());
        }
        if !out.contains(&symbol) {
            out.push(symbol);
        }
    }
    if out.is_empty() {
        return Err("Selecciona al menos un activo".into());
    }
    Ok(out)
}
fn directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta local no disponible".into())
}
#[tauri::command]
pub async fn set_research_watchlist(
    app: tauri::AppHandle,
    task: tauri::State<'_, Task>,
    symbols: Vec<String>,
) -> Result<(), String> {
    let symbols = clean(symbols)?;
    let file = directory(&app)?.join("research-watchlist-v1.json");
    let saved = symbols.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            file,
            json!({"version":"research-watchlist-v1","symbols":saved}).to_string(),
        )
    })
    .await
    .map_err(|_| "No se pudo guardar la vigilancia")?
    .map_err(|_| "No se pudo guardar la vigilancia")?;
    *task.symbols.lock().map_err(|_| "Vigilancia ocupada")? = symbols;
    Ok(())
}
#[tauri::command]
pub fn get_research_capture_status(task: tauri::State<'_, Task>) -> Result<Value, String> {
    task.status
        .lock()
        .map(|v| v.clone())
        .map_err(|_| "Estado ocupado".into())
}
/// Read only forecasts already committed to the observation journals.
pub fn bot_context(app: &tauri::AppHandle, symbol: &str, side: &str, now: u64) -> Value {
    let task = app.state::<Task>();
    let status = task.status.lock().map(|v| v.clone()).unwrap_or(Value::Null);
    bot_probability_context(&status, symbol, side, now)
}
fn bot_probability_context(status: &Value, symbol: &str, side: &str, now: u64) -> Value {
    let asset = &status["assets"][symbol];
    let baseline = &asset["arrivalBaseline"];
    let version = "log-brownian-rth-arrival-v1";
    let mut forecasts = Vec::new();
    if asset["recorded"] == true && asset["quality"]["phase"] == "RTH"
        && asset["quality"]["streaming"] == true && baseline["version"] == version
        && baseline["calibrated"] == false {
        for f in baseline["forecasts"].as_array().into_iter().flatten() {
            let direction = f["case"]["direction"].as_str().unwrap_or("");
            let valid = f["version"] == version && f["calibrated"] == false
                && f["case"]["level"]["symbol"] == symbol
                && matches!(f["case"]["level"]["horizon"].as_str(), Some("DAY" | "WEEK"))
                && matches!(direction, "up" | "down")
                && f["issuedMs"].as_u64().is_some_and(|t| t <= now && now-t <= 120_000)
                && f["case"]["deadlineMs"].as_u64().is_some_and(|t| t > now)
                && f["returns"].as_u64().is_some_and(|n| n >= 60)
                && f["probability"].as_f64().is_some_and(|p| p.is_finite() && (0.0..=1.0).contains(&p));
            if valid {
                let relation = match (side, direction) {
                    ("CALL", "up") | ("PUT", "down") => "same-direction",
                    ("CALL", "down") | ("PUT", "up") => "opposite-direction",
                    _ => "no-bot-direction",
                };
                forecasts.push(json!({"forecast":f,"relationToBot":relation}));
            }
        }
    }
    json!({"version":"bot-arrival-context-v1","symbol":symbol,"readAtMs":now,
        "state":if forecasts.is_empty(){"waiting"}else{"available"},
        "botSide":side,"forecasts":forecasts,"calibrated":false,"executionEnabled":false,
        "routeInvalidation":null,
        "explanation":"Contexto experimental de llegada a niveles; misma dirección no confirma entrada y dirección opuesta no invalida la ruta. No hay invalidación calculada por este modelo; las reglas del bot permanecen vigentes."})
}
#[tauri::command]
pub async fn get_baseline_evaluation(app:tauri::AppHandle,task:tauri::State<'_,Task>)->Result<Value,String>{
    let now=live::now_ms();
    if let Ok(cached)=task.evaluation.lock(){if cached["evaluatedMs"].as_u64().is_some_and(|t|t<=now&&now-t<300_000){return Ok(cached.clone());}}
    let dir=directory(&app)?;
    let summary=tauri::async_runtime::spawn_blocking(move||->Result<Value,String>{
        let report=snipers_core::level_dataset::audit(&dir.join("baseline-observations"),now,false).map_err(|e|e.to_string())?;
        let summary=json!({"version":"baseline-evaluation-v1","evaluatedMs":now,"forecasts":report["baselineForecasts"],"states":report["baselineStatusCounts"],"metrics":report["baselineMetrics"],"errors":report["errors"].as_array().map(Vec::len).unwrap_or(0),"validated":false});
        std::fs::write(dir.join("arrival-baseline-evaluation.json"),summary.to_string()).map_err(|e|e.to_string())?;
        Ok(summary)
    }).await.map_err(|e|e.to_string())??;
    *task.evaluation.lock().map_err(|_|"Evaluación ocupada")?=summary.clone();Ok(summary)
}

fn regular(now: u64) -> bool {
    calendar::date_at(now).is_some_and(|d| match calendar::schedule(d) {
        Day::Open(s) => now >= s.open_ms && now < s.close_ms,
        _ => false,
    })
}
fn timestamp_quality(at: Option<u64>, now: u64, max_age: u64) -> &'static str {
    match at {
        None | Some(0) => "missing",
        Some(t) if t > now => "future",
        Some(t) if now - t > max_age => "stale",
        _ => "available",
    }
}
/// Field times remain distinct from capture/publication time. Raw evidence is not a training label.
fn record(symbol: &str, mut frame: Value, now: u64) -> Value {
    if frame["symbol"].as_str() != Some(symbol) {
        frame = Value::Null;
    }
    let tick = frame["ticks"][symbol].clone();
    let price = tick["price"].as_f64().filter(|n| n.is_finite() && *n > 0.);
    let price_at = tick["tradeMs"].as_u64();
    let price_quality = if price.is_none() {
        "missing"
    } else {
        timestamp_quality(price_at, now, 15_000)
    };
    let streaming = frame["stocks"]["phase"] == "streaming";
    let eligible = regular(now) && streaming && price_quality == "available";
    let mut bars = frame["chartHistory"]["minutes"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let full_history:Vec<snipers_core::level_research::ObservedBar>=bars.iter().filter_map(|b|Some(snipers_core::level_research::ObservedBar{
        symbol:symbol.into(),timestamp_ms:b["timestampMs"].as_u64()?,available_ms:now,
        open:b["open"].as_f64()?,high:b["high"].as_f64()?,low:b["low"].as_f64()?,close:b["close"].as_f64()?
    })).collect();
    bars.sort_by_key(|b| b["timestampMs"].as_u64().unwrap_or(u64::MAX));
    let mut seen = BTreeSet::new();
    let duplicate = bars
        .iter()
        .filter_map(|b| b["timestampMs"].as_u64())
        .any(|t| !seen.insert(t));
    let invalid = bars.iter().any(|b| {
        let values = ["open", "high", "low", "close", "volume"]
            .map(|k| b[k].as_f64().filter(|v| v.is_finite()));
        match values {
            [Some(o), Some(h), Some(l), Some(c), Some(v)] => {
                l <= 0.
                    || h < o.max(c)
                    || l > o.min(c)
                    || v < 0.
                    || b["timestampMs"].as_u64().is_none()
            }
            _ => true,
        }
    });
    let future_bar = bars
        .iter()
        .any(|b| b["timestampMs"].as_u64().is_some_and(|t| t > now));
    bars.retain(|b| {
        b["timestampMs"]
            .as_u64()
            .is_some_and(|t| t.saturating_add(60_000) <= now)
    });
    if bars.len() > 5 {
        bars.drain(..bars.len() - 5);
    }
    let closed_at = bars
        .last()
        .and_then(|b| b["timestampMs"].as_u64())
        .map(|t| t.saturating_add(60_000));
    let bar_quality = if duplicate {
        "duplicate"
    } else if invalid {
        "invalid"
    } else if future_bar {
        "future"
    } else {
        timestamp_quality(closed_at, now, 120_000)
    };
    // Preserve horizon roles/scores/coverage without duplicating every option contract.
    let option = &frame["optionSnapshotStatus"];
    let horizons=option["horizons"].as_array().map(|rows|rows.iter().map(|h|{
        let structure=&h["structure"];
        json!({"mode":h["mode"],"label":h["label"],"referenceSource":h["referenceSource"],"observedMs":h["observedMs"],"expirations":h["expirations"],"exposure":h["exposure"],"reading":structure["reading"],"quality":timestamp_quality(h["observedMs"].as_u64(),now,300_000),"structure":structure})
    }).collect::<Vec<_>>()).unwrap_or_default();
    let mut row=json!({"version":"research-capture-v1","id":format!("{symbol}:{now}:{}",frame["startedMs"].as_u64().unwrap_or(0)),"symbol":symbol,"capturedMs":now,"availableMs":now,"connection":frame["startedMs"],"sourcePublishedMs":frame["publishedMs"],"session":calendar::date_at(now).map(|d|d.to_string()),"basis":"existing-feeds-point-in-time-no-orders","predictionEnabled":false,"outcome":null,
        "quality":{"phase":if regular(now){"RTH"}else{"OUTSIDE_REGULAR_SESSION"},"streaming":streaming,"priceObservationEligible":eligible,"price":price_quality,"bars":bar_quality,"options":timestamp_quality(option["observedMs"].as_u64(),now,300_000),"metrics":timestamp_quality(frame["metrics"]["asofMs"].as_u64(),now,180_000),"cvd":timestamp_quality(frame["cvd"]["at"].as_u64(),now,3000),"flow":timestamp_quality(frame["flow"]["asof_ms"].as_u64().or_else(||frame["flow"]["asofMs"].as_u64()),now,3000)},
        "evidence":{"tick":tick,"stocks":frame["stocks"],"options":frame["options"],"metrics":frame["metrics"],"chartHistory":{"minutes":bars,"sessions":frame["chartHistory"]["sessions"]},"flow":frame["flow"],"flowExpiry":frame["flowExpiry"],"optionFlowStatus":frame["optionFlowStatus"],"cvd":frame["cvd"],"gap":frame["gapReport"],"dailyRange":frame["dailyRange"],"dailyAtr":frame["dailyAtr"],"treasury":frame["treasury"],"directionOi":frame["directionOi"],"optionSnapshotStatus":{"phase":option["phase"],"detail":option["detail"],"observedMs":option["observedMs"],"observedDate":option["observedDate"],"previousObservedDate":option["previousObservedDate"],"contracts":option["contracts"],"oiMissing":option["oiMissing"],"greeksMissing":option["greeksMissing"],"storageError":option["storageError"],"exposureError":option["exposureError"],"expirations":option["expirations"],"horizons":horizons}}});
    let catalog=snipers_core::level_research::catalog(&row);
    row["arrivalBaseline"]=json!(snipers_core::arrival_baseline::forecast(&catalog.cases,&full_history,now));
    row["levelCatalog"]=json!(catalog);
    row
}
fn selected_frame(s: &live::LiveState) -> Value {
    json!({"symbol":s.symbol,"startedMs":s.started_ms,"publishedMs":s.published_ms,"stocks":s.stocks,"options":s.options,"ticks":{s.symbol.clone():s.ticks.get(&s.symbol)},"metrics":s.metrics,"chartHistory":s.chart_history,"flow":s.flow,"flowExpiry":null,"cvd":s.cvd,"gapReport":s.gap_report,"dailyAtr":s.daily_atr,"treasury":s.treasury,"directionOi":s.direction_oi,"optionSnapshotStatus":s.option_snapshot_status})
}
pub fn start(app: tauri::AppHandle) {
    if let Ok(dir) = directory(&app) {
        if let Ok(text) = std::fs::read_to_string(dir.join("research-watchlist-v1.json")) {
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                if let Some(rows) = v["symbols"].as_array() {
                    if let Ok(symbols) = clean(
                        rows.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect(),
                    ) {
                        if let Ok(mut current) = app.state::<Task>().symbols.lock() {
                            *current = symbols;
                        }
                    }
                }
            }
        }
    }
    tauri::async_runtime::spawn(async move {
        let mut saved: BTreeMap<String, (u64, u64, bool)> = BTreeMap::new();
        let mut records = 0_u64;
        loop {
            let now = live::now_ms();
            let rth = regular(now);
            let task = app.state::<Task>();
            let symbols = task.symbols.lock().map(|v| v.clone()).unwrap_or_default();
            saved.retain(|symbol, _| symbols.contains(symbol));
            let live_task = app.state::<crate::LiveTask>();
            let connected = live_task.handle.lock().is_ok_and(|v| v.is_some());
            let shared = if connected {
                live_task.state.lock().ok().and_then(|v| v.clone())
            } else {
                None
            };
            let published = if connected {
                live_task.published.lock().ok().and_then(|v| v.clone())
            } else {
                None
            };
            let remote_frame=if live_task.remote.load(std::sync::atomic::Ordering::SeqCst){live_task.remote_published.lock().ok().and_then(|v|v.clone())}else{None};
            let remote_frames=if remote_frame.is_some(){
                let requested=symbols.clone();
                tauri::async_runtime::spawn_blocking(move||massive_feed::service_ipc::request(json!({"method":"research_watch","symbols":requested}))).await.ok().and_then(Result::ok)
                    .and_then(|v|v["frames"].as_array().cloned()).unwrap_or_default()
            }else{Vec::new()};
            if let Some(shared)=&shared {let mut state=shared.lock().unwrap();if state.research_symbols!=symbols{state.research_symbols=symbols.clone();state.scanner_options_notify.notify_one();}}
            // IPC frames are published after the request; validate using a subsequent clock.
            let now=live::now_ms();
            let connection = remote_frame.as_ref().and_then(|f|f["startedMs"].as_u64()).unwrap_or_else(||published.as_ref().map(|s| s.started_ms).unwrap_or(0));
            let bucket = now / if rth { 60_000 } else { 300_000 };
            let mark = (bucket, connection, rth);
            let mut batch = Vec::new();
            for symbol in &symbols {
                if saved.get(symbol) == Some(&mark) {
                    continue;
                }
                let frame = if let Some(remote)=remote_frames.iter().find(|r|r["symbol"]==*symbol){remote.clone()}else if let Some(remote)=remote_frame.as_ref().filter(|r|r["symbol"]==*symbol){remote.clone()}else if let Some(s) = published.as_ref().filter(|s| s.symbol == *symbol) {
                    selected_frame(s)
                } else if let Some(shared) = &shared {
                    live::bot_frame(shared, symbol, now)
                } else {
                    Value::Null
                };
                batch.push((symbol.clone(), record(symbol, frame, now)));
            }
            if !batch.is_empty() {
                let dir = directory(&app);
                let result=tauri::async_runtime::spawn_blocking(move||->Result<Vec<(String,Value)>,String>{
                    let dir=dir?;let date=calendar::date_at(now).ok_or("Fecha de mercado desconocida")?.to_string();
                    let mut outcomes=Vec::new();
                    for (symbol,row) in batch {
                        let file=dir.join("research-observations").join(format!("research-{date}-{symbol}.jsonl"));
                        // Small replay journal keeps comparisons independent of bulky option telemetry.
                        let cases:Vec<Value>=row["arrivalBaseline"]["forecasts"].as_array().into_iter().flatten().map(|f|f["case"].clone()).collect();
                        let levels:Vec<Value>=cases.iter().map(|c|c["level"].clone()).collect();
                        let mut replay=row.clone();replay["evidence"]=json!({"tick":row["evidence"]["tick"],"chartHistory":row["evidence"]["chartHistory"]});
                        replay["levelCatalog"]=json!({"version":"level-contract-v1","observationId":row["id"],"levels":levels,"cases":cases,"blockers":[],"predictionEnabled":false});
                        let replay_file=dir.join("baseline-observations").join(format!("research-{date}-{symbol}.jsonl"));
                        let written=snipers_store::observation_log::append(&file,&row).and_then(|segment|{snipers_store::observation_log::append(&replay_file,&replay)?;Ok(segment)});
                        let status=match written{Ok(segment)=>json!({"recorded":true,"quality":row["quality"],"arrivalBaseline":row["arrivalBaseline"],"segment":segment.file_name().and_then(|v|v.to_str()),"error":null}),Err(e)=>json!({"recorded":false,"error":e.to_string()})};
                        outcomes.push((symbol,status));
                    }Ok(outcomes)
                }).await;
                let mut status = json!({"version":"research-capture-v1","at":now,"symbols":symbols,"predictionEnabled":false,"ordersEnabled":false,"state":if rth{"collecting"}else{"outside-session"},"assets":{}});
                match result {
                    Ok(Ok(rows)) => {
                        for (symbol, value) in rows {
                            if value["recorded"] == true {
                                saved.insert(symbol.clone(), mark);
                                records += 1;
                            }
                            status["assets"][&symbol] = value;
                        }
                    }
                    other => {
                        status["state"] = json!("error");
                        status["error"] = json!(match other {
                            Ok(Err(e)) => e,
                            Err(e) => e.to_string(),
                            _ => unreachable!(),
                        });
                    }
                }
                status["recordsThisRun"] = json!(records);
                if let Ok(mut current) = task.status.lock() {
                    if let Some(previous) = current["assets"].as_object() {
                        for (symbol, value) in previous {
                            if symbols.contains(symbol) && status["assets"][symbol].is_null() {
                                status["assets"][symbol] = value.clone();
                            }
                        }
                    }
                    *current = status.clone();
                }
                if let Ok(dir) = directory(&app) {
                    let _ = tauri::async_runtime::spawn_blocking(move || {
                        if let Err(e) = std::fs::write(
                            dir.join("research-capture-status.json"),
                            status.to_string(),
                        ) {
                            eprintln!("Research capture status failed: {e}");
                        }
                    })
                    .await;
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn bot_context_only_uses_committed_fresh_same_asset_forecasts() {
        let forecast=json!({"version":"log-brownian-rth-arrival-v1","calibrated":false,
            "issuedMs":100000,"returns":60,"probability":0.7,
            "case":{"deadlineMs":300000,"direction":"up","level":{"symbol":"QQQ","horizon":"DAY"}}});
        let status=json!({"assets":{"QQQ":{"recorded":true,"quality":{"phase":"RTH","streaming":true},
            "arrivalBaseline":{"version":"log-brownian-rth-arrival-v1","calibrated":false,"forecasts":[forecast]}}}});
        let context=super::bot_probability_context(&status,"QQQ","CALL",110000);
        assert_eq!(context["state"],"available");
        assert_eq!(context["forecasts"][0]["relationToBot"],"same-direction");
        assert_eq!(context["executionEnabled"],false);
        assert!(context["routeInvalidation"].is_null());
        assert_eq!(super::bot_probability_context(&status,"QQQ","PUT",110000)["forecasts"][0]["relationToBot"],"opposite-direction");
        for variant in 0..7 {
            let mut broken=status.clone();
            let asset=&mut broken["assets"]["QQQ"];
            match variant {
                0=>asset["recorded"]=json!(false),
                1=>asset["quality"]["streaming"]=json!(false),
                2=>asset["quality"]["phase"]=json!("OUTSIDE_REGULAR_SESSION"),
                3=>asset["arrivalBaseline"]["forecasts"][0]["issuedMs"]=json!(110001),
                4=>asset["arrivalBaseline"]["forecasts"][0]["case"]["level"]["symbol"]=json!("SPY"),
                5=>asset["arrivalBaseline"]["forecasts"][0]["probability"]=json!(1.1),
                _=>asset["arrivalBaseline"]["forecasts"][0]["calibrated"]=json!(true),
            }
            assert_eq!(super::bot_probability_context(&broken,"QQQ","CALL",110000)["state"],"waiting");
        }
        assert_eq!(super::bot_probability_context(&status,"QQQ","CALL",220001)["state"],"waiting");
        assert_eq!(super::bot_probability_context(&status,"SPY","CALL",110000)["state"],"waiting");
    }
    use super::*;
    #[test]
    fn saves_a_prospective_forecast_from_full_history_while_raw_bar_window_stays_small(){
        let now=calendar::rfc3339_ms("2026-10-09T16:00:00Z").unwrap();let mut f=frame("QQQ",now);
        f["ticks"]["QQQ"]["symbol"]=json!("QQQ");
        f["chartHistory"]["minutes"]=json!((0..121).map(|i|{let t=now-(121-i)*60_000;let p=if i%2==0{100.1}else{99.9};json!({"timestampMs":t,"open":p,"high":p,"low":p,"close":p,"volume":10})}).collect::<Vec<_>>());
        f["optionSnapshotStatus"]["horizons"]=json!([{"mode":"DAY","observedMs":now,"structure":{"complete":true,"callWall":{"strike":101.},"putWall":{"strike":99.}}}]);
        let row=record("QQQ",f,now);assert_eq!(row["evidence"]["chartHistory"]["minutes"].as_array().unwrap().len(),5);
        let forecasts=row["arrivalBaseline"]["forecasts"].as_array().unwrap();assert_eq!(forecasts.len(),2);assert_eq!(forecasts[0]["issuedMs"],now);assert_eq!(forecasts[0]["calibrated"],false);
        assert!(forecasts[0]["probability"].as_f64().is_some_and(|p|p>0.&&p<1.));assert_eq!(row["predictionEnabled"],false);
    }
    fn at() -> u64 {
        calendar::rfc3339_ms("2026-10-09T14:00:00Z").unwrap()
    }
    fn frame(symbol: &str, now: u64) -> Value {
        json!({"symbol":symbol,"startedMs":1,"publishedMs":now,"stocks":{"phase":"streaming"},"ticks":{symbol:{"price":100,"tradeMs":now-1000}},"chartHistory":{"minutes":[{"timestampMs":now-60000,"open":100,"high":101,"low":99,"close":100,"volume":10}]},"optionSnapshotStatus":{"observedMs":now-1000,"horizons":[]}})
    }
    #[test]
    fn never_borrows_price_from_another_asset() {
        let r = record("SPY", frame("QQQ", at()), at());
        assert_eq!(r["quality"]["price"], "missing");
        assert_eq!(r["quality"]["priceObservationEligible"], false);
        assert!(r["evidence"]["tick"].is_null());
    }
    #[test]
    fn preserves_event_time_and_rejects_stale_republished_price() {
        let now = at();
        let mut f = frame("QQQ", now);
        f["ticks"]["QQQ"]["tradeMs"] = json!(now - 60000);
        let r = record("QQQ", f, now);
        assert_eq!(r["sourcePublishedMs"], now);
        assert_eq!(r["quality"]["price"], "stale");
        assert_eq!(r["evidence"]["tick"]["tradeMs"], now - 60000);
        assert_eq!(r["quality"]["priceObservationEligible"], false);
    }
    #[test]
    fn outside_session_is_not_a_negative_outcome() {
        let now = calendar::rfc3339_ms("2026-10-10T02:00:00Z").unwrap();
        let r = record("QQQ", frame("QQQ", now), now);
        assert_eq!(r["quality"]["phase"], "OUTSIDE_REGULAR_SESSION");
        assert_eq!(r["quality"]["priceObservationEligible"], false);
        assert!(r["outcome"].is_null());
        assert_eq!(r["predictionEnabled"], false);
    }
    #[test]
    fn rejects_future_price_and_duplicate_bars() {
        let now = at();
        let mut f = frame("QQQ", now);
        f["ticks"]["QQQ"]["tradeMs"] = json!(now + 1);
        let b = f["chartHistory"]["minutes"][0].clone();
        f["chartHistory"]["minutes"].as_array_mut().unwrap().push(b);
        let r = record("QQQ", f, now);
        assert_eq!(r["quality"]["price"], "future");
        assert_eq!(r["quality"]["bars"], "duplicate");
    }
    #[test]
    fn keeps_horizons_separate_and_does_not_fabricate_missing_options() {
        let now = at();
        let mut f = frame("QQQ", now);
        f["optionSnapshotStatus"]["horizons"] = json!([{"mode":"DAY","observedMs":now-1000,"expirations":["2026-10-09"],"structure":{"complete":true,"reading":{"direction":"bullish"},"magnet":105,"strikeRows":[{"strike":105,"oi":100}]}},{"mode":"WEEK","observedMs":now-400000,"expirations":["2026-10-16"]}]);
        let structure = f["optionSnapshotStatus"]["horizons"][0]["structure"].clone();
        let r = record("QQQ", f, now);
        assert_eq!(r["evidence"]["optionSnapshotStatus"]["horizons"][0]["structure"], structure);
        assert_eq!(r["evidence"]["optionSnapshotStatus"]["horizons"][0]["reading"], structure["reading"]);
        assert_eq!(
            r["evidence"]["optionSnapshotStatus"]["horizons"][0]["quality"],
            "available"
        );
        assert_eq!(
            r["evidence"]["optionSnapshotStatus"]["horizons"][1]["quality"],
            "stale"
        );
        assert!(r["evidence"]["optionSnapshotStatus"]["horizons"][1]["exposure"].is_null());
    }
    #[test]
    fn watchlist_is_bounded_and_validated() {
        assert_eq!(
            clean(vec![" qqq ".into(), "QQQ".into(), "SPY".into()]).unwrap(),
            vec!["QQQ", "SPY"]
        );
        assert!(clean(vec![]).is_err());
        assert!(clean(vec!["../QQQ".into()]).is_err());
        assert!(clean(vec!["QQQ".into(); 6]).is_err());
    }
}
