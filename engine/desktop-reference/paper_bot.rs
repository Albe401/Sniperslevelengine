//! Single-contract QQQ pilot. New entries require explicit UI arming; recovery never rearms.
use massive_feed::{
    alpaca::{OrderIntent, OrderSide, PaperClient},
    broker_quotes::Quotes,
    live::now_ms,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use snipers_store::broker_orders::BrokerOrders;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::Manager;

fn default_profit_target()->f64 { 10. }
fn default_expiry_mode()->String { "three_days".into() }
#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub expiry: String,
    #[serde(default = "default_expiry_mode")]
    pub expiry_mode: String,
    pub budget: f64,
    #[serde(default = "default_profit_target")]
    pub profit_target_pct: f64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            expiry: String::new(),
            expiry_mode: default_expiry_mode(),
            budget: 1000.,
            profit_target_pct: default_profit_target(),
        }
    }
}
impl Config {
    fn validate(&self) -> Result<(), String> {
        let today = snipers_core::calendar::date_at(now_ms())
            .ok_or("Fecha no disponible")?
            .to_string();
        self.validate_for(&today)
    }
    fn validate_for(&self, today: &str) -> Result<(), String> {
        if snipers_core::calendar::day_bounds(today).is_none()||!self.budget.is_finite()||self.budget < 0.01 || !(self.budget * 5.).is_finite(){return Err("Introduce una cantidad positiva válida por entrada en USD".into());}
        if !self.profit_target_pct.is_finite() || self.profit_target_pct < 10. { return Err("El objetivo de ganancia debe ser un porcentaje válido de 10% en adelante".into()); }
        massive_feed::broker_quotes::expiry_window(today,&self.expiry_mode)?;
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub reserved_premium: f64,
    pub date: String,
    #[serde(default = "one_entry")]
    pub daily_entries: u32,
    #[serde(default)]
    pub completed_ms: u64,
    pub local_id: i64,
    pub symbol: String,
    pub side: String,
    pub phase: String,
    pub buy_id: String,
    pub sell_id: Option<String>,
    pub sequence: u32,
    pub order_ms: u64,
    pub cancel_requested: bool,
    pub close_requested: bool,
    pub reason: String,
    pub last_order: Value,
    #[serde(default)]
    pub strategy: String,
    #[serde(default)]
    pub entry_premium: Option<f64>,
    #[serde(default)]
    pub profit_target_pct: Option<f64>,
    #[serde(default)]
    pub entry_time: Option<String>,
    #[serde(default)]
    pub exit_time: Option<String>,
    #[serde(default)]
    pub exit_premium: Option<f64>,
    #[serde(default)]
    pub exit_trigger: Option<String>,
    #[serde(default)]
    pub mark_bid: Option<f64>,
    #[serde(default)]
    pub mark_ms: Option<u64>,
    #[serde(default)]
    pub management: crate::bot_position_policy::State,
    #[serde(default)]
    pub entry_context: Value,
}
fn one_entry() -> u32 {
    1
}
#[tauri::command]
pub async fn get_contract_sensitivities(
    symbol: String,
) -> Result<snipers_core::theta_vega::Read, String> {
    Quotes::load()?.sensitivities(&symbol).await
}
fn entries_today(session: Option<&Session>, date: &str) -> u32 {
    session
        .filter(|s| s.date == date)
        .map_or(0, |s| s.daily_entries)
}
fn next_entry_number(
    previous: Option<&Session>,
    date: &str,
    signal_id: i64,
) -> Result<u32, String> {
    let used = entries_today(previous, date);
    if signal_id <= 0
        || previous.is_some_and(|s| {
            s.active() || s.local_id == signal_id || signal_id as u64 <= s.completed_ms
        })
    {
        return Err("Esperando una señal nueva posterior al cierre anterior".into());
    }
    used.checked_add(1)
        .ok_or_else(|| "Contador de entradas fuera de rango".into())
}
impl Session {
    fn active(&self) -> bool {
        self.phase != "done"
    }
}
#[derive(Clone, Default)]
struct Control {
    top5: bool,
    qqq_only: bool,
    trackers: std::collections::BTreeMap<String,massive_feed::intraday_strategy::Tracker>,
    buffers: std::collections::BTreeMap<String,massive_feed::intraday_strategy::SnapshotBuffer>,
    smc_last_closed: std::collections::BTreeMap<String,u64>,
    arrival_context_saved: std::collections::BTreeMap<String,String>,
    chart_routes: std::collections::BTreeMap<String,massive_feed::bot_chart_route::Tracker>,
    ema9_rebounds: std::collections::BTreeMap<String,massive_feed::bot_ema9_rebound::Tracker>,
    pullback_learning: std::collections::BTreeMap<String,crate::bot_pullback_learning::State>,
    armed: bool,
    armed_ms: u64,
    armed_date: String,
    config: Config,
    close_requested: bool,
    public: Value,
    tracker: massive_feed::intraday_strategy::Tracker,
}
#[derive(Default)]
pub struct Task {
    control: Arc<Mutex<Control>>,
}
fn path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let d = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Directorio no disponible")?;
    std::fs::create_dir_all(&d).map_err(|_| "No se pudo abrir el diario")?;
    Ok(d.join("alpaca-paper.sqlite"))
}
async fn save(p: PathBuf, s: Option<Session>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let payload = serde_json::to_string(&s).map_err(|_| "Estado del bot inválido")?;
        BrokerOrders::open(&p)
            .and_then(|db| db.save_runtime(&payload))
            .map_err(|_| "No se pudo guardar el bot; ninguna nueva orden es segura")
    })
    .await
    .map_err(|_| "Error del diario del bot".to_owned())?
    .map_err(str::to_owned)
}
async fn audit(p: PathBuf, s: &Session, reading: &Value, action: &str, at: u64) -> Result<(), String> {
    let id=s.buy_id.clone();
    // One holding snapshot per 30 seconds; distinct exit reasons are recorded immediately.
    let key=if action=="hold" {format!("hold:{}",at/30_000)} else {format!("{action}:{}",s.reason)};
    let payload=json!({"audit_version":"decision-evidence-v1","at_ms":at,"action":action,
        "strategy":s.strategy,"session":s,"reading":reading}).to_string();
    tauri::async_runtime::spawn_blocking(move || BrokerOrders::open(&p)
        .and_then(|db|db.record_bot_decision(&id,&key,at,&payload)))
        .await.map_err(|_|"Error del diario de decisiones".to_owned())?
        .map_err(|_|"No se pudo guardar la evidencia de la decisión".to_owned())
}
fn legacy_review(app: &tauri::AppHandle) -> Value {
    let live = app.state::<crate::LiveTask>();
    if live.handle.lock().ok().is_none_or(|h| h.is_none()) {
        return Value::Null;
    }
    live.state
        .lock()
        .ok()
        .and_then(|s| s.clone())
        .and_then(|s| {
            s.lock().ok().map(|s| {
                let fresh = s
                    .ticks
                    .get("QQQ")
                    .and_then(|t| t.trade_ms)
                    .is_some_and(|t| t <= now_ms() && now_ms().saturating_sub(t) <= 5000);
                if s.stocks.phase == "streaming" && fresh {
                    s.decision_review.clone()
                } else {
                    Value::Null
                }
            })
        })
        .unwrap_or(Value::Null)
}
fn review(app:&tauri::AppHandle,preferred:Option<&str>)->Value {
    let live=app.state::<crate::LiveTask>();
    if live.handle.lock().ok().is_none_or(|h|h.is_none()){return Value::Null;}
    let Some(shared)=live.state.lock().ok().and_then(|s|s.clone()) else {return Value::Null};
    let snapshot=massive_feed::live::snapshot(&shared);
    let now=now_ms();
    let task=app.state::<Task>();let Ok(mut c)=task.control.lock() else {return Value::Null};
    let mut symbols=if c.top5 && !c.qqq_only {snapshot.scanner.bot_selection.symbols.clone()}else{vec!["QQQ".into()]};
    if let Some(root)=preferred {if !symbols.iter().any(|s|s==root){symbols.push(root.into());}}
    let armed=c.armed_ms;
    let mut readings=serde_json::Map::new();
    for symbol in &symbols {
        let mut data=if c.top5 || symbol!=&snapshot.symbol {massive_feed::live::bot_frame(&shared,symbol,now)}else{strategy_data(&snapshot)};
        if data["dailyRange"].is_null(){data["dailyRange"]=json!(shared.lock().unwrap().scanner_store.bot_daily_range(symbol,now));}
        data["neuroContext"]=json!(snapshot.neuro_contexts.get(symbol).cloned().unwrap_or_default());
        let route_frame=data.clone();
        let mut reading=c.buffers.entry(symbol.clone()).or_default().read_paper(data,now);
        massive_feed::qqq_component_reading::apply_bot(&snapshot.qqq_report,&mut reading,now);
        apply_entry_clarity(&mut reading,now);
        let route=crate::route_4h::bot_review(&shared,&route_frame,&reading,app,now);
        massive_feed::bot_route_context::attach(&mut reading,&route,now);
        massive_feed::bot_route_context::confirm_entry(&mut reading,now);
        let mut smc_frame=c.buffers.get(symbol).and_then(|b|b.settled_frame(now)).unwrap_or_else(||route_frame.clone());
        let smc_history=shared.lock().unwrap().scanner_store.bot_smc_history(symbol,now);
        if !smc_history.is_empty(){smc_frame["chartHistory"]["minutes"]=json!(smc_history);}
        let baseline=serde_json::to_value(&reading).unwrap_or(Value::Null);
        let smc_context=massive_feed::smc::read(&smc_frame,symbol,now);
        let closed=smc_context["layers"][0]["lastClosed"].as_u64().unwrap_or(0);
        massive_feed::smc_bot::apply(smc_context,&mut reading,now);
        if closed>0&&c.smc_last_closed.get(symbol).copied()!=Some(closed){
            c.smc_last_closed.insert(symbol.clone(),closed);
            if let Ok(directory)=app.path().app_local_data_dir(){
                let record=json!({"version":"smc-live-frame-v1","at":now,"symbol":symbol,"baseline":baseline,"finalReading":reading,"frame":smc_frame,"basis":"live-snapshots-no-orders"});
                let control=task.control.clone();let root=symbol.clone();
                tauri::async_runtime::spawn_blocking(move||{
                    let folder=directory.join("smc-observations");
                    let date=snipers_core::calendar::date_at(now).map(|d|d.to_string()).unwrap_or_default();
                    let result=snipers_store::observation_log::append(&folder.join(format!("smc-live-{date}.jsonl")),&record);
                    let status=match result {
                        Ok(file)=>json!({"version":"smc-recording-v2","symbol":root,"at":now,"recorded":true,"segment":file.file_name().and_then(|n|n.to_str()),"error":null}),
                        Err(error)=>{
                            if let Ok(mut c)=control.lock(){if c.smc_last_closed.get(&root)==Some(&closed){c.smc_last_closed.remove(&root);}}
                            eprintln!("SMC observation write failed for {root}: {error}");
                            json!({"version":"smc-recording-v2","symbol":root,"at":now,"recorded":false,"error":error.to_string()})
                        }
                    };
                    if let Err(error)=std::fs::write(directory.join(format!("smc-recording-{root}-status.json")),status.to_string()){eprintln!("SMC recording status write failed: {error}");}
                });
            }
        }
        let held=c.public["sessions"].as_array().and_then(|rows|rows.iter().find(|v|v["phase"]!="done"&&contract_root(v["symbol"].as_str().unwrap_or(""))==*symbol)).and_then(|v|v["side"].as_str()).map(str::to_owned);
        if held.is_none() {
            c.chart_routes.entry(symbol.clone()).or_default().apply(&route_frame,&mut reading,now);
            c.ema9_rebounds.entry(symbol.clone()).or_default().apply(&route_frame,&mut reading,now);
            apply_entry_clarity(&mut reading,now);
        }
        // Certified option activity is read natively, after all strategy filters.
        // It is carried into the report, entry context and existing decision diary.
        crate::bot_option_activity::attach(&crate::option_research::bot_status(app),&route_frame,&mut reading,now);
        crate::bot_asset_history::attach(&mut reading,now);
        if let Some(record)=c.pullback_learning.entry(symbol.clone()).or_default().update(&route_frame,&mut reading,now){
            if let Ok(directory)=app.path().app_local_data_dir(){
                let root=symbol.clone();let date=snipers_core::calendar::date_at(now).map(|d|d.to_string()).unwrap_or_default();
                tauri::async_runtime::spawn_blocking(move||{
                    if let Err(e)=snipers_store::observation_log::append(&directory.join("bot-pullback-learning").join(format!("pullback-{date}-{root}.jsonl")),&record){eprintln!("Pullback learning journal failed for {root}: {e}");}
                });
            }
        }
        let mut report=c.trackers.entry(symbol.clone()).or_default().update(&reading,armed,held.as_deref());
        // Attach AFTER strategy evaluation: experimental context cannot vote on execution.
        report["reading"]["quick_context"]["arrivalBaseline"] = crate::research_observations::bot_context(app,symbol,&reading.entry_side,now);
        let context=&report["reading"]["quick_context"]["arrivalBaseline"];
        if context["state"]=="available" {
            let key=json!([context["forecasts"],reading.entry_side,reading.entry_basis]).to_string();
            if c.arrival_context_saved.get(symbol)!=Some(&key) {
                if let Ok(directory)=app.path().app_local_data_dir() {
                    c.arrival_context_saved.insert(symbol.clone(),key.clone());
                    let date=snipers_core::calendar::date_at(now).map(|d|d.to_string()).unwrap_or_default();
                    let record=json!({"version":"bot-arrival-comparison-v1","atMs":now,"symbol":symbol,
                        "context":context,"botReading":report["reading"],"executionEnabled":false});
                    let control=task.control.clone();let root=symbol.clone();
                    tauri::async_runtime::spawn_blocking(move||{
                        if let Err(error)=snipers_store::observation_log::append(&directory.join("bot-arrival-observations").join(format!("bot-arrival-{date}-{root}.jsonl")),&record) {
                            if let Ok(mut c)=control.lock(){if c.arrival_context_saved.get(&root)==Some(&key){c.arrival_context_saved.remove(&root);}}
                            eprintln!("Bot arrival comparison write failed for {root}: {error}");
                        }
                    });
                }
            }
        }
        readings.insert(symbol.clone(),report);
    }
    let chosen=preferred.and_then(|root|readings.get(root)).or_else(||readings.values().find(|v|!v["position"].is_null())).or_else(||readings.values().next()).cloned().unwrap_or(Value::Null);
    c.public["readings"]=json!(readings);c.public["universe"]=json!(snapshot.scanner.bot_selection);
    c.public["strategy_reading"]=chosen.clone();chosen
}
fn contract_root(contract:&str)->String {snipers_core::options::underlying(&format!("O:{}",contract.strip_prefix("O:").unwrap_or(contract))).unwrap_or("").into()}
fn entry_asset_allowed(c:&Control,root:&str)->bool {
    if c.qqq_only || !c.top5 {root=="QQQ"}else{c.public["universe"]["symbols"].as_array().is_some_and(|rows|rows.iter().any(|v|v==root))}
}
fn clear_entry(r:&massive_feed::intraday_strategy::Reading,now:u64)->bool {
    let sign=match r.entry_side.as_str(){"CALL"=>1.,"PUT"=>-1.,_=>return false};
    if !r.ready||!r.eligible||r.coverage<65.||!r.coverage.is_finite()||!r.flow_one.is_finite()||!r.flow_five.is_finite()||r.flow_one*sign<0.15||r.flow_five*sign<=0.||r.quick_context["cvd"]["status"]=="contradicts"{return false;}
    let Some((last,prior))=r.closed_bars.last().zip(r.closed_bars.iter().rev().nth(1)) else{return false};
    let Some(t)=last["t"].as_u64() else{return false};let Some(p)=prior["t"].as_u64() else{return false};
    if t.checked_sub(p)!=Some(120000)||t.checked_add(120000).is_none_or(|end|end>now||now-end>120000){return false;}
    let value=|v:&Value|v.as_f64().filter(|n|n.is_finite()&&*n>0.);
    let Some((close,previous))=value(&last["close"]).zip(value(&prior["close"]))else{return false};
    let Some(target)=r.next.filter(|n|n.is_finite()&&*n>0.)else{return false};
    let Some(stop)=value(&last[if sign>0.{"low"}else{"high"}])else{return false};
    let risk=(r.spot-stop)*sign;let room=(target-r.spot)*sign;
    r.spot.is_finite()&&r.spot>0.&&(close-previous)*sign>0.&&(r.spot-close)*sign>=0.&&risk>0.&&room/risk>=1.8
}
fn apply_entry_clarity(r:&mut massive_feed::intraday_strategy::Reading,now:u64){
    let clear=clear_entry(r,now);
    if r.eligible&&!clear{r.blockers.push("Esperando entrada clara: dos cierres consecutivos de 2m, flujo 1m/5m alineado y recorrido estructural suficiente".into());}
    let ema_opposes=massive_feed::bot_ema::opposes(&r.ema,&r.entry_side,r.spot);
    if r.eligible&&ema_opposes{r.blockers.push("EMA 9/20/50 de 1m contradicen la entrada".into());}
    r.eligible &= clear && !ema_opposes;
}
#[tauri::command]
pub fn set_paper_bot_asset_mode(task:tauri::State<'_,Task>,mode:String)->Result<(),String>{
    let only=match mode.as_str(){"qqq"=>true,"five"=>false,_=>return Err("Modo no válido".into())};
    let mut c=task.control.lock().map_err(|_|"Bot ocupado")?;
    if c.qqq_only!=only {c.qqq_only=only;c.trackers.clear();c.buffers.clear();if c.armed {c.armed_ms=now_ms();}}
    Ok(())
}
#[tauri::command]
pub fn connect_top_five_bot(app:tauri::AppHandle)->Result<(),String>{
    let live=app.state::<crate::LiveTask>();
    let shared=live.state.lock().map_err(|_|"Conexión ocupada")?.clone().ok_or("Conecta Massive primero")?;
    if live.handle.lock().map_err(|_|"Conexión ocupada")?.is_none(){return Err("Conecta Massive primero".into());}
    app.state::<Task>().control.lock().map_err(|_|"Bot ocupado")?.top5=true;
    shared.lock().map_err(|_|"Estado ocupado")?.bot_enabled=true;
    Ok(())
}
pub fn background_enabled(app:&tauri::AppHandle)->bool {app.state::<Task>().control.lock().map(|c|c.top5).unwrap_or(false)}

#[cfg(test)]
mod asset_mode_tests {
    use super::*;
    #[test]
    fn mode_filters_entries_even_when_qqq_is_not_in_the_frozen_five() {
        let mut c=Control{top5:true,public:json!({"universe":{"symbols":["NVDA","AAPL","TSLA","SPY","AMZN"]}}),..Default::default()};
        assert!(entry_asset_allowed(&c,"NVDA"));assert!(!entry_asset_allowed(&c,"QQQ"));
        c.qqq_only=true;assert!(entry_asset_allowed(&c,"QQQ"));assert!(!entry_asset_allowed(&c,"NVDA"));
        c.top5=false;assert!(entry_asset_allowed(&c,"QQQ"));assert!(!entry_asset_allowed(&c,"TSLA"));
    }
}
#[cfg(test)]
mod clarity_tests {
    use super::*;
    fn sample(call:bool,t:u64)->massive_feed::intraday_strategy::Reading{
        massive_feed::intraday_strategy::Reading{ready:true,eligible:true,management_ready:true,coverage:80.,entry_side:if call{"CALL"}else{"PUT"}.into(),flow_one:if call{0.3}else{-0.3},flow_five:if call{0.2}else{-0.2},spot:100.,next:Some(if call{104.}else{96.}),closed_bars:vec![json!({"t":t,"close":if call{98.}else{102.}}),json!({"t":t+120000,"close":if call{99.}else{101.},"low":98.,"high":102.})],..Default::default()}
    }
    #[test]fn clarity_can_arrive_early_but_time_alone_never_authorizes(){
        let open=snipers_core::calendar::legacy_eastern_ms("2026-10-02 09:30:00").unwrap();
        for call in [true,false]{assert!(clear_entry(&sample(call,open),open+240000));let mut r=sample(call,open+1800000);r.flow_five=0.;apply_entry_clarity(&mut r,open+2040000);assert!(!r.eligible);assert!(r.management_ready&&r.ready);}
    }
    #[test]fn contradictions_gaps_unclosed_bars_and_no_room_keep_waiting(){
        let t=1000000;for i in 0..7{let mut r=sample(true,t);match i{0=>r.quick_context=json!({"cvd":{"status":"contradicts"}}),1=>r.flow_one=-0.2,2=>r.closed_bars[0]["t"]=json!(t-120000),3=>r.next=Some(101.),4=>r.closed_bars[1]["close"]=json!(97.),5=>r.flow_five=f64::NAN,_=>r.coverage=64.};assert!(!clear_entry(&r,t+240000),"case {i}");}assert!(!clear_entry(&sample(true,t),t+239999));
    }
    #[test]fn clarity_check_is_lightweight(){
        let r=sample(true,1000000);let start=std::time::Instant::now();for _ in 0..25000{assert!(clear_entry(&r,1240000));}let ms=start.elapsed().as_secs_f64()*1000./5000.;println!("entry clarity, five assets: {ms:.4} ms/cycle");assert!(ms<10.);
    }
    #[test]fn verified_ema_opposition_blocks_new_entry_but_not_management(){
        let t=1_000_000;let mut r=sample(true,t);
        r.ema=massive_feed::bot_ema::EmaRead{available:true,timeframe_minutes:1,ema9:Some(101.),ema20:Some(102.),ema50:Some(103.),ema200:Some(104.),..Default::default()};
        apply_entry_clarity(&mut r,t+240_000);
        assert!(!r.eligible);assert!(r.management_ready);
        assert!(r.blockers.iter().any(|b|b.contains("EMA")));
        r.ema.available=false;r.eligible=true;r.blockers.clear();
        apply_entry_clarity(&mut r,t+240_000);
        assert!(r.eligible);
    }
}
fn strategy_data(snapshot: &massive_feed::live::LiveState) -> Value {
    massive_feed::intraday_strategy::snapshot_input(snapshot)
}
fn recent_review(v: &Value, now: u64) -> bool {
    v["reviewedMs"]
        .as_u64()
        .is_some_and(|t| t <= now && now - t <= 5000)
}
fn new_signal(v: &Value, armed_ms: u64, now: u64) -> Option<(i64, String, f64)> {
    if !v["symbol"].as_str().is_some_and(|s|snipers_core::universe::stock_asset(s).is_some()) || v["entriesEnabled"] != true || !recent_review(v, now) {
        return None;
    }
    let p = &v["position"];
    let t = &p["trade"];
    let opened = t["openedMs"].as_u64()?;
    if opened < armed_ms
        || opened > now
        || now - opened > 10000
        || !t["closedMs"].is_null()
        || !matches!(t["side"].as_str(), Some("CALL" | "PUT"))
    {
        return None;
    }
    Some((
        p["tradeId"].as_i64()?,
        t["side"].as_str()?.into(),
        t["entry"].as_f64()?,
    ))
}
/// Gross gain on the actual fill, rounded up to an executable cent.
fn take_profit_reason(s: &Session, now: u64) -> Option<String> {
    if s.phase != "open" { return None; }
    let entry = s.entry_premium.filter(|n| n.is_finite() && *n > 0.)?;
    let bid = s.mark_bid.filter(|n| n.is_finite() && *n > 0.)?;
    let quote_ms = s.mark_ms?;
    if quote_ms > now || now - quote_ms > 5000 { return None; }
    let pct = s.profit_target_pct.or_else(|| (s.strategy != massive_feed::intraday_strategy::VERSION).then_some(10.)).filter(|v| v.is_finite() && *v >= 10.)?;
    let target_cents = (entry * (100. + pct) - 1e-9).ceil();
    if !target_cents.is_finite() { return None; }
    if (bid * 100. + 1e-9).floor() < target_cents { return None; }
    Some(format!("Take profit +{pct}% bruto: bid ${bid:.2} · objetivo ${:.2}", target_cents / 100.))
}
fn exit_reason(s: &Session, v: &Value, now: u64, close_ms: u64) -> Option<String> {
    if s.close_requested {
        return Some("Cierre solicitado".into());
    }
    if now >= close_ms.saturating_sub(15 * 60 * 1000) {
        return Some("Salida preventiva antes del cierre".into());
    }
    if !recent_review(v, now) {
        return Some("Motor o datos desconectados".into());
    }
    if s.strategy == massive_feed::intraday_strategy::VERSION {
        if v["reading"]["ready"] != true {
            return Some("Lectura intradía sin evidencia vigente".into());
        }
        if v["oppositionExit"] == true {
            return Some("Fuerza contraria confirmada por flujo y precio".into());
        }
        return None;
    }
    let current = &v["position"];
    if current["tradeId"].as_i64() == Some(s.local_id)
        && current["trade"]["closedMs"].as_u64().is_some()
    {
        return Some("Salida indicada por el motor".into());
    }
    if let Some(rows) = v["positionHistory"].as_array() {
        if rows.iter().any(|p| {
            p["id"].as_i64() == Some(s.local_id) && p["trade"]["closedMs"].as_u64().is_some()
        }) {
            return Some("Salida indicada por el motor".into());
        }
    }
    if current["tradeId"].as_i64() != Some(s.local_id) {
        return Some("Se perdió la posición de referencia del motor".into());
    }
    None
}
fn terminal(status: &str) -> bool {
    matches!(status, "filled" | "canceled" | "expired" | "rejected")
}
fn apply_order(s: &mut Session, order: &Value) -> Result<bool, String> {
    let expected = if s.phase == "buy_pending" {
        s.buy_id.as_str()
    } else {
        s.sell_id.as_deref().ok_or("Falta orden de salida")?
    };
    if order["client_order_id"].as_str() != Some(expected) {
        return Err("La respuesta no corresponde a la orden del bot".into());
    }
    let status = order["status"]
        .as_str()
        .ok_or("Estado de orden no disponible")?;
    s.last_order = order.clone();
    if !terminal(status) {
        return Ok(false);
    }
    let filled = order["filled_qty"]
        .as_str()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|q| *q <= 1)
        .ok_or("Cantidad ejecutada inesperada: revisar en Alpaca")?;
    if s.phase == "buy_pending" {
        if filled == 1 && s.strategy == massive_feed::intraday_strategy::VERSION {
            s.entry_premium = Some(
                order["filled_avg_price"]
                    .as_str()
                    .and_then(|p| p.parse::<f64>().ok())
                    .filter(|p| p.is_finite() && *p > 0.)
                    .ok_or("Falta prima ejecutada: reconciliar antes de gestionar la posición")?,
            );
        }
        if filled == 1 {
            s.entry_time = order["filled_at"].as_str().map(str::to_owned);
            s.management.entered_ms = s
                .entry_time
                .as_deref()
                .and_then(snipers_core::calendar::rfc3339_ms)
                .unwrap_or(s.order_ms);
        }
        s.phase = if filled == 1 { "open" } else { "done" }.into();
        s.reason = if filled == 1 {
            "Entrada ejecutada"
        } else {
            "Entrada sin ejecución"
        }
        .into();
    } else if filled == 1 {
        s.exit_time = order["filled_at"].as_str().map(str::to_owned);
        s.exit_premium = order["filled_avg_price"]
            .as_str()
            .and_then(|p| p.parse::<f64>().ok())
            .filter(|p| p.is_finite() && *p > 0.);
        s.phase = "done".into();
        s.reason = "Salida ejecutada".into();
    } else {
        s.phase = "open".into();
        s.close_requested = true;
        s.reason = "Salida pendiente: nuevo límite tras cancelación confirmada".into();
    }
    if s.phase == "done" {
        s.completed_ms = now_ms();
    }
    s.cancel_requested = false;
    Ok(true)
}

#[tauri::command]
pub async fn get_paper_bot_decisions(app: tauri::AppHandle, buy_id: String) -> Result<Vec<Value>, String> {
    if buy_id.is_empty()||buy_id.len()>200{return Err("Identificador de operación inválido".into());}
    let p=path(&app)?;
    tauri::async_runtime::spawn_blocking(move ||BrokerOrders::open(&p).and_then(|db|db.bot_decisions(&buy_id)))
        .await.map_err(|_|"Error del diario de decisiones".to_owned())?
        .map_err(|_|"No se pudo leer el diario de decisiones".to_owned())
}
#[tauri::command]
pub async fn get_paper_bot_history(app: tauri::AppHandle) -> Result<Vec<Value>, String> {
    let p = path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        BrokerOrders::open(&p)
            .and_then(|db| db.bot_history())
            .map_err(|_| "No se pudo leer el registro de operaciones".to_owned())
    })
    .await
    .map_err(|_| "Registro no disponible".to_owned())?
}
#[tauri::command]
pub fn get_paper_bot_state(task: tauri::State<'_, Task>) -> Result<Value, String> {
    let c = task.control.lock().map_err(|_| "Bot no disponible")?;
    let mut v = c.public.clone();
    if !v.is_object() {
        v = json!({});
    }
    v["top5_connected"]=c.top5.into();
    v["asset_mode"]=json!(if c.qqq_only {"qqq"}else{"five"});
    v["total_budget"]=json!(c.config.budget * 5.);v["max_positions"]=json!(5);
    v["armed"] = c.armed.into();
    v["armed_date"] = c.armed_date.clone().into();
    v["config"] = json!(c.config);
    v["environment"] = "paper".into();
    Ok(v)
}
#[tauri::command]
pub async fn arm_paper_bot(app: tauri::AppHandle, config: Config) -> Result<(), String> {
    let client = tauri::async_runtime::spawn_blocking(PaperClient::load)
        .await
        .map_err(|_| "Credenciales no disponibles")??;
    let s = client.snapshot().await?;
    if !massive_feed::paper_readiness::account_enabled(&s["account"]) {
        return Err("Cuenta Paper o sesión no disponible para opciones".into());
    }
    let target_ms = if s["clock"]["is_open"] == true {
        now_ms()
    } else {
        s["clock"]["next_open"]
            .as_str()
            .and_then(snipers_core::calendar::rfc3339_ms)
            .ok_or("Próxima sesión no disponible")?
    };
    let target_date = snipers_core::calendar::date_at(target_ms)
        .ok_or("Fecha de sesión no disponible")?
        .to_string();
    config.validate_for(&target_date)?;
    // Arming authorizes monitoring. Every entry still requires a fresh, eligible
    // reading and two new observations in tick(); missing data never places orders.
    let open_orders = client.open_orders().await?;
    let prior=app.state::<Task>().control.lock().map_err(|_|"Bot ocupado")?.public["sessions"].as_array().cloned().unwrap_or_default();
    if s["positions"].as_array().is_none_or(|rows|rows.iter().any(|p|!prior.iter().any(|v|v["phase"]!="done"&&v["symbol"]==p["symbol"])))||open_orders.as_array().is_none_or(|rows|rows.iter().any(|o|!prior.iter().any(|v|v["buy_id"]==o["client_order_id"]||v["sell_id"].is_string()&&v["sell_id"]==o["client_order_id"]))) {return Err("La cuenta contiene posiciones u órdenes ajenas al controlador".into());}
    let task = app.state::<Task>();
    let mut c = task.control.lock().map_err(|_| "Bot ocupado")?;
    if c.public["ready"] != true {
        return Err("Espera a que el controlador termine de recuperar el diario".into());
    }
    c.config = config;
    c.armed = true;
    c.armed_ms = target_ms;
    c.armed_date = target_date;
    c.tracker = Default::default();
    c.trackers.clear();c.buffers.clear();
    c.close_requested = false;
    Ok(())
}
#[tauri::command]
pub fn pause_paper_bot(task: tauri::State<'_, Task>) -> Result<(), String> {
    task.control.lock().map_err(|_| "Bot ocupado")?.armed = false;
    Ok(())
}
#[tauri::command]
pub fn close_paper_bot(task: tauri::State<'_, Task>) -> Result<(), String> {
    let mut c = task.control.lock().map_err(|_| "Bot ocupado")?;
    c.armed = false;
    c.close_requested = true;
    Ok(())
}
#[tauri::command]
pub async fn check_paper_bot() -> Result<Value, String> {
    let client = tauri::async_runtime::spawn_blocking(PaperClient::load)
        .await
        .map_err(|_| "Credenciales no disponibles")??;
    massive_feed::paper_readiness::check(&client).await
}
pub fn start(app: tauri::AppHandle) {
    let neuro_app=app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let _=crate::neuro_trader::open_neuro_trader(neuro_app.clone()).await;
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
    let analyst=app.clone();
    tauri::async_runtime::spawn(async move {loop {let _=review(&analyst,None);tokio::time::sleep(Duration::from_secs(1)).await;}});
    let control = app.state::<Task>().control.clone();
    tauri::async_runtime::spawn(async move {
        let result = run(app, control.clone()).await;
        if let Err(e) = result {
            if let Ok(mut c) = control.lock() {
                c.armed = false;
                c.public["ready"] = false.into();
                c.public["error"] = e.into();
            }
        }
    });
}
async fn run(app: tauri::AppHandle, control: Arc<Mutex<Control>>) -> Result<(), String> {
    let p = path(&app)?;
    let load_path = p.clone();
    let data = tauri::async_runtime::spawn_blocking(move || {
        BrokerOrders::open(&load_path).and_then(|db| db.load_runtime())
    })
    .await
    .map_err(|_| "Error recuperando el bot")?
    .map_err(|_| "Diario no disponible")?;
    let asset_path=p.clone();
    let stored=tauri::async_runtime::spawn_blocking(move||BrokerOrders::open(&asset_path).and_then(|db|db.load_assets())).await.map_err(|_|"Recuperación fallida")?.map_err(|_|"No se pudo leer la cartera")?;
    let mut sessions:std::collections::BTreeMap<String,Option<Session>>=std::collections::BTreeMap::new();
    if stored.is_empty(){if let Some(payload)=data {let old:Option<Session>=serde_json::from_str(&payload).map_err(|_|"Diario inválido")?;if let Some(s)=old {sessions.insert(contract_root(&s.symbol),Some(s));}}}
    else {for payload in stored {let s:Session=serde_json::from_str(&payload).map_err(|_|"Cartera inválida")?;sessions.insert(contract_root(&s.symbol),Some(s));}}
    let mut clients = None;
    let mut connection_error=String::from("Conexiones del bot pendientes de comprobar");
    loop {
        let c = control.lock().map_err(|_| "Bot ocupado")?.clone();
        if let Some(shared)=app.state::<crate::LiveTask>().state.lock().ok().and_then(|v|v.clone()) {
            let mut state=shared.lock().unwrap();state.bot_enabled=c.top5;state.bot_qqq_only=c.qqq_only;
            state.bot_targets=if !c.top5{vec![]}else if c.qqq_only{vec!["QQQ".into()]}else{c.public["universe"]["symbols"].as_array().into_iter().flatten().filter_map(|v|v.as_str().map(str::to_owned)).take(5).collect()};
            let targets=state.bot_targets.clone();let held=state.bot_held.clone();let components=state.qqq_components.clone();
            state.qqq_bot_demand=massive_feed::qqq_component_reading::bot_demand(c.armed,c.qqq_only,c.top5,&targets,&held);
            let research=state.research_symbols.clone();
            state.bot_cvd_engines.retain(|root,_|targets.contains(root)||held.contains(root)||components.contains(root)||research.contains(root));
            state.bot_held=sessions.iter().filter(|(_,s)|s.as_ref().is_some_and(Session::active)).map(|(root,_)|root.clone()).collect();
        }
        let _=review(&app,None);
        let universe=control.lock().map_err(|_|"Bot ocupado")?.public["universe"]["symbols"].as_array().cloned().unwrap_or_default();
        let mut roots=if c.top5 && !c.qqq_only {universe.iter().filter_map(|v|v.as_str().map(str::to_owned)).collect::<Vec<_>>()}else{vec!["QQQ".into()]};
        for (root,s) in &sessions {if s.as_ref().is_some_and(Session::active)&&!roots.contains(root){roots.push(root.clone());}}
        // Existing positions are serviced before scanning new opportunities.
        roots.sort_by_key(|r|!sessions.get(r).and_then(|s|s.as_ref()).is_some_and(Session::active));
        let mut details=serde_json::Map::new();let mut errors=serde_json::Map::new();
        for root in roots {
            let held=sessions.get(&root).and_then(|s|s.as_ref()).is_some_and(Session::active);
            if !c.armed&&!held {continue;}
            if clients.is_none(){
                match tauri::async_runtime::spawn_blocking(||{
                    let broker=PaperClient::load().map_err(|e|format!("Alpaca Paper: {e}"))?;
                    let quotes=Quotes::load().map_err(|e|format!("Cotizaciones Massive: {e}"))?;
                    Ok::<_,String>((broker,quotes))
                }).await {
                    Ok(Ok(value))=>{clients=Some(value);connection_error.clear();},
                    Ok(Err(error))=>connection_error=error,
                    Err(_)=>connection_error="No se pudo completar la carga local de conexiones del bot".into(),
                }
            }
            if let Some((broker,quotes))=&clients {
                let portfolio:Vec<Session>=sessions.values().filter_map(Clone::clone).collect();
                let session=sessions.entry(root.clone()).or_default();
                match tick(&app,&p,&control,&c,session,broker,quotes,&root,&portfolio).await {
                    Ok(detail)=>{details.insert(root.clone(),json!(detail));},Err(e)=>{errors.insert(root.clone(),json!(e));}
                }
            } else {errors.insert(root.clone(),json!(format!("Conexiones del bot no disponibles · {connection_error}")));}
            control.lock().map_err(|_|"Bot ocupado")?.public["sessions"]=json!(sessions.values().filter_map(|s|s.as_ref()).collect::<Vec<_>>());
        }
        {
            let mut c=control.lock().map_err(|_|"Bot ocupado")?;
            c.public["ready"]=json!(true);c.public["detail"]=json!(if c.armed {"Cartera Paper · analizando activos en segundo plano"}else{"Entradas pausadas · gestión de posiciones existentes activa"});
            c.public["asset_status"]=json!(details);c.public["asset_errors"]=json!(errors);c.public["error"]=Value::Null;
            let rows:Vec<_>=sessions.values().filter_map(|s|s.as_ref()).collect();
            c.public["sessions"]=json!(rows);c.public["session"]=json!(rows.iter().find(|s|s.active()).or_else(||rows.first()));c.public["updated_ms"]=json!(now_ms());
            c.public["reserved_total"]=json!(rows.iter().filter(|s|s.active()).map(|s|reserved_cost(s)).sum::<f64>());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
fn prepared_signal_matches(before:&Value,after:&Value,root:&str,side:&str,now:u64)->bool {
    recent_review(after,now)&&after["symbol"]==root&&after["reading"]["ready"]==true&&after["reading"]["eligible"]==true
        &&after["reading"]["entry_side"]==side&&after["reading"]["entry_basis"]==before["reading"]["entry_basis"]
        &&after["reading"]["next"]==before["reading"]["next"]
        &&(before["reading"]["entry_basis"]!="ema9-rebound" ||
            after["reading"]["quick_context"]["ema9Rebound"]["confirmed"]==true
            &&after["reading"]["quick_context"]["ema9Rebound"]["key"]==before["reading"]["quick_context"]["ema9Rebound"]["key"])
        &&(before["reading"]["quick_context"]["chartRoute"]["confirmed"]!=true ||
            after["reading"]["quick_context"]["chartRoute"]["confirmed"]==true
            &&after["reading"]["quick_context"]["chartRoute"]["key"]==before["reading"]["quick_context"]["chartRoute"]["key"])
        &&(before["reading"]["entry_basis"]!="magnet-wall"||after["reading"]["structural_entry"]["key"]==before["reading"]["structural_entry"]["key"])
        &&(before["reading"]["entry_basis"]!="vwap-rebound"||after["reading"]["closed_bars"].as_array().and_then(|b|b.last()).map(|b|&b["t"])==before["reading"]["closed_bars"].as_array().and_then(|b|b.last()).map(|b|&b["t"]))
        &&after["position"].is_object()
}
fn reserved_cost(s:&Session)->f64 {
    let declared=s.reserved_premium.max(s.entry_premium.unwrap_or(0.)*100.);
    if declared.is_finite()&&declared>0.{return declared;}
    s.last_order["limit_price"].as_str().and_then(|p|p.parse::<f64>().ok()).filter(|p|p.is_finite()&&*p>0.).map(|p|p*100.).unwrap_or(f64::INFINITY)
}
fn remaining_budget(sessions:&[Session],per_entry:f64)->f64 {
    if !per_entry.is_finite()||per_entry<=0.{return 0.;}
    if !(per_entry*5.).is_finite(){return 0.;}
    (per_entry*5.-sessions.iter().filter(|s|s.active()).map(reserved_cost).sum::<f64>()).max(0.).min(per_entry)
}
fn validate_portfolio(sessions:&[Session],positions:&Value,orders:&Value,root:&str)->Result<(),String>{
    let active:Vec<_>=sessions.iter().filter(|s|s.active()).collect();
    if active.len()>=5 || active.iter().any(|s|contract_root(&s.symbol)==root){return Err("Máximo cinco posiciones y una por activo".into());}
    let positions=positions.as_array().ok_or("Posiciones no verificadas")?;
    let orders=orders.as_array().ok_or("Órdenes no verificadas")?;
    if positions.iter().any(|p|!active.iter().any(|s|p["symbol"]==s.symbol)||p["side"]!="long"||p["qty"]!="1")
        ||orders.iter().any(|o|!active.iter().any(|s|o["client_order_id"]==s.buy_id||s.sell_id.as_ref().is_some_and(|id|o["client_order_id"]==*id))){return Err("Posición u orden externa al controlador: revisar cuenta".into());}
    Ok(())
}
async fn tick(
    app: &tauri::AppHandle,
    p: &std::path::Path,
    control: &Arc<Mutex<Control>>,
    c: &Control,
    session: &mut Option<Session>,
    broker: &PaperClient,
    quotes: &Quotes,
    root: &str,
    portfolio: &[Session],
) -> Result<String, String> {
    let mut v = if session
        .as_ref()
        .is_some_and(|s| s.active() && s.strategy != massive_feed::intraday_strategy::VERSION)
    {
        legacy_review(app)
    } else {
        review(app,Some(root))
    };
    let now = now_ms();
    if let Some(s) = session.as_mut().filter(|s| s.active()) {
        if c.close_requested {
            s.close_requested = true;
        }
        if s.phase == "buy_pending" || s.phase == "sell_pending" {
            if s.phase == "buy_pending" && exit_reason(s, &v, now, u64::MAX).is_some() {
                s.close_requested = true;
            }
            let id = if s.phase == "buy_pending" {
                s.buy_id.clone()
            } else {
                s.sell_id.clone().ok_or("Falta ID de salida")?
            };
            let journal_path = p.to_owned();
            let check_id = id.clone();
            let not_sent = tauri::async_runtime::spawn_blocking(move || {
                BrokerOrders::open(&journal_path).and_then(|db| db.was_not_submitted(&check_id))
            })
            .await
            .map_err(|_| "No se pudo verificar el envío")?
            .map_err(|_| "Diario no disponible para recuperación")?;
            if not_sent {
                if s.phase == "buy_pending" {
                    s.phase = "done".into();
                    s.completed_ms = now;
                    s.reason =
                        "Entrada no enviada: falló la verificación previa; esperar señal nueva"
                            .into();
                    let mut latest = control.lock().map_err(|_| "Bot ocupado")?;
                    latest.trackers.remove(root);
                } else {
                    s.phase = "open".into();
                    s.close_requested = true;
                    s.reason =
                        "Salida no enviada: actualizar cotización y preparar otro intento".into();
                }
                save(p.to_owned(), Some(s.clone())).await?;
                return Ok(s.reason.clone());
            }
            let order = broker.order(&id).await?;
            let status = order["status"]
                .as_str()
                .ok_or("Estado de orden no disponible")?;
            if apply_order(s, &order)? {
                save(p.to_owned(), Some(s.clone())).await?;
                if !s.active() {
                    let mut latest = control.lock().map_err(|_| "Bot ocupado")?;
                    latest.trackers.remove(root);
                }
                return Ok(s.reason.clone());
            }
            if (now.saturating_sub(s.order_ms) > 15000
                || s.close_requested
                || !c.armed && s.phase == "buy_pending")
                && !s.cancel_requested
            {
                broker.cancel(&id).await?;
                s.cancel_requested = true;
                save(p.to_owned(), Some(s.clone())).await?;
            }
            return Ok(format!("{} · {}", s.phase, status));
        }
        if s.phase == "open" {
            let clock = broker.clock().await?;
            let close = clock["next_close"]
                .as_str()
                .and_then(snipers_core::calendar::rfc3339_ms)
                .ok_or("Hora de cierre no disponible")?;
            let open = clock["is_open"] == true;
            if open && !s.close_requested {
                match quotes.quote(&s.symbol).await {
                    Ok(q) if q.validate(now_ms(), false, 0.).is_ok() => {
                        s.mark_bid = Some(q.bid); s.mark_ms = Some(q.timestamp_ms);
                    }
                    _ => { s.mark_bid = None; s.mark_ms = None; }
                }
                // Re-read this underlying after REST latency, never justify holding with stale data.
                if s.strategy == massive_feed::intraday_strategy::VERSION { v = review(app, Some(root)); }
                v["holding"] = json!({"entry_premium":s.entry_premium,"bid":s.mark_bid,"quote_ms":s.mark_ms,"target":s.entry_context["next"]});
            }
            let now = now_ms();
            let before =
                serde_json::to_value(&s.management).map_err(|_| "Estado de gestión inválido")?;
            let reason = if s.close_requested {
                Some(
                    s.exit_trigger
                        .clone()
                        .unwrap_or_else(|| "Cierre solicitado".into()),
                )
            } else if open
                && now >= close.saturating_sub(crate::bot_position_policy::CLOSE_BUFFER_MS)
                && crate::bot_position_policy::expires_today_or_earlier(&s.symbol, now)
            {
                Some("Cierre antes del vencimiento: no conservar durante la noche".into())
            } else if let Some(profit) = take_profit_reason(s, now).filter(|_| open) {
                Some(profit)
            } else if s.strategy == massive_feed::intraday_strategy::VERSION {
                s.management.evaluate(&v, &s.side, now, close, open)
            } else {
                exit_reason(s, &v, now, close)
            };
            // Persist a decision BEFORE requesting quotes: a network error cannot erase it.
            if let Some(reason) = &reason {
                s.close_requested = true;
                s.exit_trigger.get_or_insert_with(|| reason.clone());
                s.reason = reason.clone();
                save(p.to_owned(), Some(s.clone())).await?;
            } else if before
                != serde_json::to_value(&s.management).map_err(|_| "Estado de gestión inválido")?
            {
                s.reason = s.management.assessment["reason"]
                    .as_str()
                    .unwrap_or("Evaluando posición")
                    .into();
                save(p.to_owned(), Some(s.clone())).await?;
            }
            // An audit failure must never suppress an already requested protective exit.
            if let Err(error)=audit(p.to_owned(),s,&v,if reason.is_some(){"exit"}else{"hold"},now).await {
                eprintln!("Paper decision audit: {error}");
            }
            if !open {
                return Ok(if reason.is_some() {
                    "Salida pendiente: mercado cerrado; reintentar al abrir"
                } else {
                    "Mercado cerrado: posición bajo gestión; reevaluar al abrir"
                }
                .into());
            }
            if reason.is_some() {
                let positions = broker.positions().await?;
                let held = positions
                    .as_array()
                    .and_then(|rows| rows.iter().find(|r| r["symbol"] == s.symbol));
                let Some(held) = held else {
                    s.phase = "done".into();
                    s.completed_ms = now_ms();
                    s.reason =
                        "Alpaca ya no informa posición en este contrato; revisar diario".into();
                    save(p.to_owned(), Some(s.clone())).await?;
                    return Ok(s.reason.clone());
                };
                if held["side"] != "long"
                    || held["qty"].as_str() != Some("1")
                    || held["qty_available"].as_str() != Some("1")
                {
                    return Err("Posición distinta de un contrato largo disponible: requiere revisión manual".into());
                }
                if s.sequence >= 5 {
                    return Err("Cinco intentos de salida sin completar. Gestiona la posición en Alpaca Paper".into());
                }
                let quote = quotes.quote(&s.symbol).await?;
                quote.validate(now_ms(), false, 0.)?;
                s.sequence += 1;
                let id = format!("snipers-bot-{}-{}-{}-s{}", s.date, root, s.local_id, s.sequence);
                s.sell_id = Some(id.clone());
                s.phase = "sell_pending".into();
                s.order_ms = now_ms();
                s.cancel_requested = false;
                save(p.to_owned(), Some(s.clone())).await?;
                let intent = OrderIntent {
                    symbol: s.symbol.clone(),
                    qty: 1,
                    limit_price: quote.limit(false),
                    side: OrderSide::Sell,
                    client_order_id: id,
                };
                let result =
                    crate::alpaca::submit_fresh(app.clone(), intent, Some(quote.timestamp_ms))
                        .await;
                if let Ok(ref order) = result {
                    s.last_order = order.clone();
                    save(p.to_owned(), Some(s.clone())).await?;
                }
                result?;
                return Ok("Salida límite enviada a Alpaca Paper".into());
            }
            match quotes.quote(&s.symbol).await {
                Ok(q) if q.validate(now_ms(), false, 0.).is_ok() => {
                    s.mark_bid = Some(q.bid);
                    s.mark_ms = Some(q.timestamp_ms);
                }
                _ => {
                    s.mark_bid = None;
                    s.mark_ms = None;
                }
            }
            return Ok(s.reason.clone());
        }
    }
    if !c.armed {
        return Ok("Nuevas entradas pausadas".into());
    }
    let date = snipers_core::calendar::date_at(now)
        .ok_or("Fecha no disponible")?
        .to_string();
    if date != c.armed_date {
        if date > c.armed_date {
            control.lock().map_err(|_| "Bot ocupado")?.armed = false;
        }
        return Ok(format!(
            "Entradas autorizadas solo para {} · esperando sesión",
            c.armed_date
        ));
    }
    if !entry_asset_allowed(c,root){return Ok("Activo fuera del modo de nuevas entradas".into());}
    c.config.validate()?;
    let Some((local_id, side, spot)) = new_signal(&v, c.armed_ms, now) else {
        return Ok("Armado · esperando una nueva entrada del motor QQQ".into());
    };
    if v["symbol"]!=root{return Err("Lectura de otro activo".into());}
    let entry_number = next_entry_number(session.as_ref(), &date, local_id)?;
    let clock = broker.clock().await?;
    let close = clock["next_close"]
        .as_str()
        .and_then(snipers_core::calendar::rfc3339_ms)
        .ok_or("Cierre no disponible")?;
    if clock["is_open"] != true || now >= close.saturating_sub(30 * 60 * 1000) {
        return Err("No se admiten entradas en los últimos 30 minutos de sesión".into());
    }
    let positions=broker.positions().await?;let orders=broker.open_orders().await?;
    validate_portfolio(portfolio,&positions,&orders,root)?;
    let available=remaining_budget(portfolio,c.config.budget);
    if available<0.01{return Err("Presupuesto elegido reservado en posiciones u órdenes pendientes".into());}
    let target=v["reading"]["next"].as_f64().ok_or("Objetivo estructural no disponible")?;
    let range=v["reading"]["daily_range"].as_f64().ok_or("Falta rango diario verificado para elegir vencimiento")?;
    let (quote,expiry_plan)=quotes.select_auto(root,&side,spot,target,range,available,&c.config.expiry_mode).await?;
    let account=broker.account().await?;
    let buying_power=account["options_buying_power"].as_str().or_else(||account["non_marginable_buying_power"].as_str()).and_then(|v|v.parse::<f64>().ok());
    if !massive_feed::paper_readiness::account_enabled(&account)||buying_power.is_none_or(|cash|!cash.is_finite()||cash<quote.ask*100.){return Err("Poder de compra de opciones insuficiente o no verificado".into());}
    quote.validate(now_ms(), true, available)?;
    if (quote.ask - quote.bid) / quote.ask > 0.03 {
        return Err("Spread superior al 3%: supera el límite de entrada permitido".into());
    }
    let latest = control.lock().map_err(|_| "Bot ocupado")?.clone();
    let current=review(app,Some(root));
    if !latest.armed
        || latest.armed_ms != c.armed_ms
        || !prepared_signal_matches(&v,&current,root,&side,now_ms())
        || latest.qqq_only != c.qqq_only
        || !entry_asset_allowed(&latest,root)
    {
        return Err("La señal cambió o se pausó durante la preparación".into());
    }
    let id = format!("snipers-bot-{date}-{root}-{local_id}-buy");
    let mut s = Session {
        reserved_premium:quote.limit(true).parse::<f64>().map_err(|_|"Prima inválida")?*100.,
        date,
        daily_entries: entry_number,
        completed_ms: 0,
        local_id,
        symbol: quote.symbol.clone(),
        side,
        phase: "buy_pending".into(),
        buy_id: id.clone(),
        sell_id: None,
        sequence: 0,
        order_ms: now_ms(),
        cancel_requested: false,
        close_requested: false,
        reason: "Entrada Control intradía preparada".into(),
        strategy: massive_feed::intraday_strategy::VERSION.into(),
        entry_premium: None,
        profit_target_pct: Some(c.config.profit_target_pct),
        entry_time: None,
        exit_time: None,
        exit_premium: None,
        exit_trigger: None,
        mark_bid: None,
        mark_ms: None,
        management: Default::default(),
        entry_context: Value::Null,
        last_order: Value::Null,
    };
    s.management = crate::bot_position_policy::State::capture(&v, &s.side, now);
    s.management.swing=true;
    if s.management.anchor.is_none()||s.management.target.is_none(){return Err("Entrada sin invalidación u objetivo estructural verificable".into());}
    let r = &v["reading"];
    s.entry_context = json!({"management_version":"structural-overnight-smc-v3", "smc_version":massive_feed::smc_bot::VERSION, "maximum_loss":"paid-premium", "quick_context":r["quick_context"], "at":now,
        "expiry_plan":expiry_plan,"underlying":root,"entry_basis":r["entry_basis"],"entry_side":r["entry_side"],"structural_entry":r["structural_entry"],
        "spot":r["spot"],"vwap":r["vwap"],"levels":r["levels"],"next":r["next"],
        "flow_one":r["flow_one"],"flow_five":r["flow_five"],"net":r["net"],
        "coverage":r["coverage"],"components":r["components"],"closed_bars":r["closed_bars"],"ema":r["ema"],"neuro_context":r["neuro_context"]});
    audit(p.to_owned(),&s,&v,"entry",now).await?;
    save(p.to_owned(), Some(s.clone())).await?;
    *session = Some(s.clone());
    let result = crate::alpaca::submit_fresh(
        app.clone(),
        OrderIntent {
            symbol: quote.symbol.clone(),
            qty: 1,
            limit_price: quote.limit(true),
            side: OrderSide::Buy,
            client_order_id: id,
        },
        Some(quote.timestamp_ms),
    )
    .await;
    if let Ok(ref order) = result {
        let s = session.as_mut().unwrap();
        s.last_order = order.clone();
        save(p.to_owned(), Some(s.clone())).await?;
    }
    result?;
    Ok("Entrada límite enviada a Alpaca Paper".into())
}

#[cfg(test)]
mod tests {
    #[test] fn budget_is_user_selected_and_unknown_reservations_block(){
        let mut c=Config::default();c.budget=12500.;assert!(c.validate_for("2026-10-10").is_ok());
        assert_eq!(remaining_budget(&[],12500.),12500.);
        c.budget=0.01;assert!(c.validate_for("2026-10-10").is_ok());
        for invalid in [0.,-1.,f64::NAN,f64::INFINITY,f64::MAX]{c.budget=invalid;assert!(c.validate_for("2026-10-10").is_err());}
        let mut s=session();s.phase="open".into();s.reserved_premium=12500.;assert_eq!(remaining_budget(&vec![s.clone();5],12500.),0.);
        s.reserved_premium=0.;s.entry_premium=None;s.last_order=Value::Null;assert_eq!(remaining_budget(&[s],12500.),0.);
    }
    use super::*;
    #[test] fn portfolio_counts_pending_orders_and_restored_unknown_costs(){
        let mut a=session();a.reserved_premium=1000.;a.phase="buy_pending".into();
        let mut b=a.clone();b.symbol="SPY261016C00500000".into();b.buy_id="snipers-second".into();
        assert_eq!(remaining_budget(&[a.clone(),b.clone()],1000.),1000.);
        assert!(validate_portfolio(&[a.clone(),b.clone()],&json!([]),&json!([{"client_order_id":a.buy_id},{"client_order_id":b.buy_id}]),"NVDA").is_ok());
        assert!(validate_portfolio(&[a.clone()],&json!([]),&json!([]),"QQQ").is_err());
        assert!(validate_portfolio(&[a.clone()],&json!([{"symbol":"SPY261016C00500000","side":"long","qty":"1"}]),&json!([]),"NVDA").is_err());
        assert_eq!(remaining_budget(&vec![a.clone();5],1000.),0.);
        a.reserved_premium=0.;assert_eq!(remaining_budget(&[a],1000.),0.);
        b.phase="done".into();assert_eq!(remaining_budget(&[b],1000.),1000.);
    }
    #[test] fn preparation_revalidates_asset_side_route_and_current_evidence(){
        let before=json!({"symbol":"SPY","reviewedMs":10000,"position":{},"reading":{"ready":true,"eligible":true,"entry_side":"CALL","entry_basis":"magnet-wall","next":600.,"structural_entry":{"key":"same"}}});
        assert!(prepared_signal_matches(&before,&before,"SPY","CALL",10000));
        assert!(!prepared_signal_matches(&before,&before,"QQQ","CALL",10000));
        assert!(!prepared_signal_matches(&before,&before,"SPY","PUT",10000));
        assert!(!prepared_signal_matches(&before,&before,"SPY","CALL",16000));
        let mut changed=before.clone();changed["reading"]["structural_entry"]["key"]=json!("new");
        assert!(!prepared_signal_matches(&before,&changed,"SPY","CALL",10000));
        let mut rebound=before.clone();rebound["reading"]["entry_basis"]=json!("vwap-rebound");rebound["reading"]["closed_bars"]=json!([{"t":120000}]);
        assert!(prepared_signal_matches(&rebound,&rebound,"SPY","CALL",10000));
        let mut changed=rebound.clone();changed["reading"]["closed_bars"][0]["t"]=json!(240000);
        assert!(!prepared_signal_matches(&rebound,&changed,"SPY","CALL",10000));
        let mut chart=before.clone();chart["reading"]["quick_context"]=json!({"chartRoute":{"confirmed":true,"key":"route-1"}});
        assert!(prepared_signal_matches(&chart,&chart,"SPY","CALL",10000));
        let mut changed=chart.clone();changed["reading"]["quick_context"]["chartRoute"]["confirmed"]=json!(false);
        assert!(!prepared_signal_matches(&chart,&changed,"SPY","CALL",10000));
        let mut ema=before.clone();ema["reading"]["entry_basis"]=json!("ema9-rebound");
        ema["reading"]["quick_context"]=json!({"ema9Rebound":{"confirmed":true,"key":"bounce-1"}});
        assert!(prepared_signal_matches(&ema,&ema,"SPY","CALL",10000));
        let mut lost=ema.clone();lost["reading"]["quick_context"]["ema9Rebound"]["confirmed"]=json!(false);
        assert!(!prepared_signal_matches(&ema,&lost,"SPY","CALL",10000));
        lost=ema.clone();lost["reading"]["quick_context"]["ema9Rebound"]["key"]=json!("bounce-2");
        assert!(!prepared_signal_matches(&ema,&lost,"SPY","CALL",10000));
        changed=chart.clone();changed["reading"]["quick_context"]["chartRoute"]["key"]=json!("route-2");
        assert!(!prepared_signal_matches(&chart,&changed,"SPY","CALL",10000));
    }
    #[test]
    fn normalized_greeks_reach_the_bot_strategy_report_without_orders() {
        use snipers_core::{
            calendar,
            greek_support::Tracker,
            options::{normalize, Observation},
        };
        let now = calendar::rfc3339_ms("2026-09-25T15:00:00Z").unwrap();
        let ticker = "O:QQQ260925C00745000";
        let c=normalize(&json!({"details":{"ticker":ticker,"expiration_date":"2026-09-25","strike_price":745,"contract_type":"call"},"greeks":{"theta":-0.1,"vega":0.2},"implied_volatility":0.2,"underlying_asset":{"price":745,"last_updated":now*1_000_000},"last_quote":{"last_updated":now*1_000_000,"timeframe":"REAL-TIME"}}),"QQQ").unwrap();
        let mut observation = Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: now,
            observed_date: "2026-09-25".into(),
            oi_reference_date: None,
            contracts: [(ticker.into(), c)].into(),
        };
        let mut support = Tracker::default();
        support.update(Some(&observation), "QQQ", true, now);
        support.update(Some(&observation), "QQQ", true, now + 61000);
        let at = now + 300000;
        observation.observed_ms = at;
        let c = observation.contracts.values_mut().next().unwrap();
        c.iv = Some(0.19);
        c.quote_ms = Some(at);
        c.underlying_ms = Some(at);
        let context = support.update(Some(&observation), "QQQ", true, at);
        let mut snapshot = massive_feed::live::LiveState::default();
        snapshot.symbol = "QQQ".into();
        snapshot.published_ms = at;
        snapshot.greek_support = context;
        let reading = massive_feed::intraday_strategy::read(&strategy_data(&snapshot), at);
        let report = massive_feed::intraday_strategy::Tracker::default().update(&reading, at, None);
        let received = &report["reading"]["greek_support"];
        assert_eq!(received["comparable"], 1);
        assert_eq!(received["contracts"][0]["contract"]["theta"], -0.1);
        assert!(received["contracts"][0]["charmDeltaDay"].is_number());
        assert!(received["contracts"][0]["vannaDeltaIvPoint"].is_number());
        assert!(received["contracts"][0]["vegaEffectUnit"].as_f64().unwrap() < 0.);
        assert!(report["position"].is_null());
        assert!(!reading.eligible);
    }
    #[test]
    fn take_profit_requires_fresh_bid_and_actual_fill() {
        let mut s = session();
        s.entry_premium = Some(2.);
        s.mark_ms = Some(10000);
        for side in ["CALL", "PUT"] {
            s.side = side.into();
            s.mark_bid = Some(2.19);
            assert!(take_profit_reason(&s, 10000).is_none());
            s.mark_bid = Some(2.20);
            assert!(take_profit_reason(&s, 15000).unwrap().contains("+10%"));
            assert!(take_profit_reason(&s, 15001).is_none());
            assert!(take_profit_reason(&s, 9999).is_none());
        }
        s.entry_premium = Some(2.01);
        s.mark_bid = Some(2.21);
        assert!(take_profit_reason(&s, 10000).is_none());
        s.mark_bid = Some(2.22);
        assert!(take_profit_reason(&s, 10000).is_some());
        s.entry_premium = None;
        assert!(take_profit_reason(&s, 10000).is_none());
        s.entry_premium = Some(2.);
        s.phase = "buy_pending".into();
        assert!(take_profit_reason(&s, 10000).is_none());
    }
    #[test]
    fn premium_movements_do_not_trigger_fixed_exits() {
        let mut s = session();
        s.strategy = massive_feed::intraday_strategy::VERSION.into();
        s.entry_premium = Some(2.0);
        let v = json!({"reviewedMs":10000,"reading":{"ready":true},"oppositionExit":false});
        for bid in [0.01, 1.0, 1.70, 2.0, 2.40, 8.0] {
            s.mark_bid = Some(bid);
            assert_eq!(exit_reason(&s, &v, 10000, u64::MAX), None);
        }
        s.close_requested = true;
        assert!(exit_reason(&s, &v, 10000, u64::MAX).is_some());
    }
    #[test]
    fn actual_entry_premium_survives_exit_and_restart() {
        let mut s = session();
        s.strategy = massive_feed::intraday_strategy::VERSION.into();
        s.phase = "buy_pending".into();
        apply_order(&mut s,&json!({"client_order_id":"snipers-test","status":"filled","filled_qty":"1","filled_avg_price":"2.00"})).unwrap();
        let mut restored: Session =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(restored.entry_premium, Some(2.));
        restored.phase = "sell_pending".into();
        restored.sell_id = Some("snipers-exit".into());
        apply_order(
            &mut restored,
            &json!({"client_order_id":"snipers-exit","status":"canceled","filled_qty":"0"}),
        )
        .unwrap();
        assert_eq!(restored.entry_premium, Some(2.));
        assert!(restored.close_requested);
    }
    #[test]
    fn daily_counter_survives_restart_and_resets_only_on_new_date() {
        let mut s = session();
        s.daily_entries = 2;
        s.phase = "done".into();
        let restored: Session = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(entries_today(Some(&restored), "2026-09-25"), 2);
        assert_eq!(entries_today(Some(&restored), "2026-09-28"), 0);
        let mut legacy = serde_json::to_value(&s).unwrap();
        legacy.as_object_mut().unwrap().remove("daily_entries");
        assert_eq!(
            serde_json::from_value::<Session>(legacy)
                .unwrap()
                .daily_entries,
            1
        );
    }
    #[test]
    fn unlimited_entries_still_require_completed_trades_and_new_signals() {
        assert_eq!(next_entry_number(None, "2026-09-25", 10000).unwrap(), 1);
        let mut s = session();
        s.local_id = 10000;
        assert!(next_entry_number(Some(&s), "2026-09-25", 20000).is_err());
        s.phase = "done".into();
        s.completed_ms = 15000;
        assert!(next_entry_number(Some(&s), "2026-09-25", 10000).is_err());
        assert!(next_entry_number(Some(&s), "2026-09-25", 14000).is_err());
        assert_eq!(next_entry_number(Some(&s), "2026-09-25", 20000).unwrap(), 2);
        s.daily_entries = 2;
        s.local_id = 20000;
        s.completed_ms = 25000;
        assert_eq!(next_entry_number(Some(&s), "2026-09-25", 30000).unwrap(), 3);
        s.daily_entries = 99;
        assert_eq!(
            next_entry_number(Some(&s), "2026-09-25", 30000).unwrap(),
            100
        );
        s.phase = "open".into();
        assert!(next_entry_number(Some(&s), "2026-09-25", 30000).is_err());
        s.phase = "done".into();
        assert_eq!(next_entry_number(Some(&s), "2026-09-28", 30000).unwrap(), 1);
    }
    #[test]
    fn intraday_exit_does_not_follow_old_coordinator_or_structural_target() {
        let mut s = session();
        s.strategy = massive_feed::intraday_strategy::VERSION.into();
        let mut v = json!({"reviewedMs":10000,"reading":{"ready":true},"position":null,"oppositionExit":false});
        assert_eq!(exit_reason(&s, &v, 10000, u64::MAX), None);
        v["oppositionExit"] = true.into();
        assert!(exit_reason(&s, &v, 10000, u64::MAX)
            .unwrap()
            .contains("Fuerza contraria"));
        v["oppositionExit"] = false.into();
        v["reading"]["ready"] = false.into();
        assert!(exit_reason(&s, &v, 10000, u64::MAX).is_some());
    }
    fn signal() -> Value {
        json!({"symbol":"QQQ","entriesEnabled":true,"reviewedMs":10000,"position":{"tradeId":7,"trade":{"openedMs":9500,"closedMs":null,"side":"CALL","entry":745.}}})
    }
    #[test]
    fn configurable_profit_target_is_validated_and_captured() {
        let mut c=Config::default();
        for pct in [10.,25.,100.,250.] { c.profit_target_pct=pct;assert!(c.validate_for("2026-10-10").is_ok()); }
        for pct in [9.99,0.,-10.,f64::NAN,f64::INFINITY] { c.profit_target_pct=pct;assert!(c.validate_for("2026-10-10").is_err()); }
        let old:Config=serde_json::from_value(json!({"expiry":"","budget":1000.})).unwrap();assert_eq!(old.profit_target_pct,10.);
        let mut s=session();s.strategy=massive_feed::intraday_strategy::VERSION.into();s.phase="open".into();s.entry_premium=Some(2.);s.mark_ms=Some(10000);s.mark_bid=Some(2.5);
        assert!(take_profit_reason(&s,10000).is_none());
        s.profit_target_pct=Some(25.);s.mark_bid=Some(2.49);assert!(take_profit_reason(&s,10000).is_none());s.mark_bid=Some(2.5);assert!(take_profit_reason(&s,10000).unwrap().contains("+25%"));assert!(take_profit_reason(&s,15001).is_none());
        let restored:Session=serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();assert_eq!(restored.profit_target_pct,Some(25.));
    }
    fn session() -> Session {
        Session {
            reserved_premium:0.,
            daily_entries: 1,
            completed_ms: 0,
            strategy: String::new(),
            entry_premium: None,
            profit_target_pct: None,
            entry_time: None,
            exit_time: None,
            exit_premium: None,
            exit_trigger: None,
            mark_bid: None,
            mark_ms: None,
            management: Default::default(),
            entry_context: Value::Null,
            date: "2026-09-25".into(),
            local_id: 7,
            symbol: "QQQ260925C00745000".into(),
            side: "CALL".into(),
            phase: "open".into(),
            buy_id: "snipers-test".into(),
            sell_id: None,
            sequence: 0,
            order_ms: 10000,
            cancel_requested: false,
            close_requested: false,
            reason: String::new(),
            last_order: Value::Null,
        }
    }
    #[test]
    fn only_new_fresh_supported_asset_signals() {
        assert!(new_signal(&signal(), 9000, 10000).is_some());
        assert!(new_signal(&signal(), 9600, 10000).is_none());
        assert!(new_signal(&signal(), 9000, 20000).is_none());
        let mut v = signal();
        v["symbol"] = "SPX".into();
        assert!(new_signal(&v, 9000, 10000).is_none());
    }
    #[test]
    fn exit_on_disconnect_local_close_and_session_end() {
        let s = session();
        assert!(exit_reason(&s, &signal(), 10000, 2000000).is_none());
        assert!(exit_reason(&s, &Value::Null, 10000, 2000000).is_some());
        assert!(exit_reason(&s, &signal(), 10000, 800000).is_some());
        let mut v = signal();
        v["position"]["trade"]["closedMs"] = 10000.into();
        assert!(exit_reason(&s, &v, 10000, 2000000).is_some());
    }
    #[test]
    fn cancellation_ack_is_not_a_fill() {
        assert!(!terminal("pending_cancel"));
        assert!(!terminal("partially_filled"));
        assert!(terminal("canceled"));
        assert!(terminal("filled"));
    }
    #[test]
    fn simulated_broker_round_trip() {
        let mut s = session();
        s.phase = "buy_pending".into();
        let ack = json!({"client_order_id":s.buy_id,"status":"accepted","filled_qty":"0"});
        assert!(!apply_order(&mut s, &ack).unwrap());
        assert_eq!(s.phase, "buy_pending");
        let fill = json!({"client_order_id":s.buy_id,"status":"filled","filled_qty":"1","filled_avg_price":"1.25"});
        assert!(apply_order(&mut s, &fill).unwrap());
        assert_eq!(s.phase, "open");
        s.phase = "sell_pending".into();
        s.sell_id = Some("snipers-sell".into());
        assert!(!apply_order(
            &mut s,
            &json!({"client_order_id":"snipers-sell","status":"pending_cancel","filled_qty":"0"})
        )
        .unwrap());
        assert!(apply_order(
            &mut s,
            &json!({"client_order_id":"snipers-sell","status":"canceled","filled_qty":"0"})
        )
        .unwrap());
        assert_eq!(s.phase, "open");
        assert!(s.close_requested);
        s.phase = "sell_pending".into();
        assert!(apply_order(
            &mut s,
            &json!({"client_order_id":"snipers-sell","status":"filled","filled_qty":"1"})
        )
        .unwrap());
        assert_eq!(s.phase, "done");
    }
    #[test]
    fn history_uses_broker_fill_times_and_preserves_exit_trigger() {
        let mut s = session();
        s.phase = "buy_pending".into();
        apply_order(&mut s, &json!({"client_order_id":"snipers-test","status":"filled","filled_qty":"1","filled_avg_price":"2.00","filled_at":"2026-09-28T14:00:00Z"})).unwrap();
        assert_eq!(s.entry_time.as_deref(), Some("2026-09-28T14:00:00Z"));
        s.phase = "sell_pending".into();
        s.sell_id = Some("snipers-sell".into());
        s.exit_trigger = Some("Take profit".into());
        apply_order(&mut s, &json!({"client_order_id":"snipers-sell","status":"filled","filled_qty":"1","filled_avg_price":"2.30","filled_at":"2026-09-28T14:05:00Z"})).unwrap();
        assert_eq!(s.exit_time.as_deref(), Some("2026-09-28T14:05:00Z"));
        assert_eq!(s.exit_premium, Some(2.3));
        assert_eq!(s.exit_trigger.as_deref(), Some("Take profit"));
    }
    #[test]
    fn canceled_buy_and_restart_preserve_identity() {
        let mut s = session();
        s.phase = "buy_pending".into();
        let s_json = serde_json::to_string(&s).unwrap();
        let mut recovered: Session = serde_json::from_str(&s_json).unwrap();
        assert_eq!(s.buy_id, recovered.buy_id);
        assert!(apply_order(
            &mut recovered,
            &json!({"client_order_id":"snipers-test","status":"canceled","filled_qty":"0"})
        )
        .unwrap());
        assert_eq!(recovered.phase, "done");
    }
    #[test]
    fn foreign_and_fractional_fills_rejected() {
        let mut s = session();
        s.phase = "buy_pending".into();
        assert!(apply_order(
            &mut s,
            &json!({"client_order_id":"foreign","status":"filled","filled_qty":"1"})
        )
        .is_err());
        assert!(apply_order(
            &mut s,
            &json!({"client_order_id":"snipers-test","status":"filled","filled_qty":"0.5"})
        )
        .is_err());
    }
}
