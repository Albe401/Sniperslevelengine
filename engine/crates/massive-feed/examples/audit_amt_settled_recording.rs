//! Offline clock settlement probe; never opens provider connections.
use massive_feed::{recording::Recording,telemetry::Telemetry,auction_live,trade_units,Event,Market};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();if args.len()!=3{return Err("Uso: registro resultado-nuevo".into());}
 let recording=Recording::read(std::path::Path::new(&args[1]))?;let mut profile=auction_live::Engine::default();let mut telemetry=Telemetry::default();let mut now=0;
 for row in &recording.records{if !matches!(row.market,Market::Stocks){continue;}let symbol=match &row.event{Event::Trade{sym,..}|Event::Quote{sym,..}=>sym,_=>continue};if symbol!=&recording.symbol{continue;}now=now.max(row.received_ms);if !telemetry.observe(row.market,&row.event,row.received_ms){continue;}if let Event::Trade{sym,p,s,t,q,metadata}=&row.event{if let Some(size)=trade_units::stock_trade_size(*s,metadata.ds.as_deref()){profile.trade(sym,*p,size,*t,*q,&metadata.c,row.received_ms);}}}
 let at_last=profile.snapshot(&recording.symbol,now,true,vec![]);let after_clock=profile.snapshot(&recording.symbol,now+1000,true,vec![]);
 let result=serde_json::json!({"scope":"Same retained historical recording; advance audit clock by 1000 ms, no invented events or provider calls","lastReceivedMs":now,"atLastReceived":at_last,"afterClockSettled":after_clock});
 let file=std::fs::OpenOptions::new().write(true).create_new(true).open(&args[2])?;serde_json::to_writer_pretty(file,&result)?;Ok(())
}
