use massive_feed::{background_capture,credentials::Credentials,live,service_ipc};
use std::sync::{Arc,Mutex};
fn desktop_running()->Result<bool,&'static str>{
 #[cfg(windows)]{use std::os::windows::process::CommandExt;let out=std::process::Command::new("tasklist.exe").args(["/FI","IMAGENAME eq snipers-desktop.exe","/FO","CSV","/NH"]).creation_flags(0x08000000).output().map_err(|_|"No se pudo comprobar Snipers")?;if !out.status.success(){return Err("No se pudo comprobar Snipers");}Ok(String::from_utf8_lossy(&out.stdout).to_ascii_lowercase().contains("\"snipers-desktop.exe\""))}
 #[cfg(not(windows))]{Err("Comprobación requiere Windows")}
}
#[tokio::main]async fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();
 if args.get(1).map(String::as_str)==Some("--check"){println!("{}",serde_json::json!({"mode":"offline_check","desktopRunning":desktop_running()?,"connectsToMassive":false,"independentProcess":true,"uiSharedConnectionIntegrated":true}));return Ok(());}
 if args.get(1).map(String::as_str)==Some("--rpc"){
  let method=args.get(2).ok_or("Método ausente")?;let result=service_ipc::request(serde_json::json!({"method":method,"symbol":args.get(3)}))?;println!("{}",result);return Ok(());
 }
 if args.len()!=4||!["--run","--resume-shared"].contains(&args[1].as_str()){return Err("Uso: --check, --rpc <metodo> o --run <simbolo> <directorio-nuevo>".into());}
 // Migration startup still refuses an existing desktop; once attached, clients may come and go.
 // Explicit recovery only after verifying that the existing UI uses remote IPC.
 if args[1]!="--resume-shared" && desktop_running()?{return Err("Inicia el receptor antes de abrir Snipers durante la migración.".into());}
 let listener=std::net::TcpListener::bind(service_ipc::ADDRESS).map_err(|_|"Ya hay receptor o puerto ocupado")?;
 let initial=live::new_state_for(&args[2])?;let slot=Arc::new(Mutex::new(initial));let desired=Arc::new(Mutex::new(args[2].clone()));
 let directory=std::path::Path::new(&args[3]);if let Some(parent)=directory.parent(){std::fs::create_dir_all(parent)?;}std::fs::create_dir(directory)?;
 let (capture,writer)=background_capture::open(&directory.join("events.jsonl"),10000)?;
 let _server=service_ipc::serve(listener,slot.clone(),desired.clone())?;
 let application_directory=std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).ok_or("Carpeta local no disponible")?.join("com.snipers.workspace");std::fs::create_dir_all(&application_directory)?;
 let mut next_reading=0;let mut reading_bytes=0usize;let mut reading_archive_full=false;
 let reason='receiver:loop{
  let symbol=desired.lock().unwrap().clone();let state=live::new_state_for(&symbol)?;
  {
   let mut next=state.lock().unwrap();let old=slot.lock().unwrap().clone();let old=old.lock().unwrap();
   next.background_capture=Some(capture.clone());next.direction_enabled=old.direction_enabled;next.research_symbols=old.research_symbols.clone();next.qqq_ui_enabled=old.qqq_ui_enabled;next.price_alert_engine=old.price_alert_engine.clone();next.set_etf_scanner_enabled(old.etf_scanner_enabled);next.scanner_store=old.scanner_store.clone();next.scanner_options=old.scanner_options.clone();next.bot_ticks=old.bot_ticks.clone();next.qqq_components=old.qqq_components.clone();
   next.snapshot_lanes=old.snapshot_lanes.clone();
  }
  *slot.lock().unwrap()=state.clone();
  let key=Credentials::load()?;
  let (learning_tx,learning_rx)=tokio::sync::mpsc::channel(1);let (v2_tx,v2_rx)=tokio::sync::mpsc::channel(1);let mut next_learning=0u64;
  let background=async{tokio::join!(live::run(key,state.clone()),massive_feed::qqq_component_reading::run(state.clone(),application_directory.clone()),massive_feed::swing_observer::run(state.clone(),directory.to_path_buf()),massive_feed::neuro_context::run(state.clone()),massive_feed::learning_observation::run(application_directory.clone(),learning_rx),massive_feed::passive_learning_v2::run(application_directory.clone(),state.clone(),v2_rx));};tokio::pin!(background);
  let mut timer=tokio::time::interval(std::time::Duration::from_secs(1));
  loop{tokio::select!{
   _=&mut background=>break 'receiver "feed_finished",
   _=timer.tick()=>{
    if directory.join("STOP").exists(){break 'receiver "user_stop_file";}
    // A failed archive must remain visible, but must not freeze live market analysis.
    if *desired.lock().unwrap()!=symbol{break;}
    let frame=live::snapshot(&state);if live::now_ms()>=next_learning{next_learning=live::now_ms()+2000;let _=learning_tx.try_send(frame.clone());let _=v2_tx.try_send(frame.clone());}
    let health=serde_json::json!({"at":live::now_ms(),"pid":std::process::id(),"symbol":symbol,"stocks":frame.stocks,"options":frame.options,"capture":capture.health(),"captureDegraded":capture.health()["writeFailed"],"uiSharedConnectionIntegrated":true,"readingArchiveFull":reading_archive_full});
    std::fs::write(directory.join("health.json"),serde_json::to_vec_pretty(&health)?)?;
    if !reading_archive_full && live::now_ms()>=next_reading{let bytes=serde_json::to_vec(&frame)?;reading_bytes+=bytes.len();if reading_bytes>256*1024*1024{reading_archive_full=true;}else{std::fs::write(directory.join(format!("reading-{}.json",live::now_ms())),bytes)?;}next_reading=live::now_ms()+30000;}
   }
  }}
  // Drop all old feed futures before starting the requested asset, preserving single ownership.
 };
 drop(writer);std::fs::write(directory.join("stopped.json"),serde_json::to_vec_pretty(&serde_json::json!({"reason":reason,"capture":capture.health(),"at":live::now_ms()}))?)?;
 Ok(())
}
