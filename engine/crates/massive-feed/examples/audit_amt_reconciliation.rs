//! Read-only historical REST comparison. No runtime state or orders.
use massive_feed::{auction_live::Engine,auction_reconciliation,auction_session,credentials::Credentials,recording::Recording,telemetry::Telemetry,trade_units,Event,Market};
#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error>> {
 let args:Vec<_>=std::env::args().collect();if args.len()!=3{return Err("Uso: audit_amt_reconciliation recording.json evidence-new.json".into());}
 let recording=Recording::read(std::path::Path::new(&args[1]))?;
 let mut engine=Engine::default();let mut telemetry=Telemetry::default();let mut now=0;let mut last=0;
 for row in &recording.records {
  if !matches!(row.market,Market::Stocks){continue;}
  let symbol=match &row.event{Event::Trade{sym,..}|Event::Quote{sym,..}=>sym,_=>continue};
  if symbol!=&recording.symbol{continue;}now=now.max(row.received_ms);
  if !telemetry.observe(row.market,&row.event,row.received_ms){continue;}
  if let Event::Trade{sym,p,s,t,q,metadata}=&row.event {
   last=last.max(*t);if let Some(size)=trade_units::stock_trade_size(*s,metadata.ds.as_deref()){engine.trade(sym,*p,size,*t,*q,&metadata.c,row.received_ms);}
  }
 }
 now=now.max(last)+1000;
 let snapshot=engine.snapshot(&recording.symbol,now,true,vec![]).ok_or("Sin perfil causal de referencia")?;
 let start=snapshot["observedSinceMs"].as_u64().ok_or("Sin inicio observado")?;
 let request=engine.reconciliation_window(start,last+1).ok_or("Captura insuficiente para ventana")?;
 let key=Credentials::load()?;let timer=std::time::Instant::now();
 let reference=auction_session::fetch_window(&key,&request,||true).await?;
 let report=auction_reconciliation::compare(&request,&reference.trades,reference.excluded_rows,reference.unresolved_corrections,now)?;
 let evidence=serde_json::json!({"symbol":recording.symbol,"scope":"Historical retained tape vs independent REST; sampled window only","auditRunAtUtcMs":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis(),"window":{"startMs":request.start_ms,"endMs":request.end_ms},"live":request.live,"reference":reference,"report":report,"referenceElapsedMs":timer.elapsed().as_secs_f64()*1000.});
 let file=std::fs::OpenOptions::new().write(true).create_new(true).open(&args[2])?;serde_json::to_writer_pretty(file,&evidence)?;
 println!("{}",serde_json::json!({"status":report.status,"output":args[2],"liveEvents":report.live_events,"restEvents":report.rest_events,"wholeSessionCertified":false}));Ok(())
}
