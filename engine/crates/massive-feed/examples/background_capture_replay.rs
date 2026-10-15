fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();if args.len()!=3{return Err("Uso: <grabacion> <journal-nuevo>".into());}
 let recording=massive_feed::recording::Recording::read(std::path::Path::new(&args[1]))?;
 let (capture,writer)=massive_feed::background_capture::open(std::path::Path::new(&args[2]),10000)?;
 for row in &recording.records{capture.offer(row.market,&row.event,row.received_ms);}
 drop(writer);let health=capture.health();
 assert_eq!(health["written"].as_u64(),Some(recording.records.len() as u64));assert_eq!(health["dropped"],0);assert_eq!(health["writeFailed"],false);
 println!("{}",serde_json::json!({"scope":"Replay de grabación real parcial, no eventos nuevos del servicio","symbol":recording.symbol,"recordedEvents":recording.records.len(),"overwrittenRecords":recording.overwritten_records,"capture":health}));Ok(())
}
