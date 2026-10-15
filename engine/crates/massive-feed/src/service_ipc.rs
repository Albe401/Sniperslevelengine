//! Loopback-only, bounded, read-only-data IPC. No broker or order commands.
use crate::live;
use serde_json::{json,Value};
use std::{io::{Read,Write},net::{TcpListener,TcpStream,SocketAddr},sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},time::Duration};
pub const ADDRESS:&str="127.0.0.1:36541";
pub const DIRECT_READONLY_ADDRESS:&str="127.0.0.1:36542";
pub const PROTOCOL:&str="snipers-data-service-v1";
const MAX_REQUEST:usize=16*1024;
const MAX_RESPONSE:usize=32*1024*1024;
pub type Slot=Arc<Mutex<live::SharedState>>;
pub type Desired=Arc<Mutex<String>>;
fn receive(stream:&mut TcpStream,limit:usize)->Result<Value,&'static str>{
 let mut header=[0;4];stream.read_exact(&mut header).map_err(|_|"IPC sin respuesta")?;
 let len=u32::from_be_bytes(header) as usize;if len>limit{return Err("IPC excede límite");}
 let mut bytes=vec![0;len];stream.read_exact(&mut bytes).map_err(|_|"IPC incompleto")?;
 serde_json::from_slice(&bytes).map_err(|_|"IPC inválido")
}
fn send(stream:&mut TcpStream,value:&Value,limit:usize)->Result<(),&'static str>{
 let bytes=serde_json::to_vec(value).map_err(|_|"IPC no serializable")?;
 if bytes.len()>limit{return Err("IPC excede límite");}
 stream.write_all(&(bytes.len() as u32).to_be_bytes()).and_then(|_|stream.write_all(&bytes)).map_err(|_|"IPC interrumpido")
}
pub fn present()->bool{TcpStream::connect_timeout(&ADDRESS.parse::<SocketAddr>().unwrap(),Duration::from_millis(300)).is_ok()}
pub fn request(value:Value)->Result<Value,String>{
 request_at(ADDRESS,value)
}
pub fn request_at(address:&str,value:Value)->Result<Value,String>{
 let target:SocketAddr=address.parse().map_err(|_|"Dirección local inválida")?;if !target.ip().is_loopback(){return Err("Solo receptor local".into());}
 let mut stream=TcpStream::connect_timeout(&target,Duration::from_secs(2)).map_err(|_|"Motor independiente no disponible")?;
 stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(|_|"IPC no disponible")?;stream.set_write_timeout(Some(Duration::from_secs(5))).map_err(|_|"IPC no disponible")?;
 send(&mut stream,&value,MAX_REQUEST).map_err(str::to_owned)?;
 let response=receive(&mut stream,MAX_RESPONSE).map_err(str::to_owned)?;
 if response["protocol"]!=PROTOCOL{return Err("Protocolo del motor incompatible; no se abre otra conexión".into());}
 if response["ok"]!=true{return Err(response["error"].as_str().unwrap_or("Solicitud no disponible").into());}
 Ok(response["result"].clone())
}
fn dispatch(slot:&Slot,desired:&Desired,request:Value)->Result<Value,&'static str>{
 dispatch_at(slot,desired,request,None)
}
fn dispatch_at(slot:&Slot,desired:&Desired,request:Value,audit_clock:Option<u64>)->Result<Value,&'static str>{
 let shared=slot.lock().map_err(|_|"Estado ocupado")?.clone();
 match request["method"].as_str(){
  Some("snapshot_bundle")=>Ok(crate::snapshot_lanes::bundle(&shared,audit_clock.unwrap_or_else(live::now_ms))),
  Some("snapshot")=>{let frame=match audit_clock{Some(ms)=>live::snapshot_for_audit(&shared,ms),None=>live::snapshot(&shared)};let mut value=serde_json::to_value(frame).map_err(|_|"Lectura no disponible")?;value["dataService"]=json!({"protocol":PROTOCOL,"independent":true});if let Some(ms)=audit_clock{value["dataService"]["auditClockMs"]=json!(ms);}Ok(value)},
  Some("select")=>{let symbol=request["symbol"].as_str().ok_or("Activo ausente")?;if snipers_core::universe::stock_asset(symbol).is_none(){return Err("Activo no habilitado");}*desired.lock().map_err(|_|"Selección ocupada")?=symbol.into();Ok(json!({"requestedSymbol":symbol}))},
  Some("learning_observation")=>{let frame=live::snapshot(&shared);let mut value=crate::learning_observation::observation_value(&frame,live::now_ms(),"independent-data-service");value["gammaComparable"]=json!(frame.option_snapshot_status.horizons.iter().find(|h|h.mode=="DAY").is_some_and(|h|Some(h.observed_ms)==request["snapshotMs"].as_u64()&&h.structure.complete&&h.exposure.gex_complete));Ok(value)},
  Some("research_watch")=>{
   let rows=request["symbols"].as_array().ok_or("Lista de investigación inválida")?;
   if rows.len()>5{return Err("Máximo cinco activos de investigación");}
   let mut symbols=Vec::new();for row in rows {let symbol=row.as_str().ok_or("Activo inválido")?;
    if snipers_core::universe::stock_asset(symbol).is_none(){return Err("Activo no habilitado");}
    if !symbols.iter().any(|s|s==symbol){symbols.push(symbol.to_owned());}
   }
   {let mut state=shared.lock().map_err(|_|"Estado ocupado")?;state.research_symbols=symbols.clone();state.scanner_options_notify.notify_one();}
   let current=live::snapshot(&shared);let mut frames=Vec::new();for symbol in &symbols{
    if *symbol==current.symbol {frames.push(serde_json::to_value(&current).map_err(|_|"Lectura no disponible")?);}else{frames.push(live::bot_frame(&shared,symbol,live::now_ms()));}
   }
   Ok(json!({"symbols":symbols,"frames":frames,"executionEnabled":false}))
  },
  Some("analyst")=>{let frame=live::snapshot(&shared);crate::analyst_context::prepare(&frame,live::now_ms()).map_err(|_|"Contexto sin datos vigentes")},
  Some("direction_enabled")=>{let enabled=request["enabled"].as_bool().ok_or("Estado inválido")?;let mut state=shared.lock().map_err(|_|"Estado ocupado")?;state.direction_enabled=enabled;state.scanner_options_notify.notify_one();Ok(Value::Null)},
  Some("direction")=>{let s=live::snapshot(&shared);let state=shared.lock().map_err(|_|"Estado ocupado")?;let mut frames=vec![json!({"symbol":s.symbol,"startedMs":s.started_ms,"publishedMs":s.published_ms,"stocks":s.stocks,"options":s.options,"ticks":s.ticks,"metrics":s.metrics,"treasury":s.treasury,"directionOi":s.direction_oi,"minutes":s.minutes,"cvd":s.cvd,"chartHistory":s.chart_history,"flow":s.flow,"optionSnapshotStatus":s.option_snapshot_status})];let mut symbols=state.scanner_options.symbols();symbols.truncate(20);let enabled=state.direction_enabled;drop(state);for symbol in symbols{if symbol!=s.symbol{let frame=live::bot_frame(&shared,&symbol,live::now_ms());if frame["ticks"][&symbol].is_object(){frames.push(frame);}}}Ok(json!({"connected":true,"enabled":enabled,"frames":frames}))},
  Some("etf_enabled")=>{let enabled=request["enabled"].as_bool().ok_or("Estado inválido")?;shared.lock().map_err(|_|"Estado ocupado")?.set_etf_scanner_enabled(enabled);Ok(Value::Null)},
  Some("qqq")=>{let mut state=shared.lock().map_err(|_|"Estado ocupado")?;state.qqq_ui_enabled=request["enabled"].as_bool().unwrap_or(false);Ok(json!({"connected":state.stocks.phase=="streaming","report":state.qqq_report}))},
  Some("alerts")=>{let alerts=serde_json::from_value(request["alerts"].clone()).map_err(|_|"Alarmas inválidas")?;shared.lock().map_err(|_|"Estado ocupado")?.price_alert_engine.configure(alerts).map_err(|_|"Alarmas inválidas")?;Ok(Value::Null)},
  Some("refresh_options")=>{shared.lock().map_err(|_|"Estado ocupado")?.snapshot_notify.notify_one();Ok(Value::Null)},
  Some("recording")=>{let state=shared.lock().map_err(|_|"Estado ocupado")?;serde_json::to_value(crate::recording::Recording::capture(&state.symbol,&state.telemetry)).map_err(|_|"Registro no disponible")},
  _=>Err("Método no admitido; ejecución de órdenes no disponible"),
 }
}
pub struct Server{stop:Arc<AtomicBool>,thread:Option<std::thread::JoinHandle<()>>}
pub fn serve(listener:TcpListener,slot:Slot,desired:Desired)->std::io::Result<Server>{
 serve_mode(listener,slot,desired,false,None)
}
/// Offline harness clock is fixed at construction, never accepted from an IPC request.
pub fn serve_for_audit(listener:TcpListener,slot:Slot,desired:Desired,as_of_ms:u64)->std::io::Result<Server>{serve_mode(listener,slot,desired,false,Some(as_of_ms))}
/// Direct desktop observations only. Selection, toggles, credentials and orders are unavailable.
pub fn serve_readonly(listener:TcpListener,slot:Slot)->std::io::Result<Server>{
 serve_mode(listener,slot,Arc::new(Mutex::new(String::new())),true,None)
}
fn serve_mode(listener:TcpListener,slot:Slot,desired:Desired,readonly:bool,audit_clock:Option<u64>)->std::io::Result<Server>{
 if !listener.local_addr()?.ip().is_loopback(){return Err(std::io::Error::other("Solo loopback"));}
 listener.set_nonblocking(true)?;let stop=Arc::new(AtomicBool::new(false));let worker_stop=stop.clone();
 let thread=std::thread::spawn(move||{while !worker_stop.load(Ordering::Relaxed){match listener.accept(){Ok((mut stream,_))=>{
  let _=stream.set_read_timeout(Some(Duration::from_secs(2)));let _=stream.set_write_timeout(Some(Duration::from_secs(2)));
  let result=receive(&mut stream,MAX_REQUEST).and_then(|r|{
   if readonly&&!matches!(r["method"].as_str(),Some("snapshot"|"direction"|"snapshot_bundle")){return Err("Solo lectura: snapshot, direction y snapshot_bundle");}
   let mut value=dispatch_at(&slot,&desired,r,audit_clock)?;
   if readonly&&value["dataService"].is_object(){value["dataService"]["independent"]=json!(false);value["dataService"]["readonly"]=json!(true);}
   Ok(value)
  });
  let response=match result{Ok(v)=>json!({"protocol":PROTOCOL,"ok":true,"result":v}),Err(e)=>json!({"protocol":PROTOCOL,"ok":false,"error":e})};
  let _=send(&mut stream,&response,MAX_RESPONSE);
 },Err(e)if e.kind()==std::io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(20)),Err(_)=>break}}});
 Ok(Server{stop,thread:Some(thread)})
}
impl Drop for Server{fn drop(&mut self){self.stop.store(true,Ordering::Relaxed);if let Some(thread)=self.thread.take(){let _=thread.join();}}}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn research_is_passive_bounded_and_preserves_asset_identity(){
  let shared=live::new_state_for("QQQ").unwrap();let slot=Arc::new(Mutex::new(shared.clone()));let desired=Arc::new(Mutex::new("QQQ".to_owned()));
  let response=dispatch(&slot,&desired,json!({"method":"research_watch","symbols":["QQQ","NVDA","AMZN","BAC","NBIS"]})).unwrap();
  assert_eq!(response["frames"].as_array().unwrap().len(),5);assert_eq!(response["frames"][3]["symbol"],"BAC");assert!(response["frames"][3]["ticks"]["BAC"].is_null());
  assert!(!shared.lock().unwrap().bot_enabled);assert_eq!(*desired.lock().unwrap(),"QQQ");
  let state=shared.lock().unwrap();assert_eq!(state.scanner_options.priority,vec!["QQQ","NVDA","AMZN","BAC","NBIS"]);assert_eq!(state.scanner_options.capacity,20);drop(state);
  assert!(dispatch(&slot,&desired,json!({"method":"research_watch","symbols":["QQQ","NVDA","AMZN","BAC","NBIS","AAPL"]})).is_err());
  assert!(dispatch(&slot,&desired,json!({"method":"research_watch","symbols":["INVALID!"]})).is_err());assert_eq!(shared.lock().unwrap().research_symbols.len(),5);
 }
 #[test]fn direct_observer_never_mutates_feed_or_selects_symbol(){
  let state=live::new_state_for("QQQ").unwrap();let slot=Arc::new(Mutex::new(state));let listener=TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap().to_string();let _server=serve_readonly(listener,slot).unwrap();
  for method in ["select","direction_enabled","etf_enabled","qqq","refresh_options","alerts","place_order"]{assert!(request_at(&address,json!({"method":method,"symbol":"NVDA","enabled":true})).is_err());}
  let f=request_at(&address,json!({"method":"snapshot"})).unwrap();assert_eq!(f["symbol"],"QQQ");assert_eq!(f["dataService"]["readonly"],true);assert_eq!(f["dataService"]["independent"],false);
  assert!(request_at(&address,json!({"method":"direction"})).is_ok());
 }
 #[test]fn clients_detach_without_stopping_shared_receiver_and_selection_is_requested(){
  let shared=live::new_state_for("QQQ").unwrap();let slot=Arc::new(Mutex::new(shared));let desired=Arc::new(Mutex::new("QQQ".to_owned()));let listener=TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap().to_string();let _server=serve(listener,slot,desired.clone()).unwrap();
  let first=request_at(&address,json!({"method":"snapshot"})).unwrap();assert_eq!(first["symbol"],"QQQ");
  assert!(request_at(&address,json!({"method":"send_order"})).is_err());
  request_at(&address,json!({"method":"select","symbol":"NVDA"})).unwrap();assert_eq!(*desired.lock().unwrap(),"NVDA");
  assert!(request_at(&address,json!({"method":"select","symbol":"INVALID!"})).is_err());
  assert_eq!(request_at(&address,json!({"method":"snapshot"})).unwrap()["symbol"],"QQQ");
 }
}
