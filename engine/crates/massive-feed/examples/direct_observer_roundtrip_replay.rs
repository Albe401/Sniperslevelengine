use massive_feed::{live,recording::Recording,service_ipc};
use std::{sync::{Arc,Mutex},net::TcpListener};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();if args.len()!=3{return Err("Uso: <grabacion> <resultado-nuevo>".into());}
 let recording=Recording::read(std::path::Path::new(&args[1]))?;let shared=live::new_state_for(&recording.symbol)?;live::ingest_recording(&shared,&recording);
 let slot=Arc::new(Mutex::new(shared.clone()));let listener=TcpListener::bind("127.0.0.1:0")?;let address=listener.local_addr()?.to_string();let _server=service_ipc::serve_readonly(listener,slot)?;
 let remote=service_ipc::request_at(&address,serde_json::json!({"method":"snapshot"}))?;let direct=serde_json::to_value(live::snapshot(&shared))?;
 for period in ["premarket","regular"]{for field in ["buy","sell","unknown","delta"]{assert_eq!(remote["cvd"][period][field],direct["cvd"][period][field]);}}
 assert_eq!(remote["metrics"],direct["metrics"]);assert_eq!(remote["ticks"],direct["ticks"]);
 // A second client after the first connection closes still reads the same receiver.
 let second=service_ipc::request_at(&address,serde_json::json!({"method":"snapshot"}))?;assert_eq!(second["cvd"]["regular"],remote["cvd"]["regular"]);
 let historical=massive_feed::comparison::compare(&recording)?;let r=&historical["candidateCvd"]["regular"];let total=r["buy"].as_f64().unwrap()+r["sell"].as_f64().unwrap()+r["unknown"].as_f64().unwrap();assert!((total-2272.698918).abs()<1e-8);assert!((r["delta"].as_f64().unwrap()+739.99778).abs()<1e-8);
 // A live observation on another date must not publish yesterday's CVD as today's.
 let archived_at=recording.records.iter().map(|r|r.received_ms).max().unwrap();
 if snipers_core::calendar::date_at(archived_at)!=snipers_core::calendar::date_at(live::now_ms()){
  assert_eq!(remote["cvd"]["available"],false);assert_eq!(remote["cvd"]["regular"]["delta"],0.0);
 }
 let result=serde_json::json!({"status":"passed","observer":"desktop_direct_readonly","symbol":recording.symbol,"inputRows":recording.records.len(),"historicalRegularVolume":total,"historicalRegularDelta":r["delta"],"roundtripChangedQuantities":0,"clientDisconnectStopsReceiver":false,"scope":"Real retained recording arithmetic and live transport equality; historical CVD is not relabelled as current; not current market delivery","reading":remote,"historicalCalculation":historical});
 let file=std::fs::OpenOptions::new().write(true).create_new(true).open(&args[2])?;serde_json::to_writer_pretty(&file,&result)?;file.sync_all()?;println!("{}",serde_json::json!({"status":"passed","regularVolume":total,"clientDisconnectStopsReceiver":false}));Ok(())
}
