use massive_feed::{live,recording::Recording,service_ipc};
use std::{sync::{Arc,Mutex},net::TcpListener};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();if args.len()!=3{return Err("Uso: <grabacion> <resultado-nuevo>".into());}
 let recording=Recording::read(std::path::Path::new(&args[1]))?;let shared=live::new_state_for(&recording.symbol)?;live::ingest_recording(&shared,&recording);
 let as_of_ms=recording.records.iter().map(|r|r.received_ms).max().ok_or("Registro vacío")?;
 let slot=Arc::new(Mutex::new(shared.clone()));let desired=Arc::new(Mutex::new(recording.symbol.clone()));let listener=TcpListener::bind("127.0.0.1:0")?;let address=listener.local_addr()?.to_string();let _server=service_ipc::serve_for_audit(listener,slot,desired,as_of_ms)?;
 let remote=service_ipc::request_at(&address,serde_json::json!({"method":"snapshot"}))?;let direct=serde_json::to_value(live::snapshot_for_audit(&shared,as_of_ms))?;
 for period in ["premarket","regular"]{for field in ["buy","sell","unknown","delta"]{assert_eq!(remote["cvd"][period][field],direct["cvd"][period][field]);}}
 assert_eq!(remote["metrics"],direct["metrics"]);assert_eq!(remote["ticks"],direct["ticks"]);
 // A second client after the first connection closes still reads the same receiver.
 let second=service_ipc::request_at(&address,serde_json::json!({"method":"snapshot"}))?;assert_eq!(second["cvd"]["regular"],remote["cvd"]["regular"]);
 let r=&remote["cvd"]["regular"];let total=r["buy"].as_f64().unwrap()+r["sell"].as_f64().unwrap()+r["unknown"].as_f64().unwrap();assert!((total-2272.698918).abs()<1e-8);
 let result=serde_json::json!({"status":"passed","symbol":recording.symbol,"inputRows":recording.records.len(),"regularVolume":total,"regularDelta":r["delta"],"roundtripChangedQuantities":0,"clientDisconnectStopsReceiver":false,"scope":"Real retained historical recording; independent process connection simulation, not current market delivery","reading":remote});
 let file=std::fs::OpenOptions::new().write(true).create_new(true).open(&args[2])?;serde_json::to_writer_pretty(&file,&result)?;file.sync_all()?;println!("{}",serde_json::json!({"status":"passed","regularVolume":total,"clientDisconnectStopsReceiver":false}));Ok(())
}
