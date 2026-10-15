//! Real retained option tape replay, quantity only. Premium multiplier is not inferred.
use massive_feed::{recording::Recording,telemetry::Telemetry,Event,Market};
use snipers_core::flow::{Engine,Contract};
use std::collections::BTreeMap;
fn main()->Result<(),Box<dyn std::error::Error>> {
 let args:Vec<_>=std::env::args().collect();if args.len()!=3{return Err("Uso: registro resultado-nuevo".into());}
 let recording=Recording::read(std::path::Path::new(&args[1]))?;let mut contracts=BTreeMap::new();
 for row in &recording.records {
  if !matches!(row.market,Market::Options){continue;}
  let ticker=match &row.event{Event::Quote{sym,..}|Event::Trade{sym,..}=>sym,_=>continue};
  if snipers_core::options::underlying(ticker).is_none(){continue;}
  let tail=&ticker[ticker.len()-15..];
  let c=Contract{ticker:ticker.clone(),kind:if &tail[6..7]=="C"{"call"}else{"put"}.into(),strike:tail[7..].parse::<f64>()?/1000.,expiration:format!("20{}-{}-{}",&tail[..2],&tail[2..4],&tail[4..6]),shares_per_contract:None,premium_multiplier:None};contracts.insert(ticker.clone(),c);
 }
 let mut engine=Engine::default();engine.set_contracts(contracts);let mut telemetry=Telemetry::default();let mut now=0;let mut last=0;
 for row in &recording.records {
  if !matches!(row.market,Market::Options){continue;}now=now.max(row.received_ms);
  if !telemetry.observe(row.market,&row.event,row.received_ms){continue;}
  match &row.event {
   Event::Quote{sym,bp,ap,bs,ask_size,t}=>engine.quote_with_depth(sym,*bp,*ap,*t,Some(*bs),Some(*ask_size)),
   Event::Trade{sym,p,s,t,..}=>{last=last.max(*t);engine.trade(sym,*p,*s,*t,row.received_ms);},
   _=>(),
  }
 }
 let snapshot=engine.snapshot(now.max(last));
 let file=std::fs::OpenOptions::new().write(true).create_new(true).open(&args[2])?;
 serde_json::to_writer_pretty(file,&serde_json::json!({"scope":"Retained option tape only, multiple underlyings; not same asset/window as prior 73 percent, not market reception certification","quantityUnit":"contracts","premiumMultiplier":"not_inferred","snapshot":snapshot}))?;
 println!("{}",serde_json::to_value(&snapshot.windows[0].classification_quality)?);Ok(())
}
