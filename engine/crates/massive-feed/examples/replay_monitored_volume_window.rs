//! Offline replay of received events; no sockets, credentials or execution.
use std::collections::BTreeMap;
use snipers_core::flow::{Contract,Engine};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let dir=std::path::PathBuf::from(std::env::args().nth(1).ok_or("Audit directory required")?);
 let results:serde_json::Value=serde_json::from_slice(&std::fs::read(dir.join("results.json"))?)?;
 let now=results["endMs"].as_u64().ok_or("Missing end")?-1;
 let symbols=["QQQ","NVDA","AMZN","AAPL","NBIS"];
 let state=massive_feed::live::new_state_for("QQQ")?;
 state.lock().unwrap().research_symbols=symbols.iter().map(|s|s.to_string()).collect();
 let mut recordings=Vec::new();
 for i in 0..1000 {let path=dir.join(format!("replay-{i}.json"));if !path.exists(){break;}recordings.push(massive_feed::recording::Recording::read(&path)?);}
 let raw:Vec<serde_json::Value>=std::fs::read_to_string(dir.join("window-events.jsonl"))?.lines().map(serde_json::from_str).collect::<Result<_,_>>()?;
 let mut engines:BTreeMap<String,Engine>=symbols.iter().map(|s|(s.to_string(),Engine::default())).collect();
 for symbol in symbols {
  let mut contracts=BTreeMap::new();
  for row in &raw{
   let Some(ticker)=row["event"].get("sym").and_then(|s|s.as_str()) else {continue};
   if ticker.starts_with(&format!("O:{symbol}"))&&ticker.len()==symbol.len()+17 {
    let kind=if ticker.as_bytes()[ticker.len()-9]==b'C'{"call"}else{"put"};
    contracts.insert(ticker.to_string(),Contract{ticker:ticker.into(),kind:kind.into(),strike:ticker[ticker.len()-8..].parse::<f64>()?/1000.,expiration:"2026-10-13".into(),shares_per_contract:None,premium_multiplier:None});
   }
  }
  engines.get_mut(symbol).unwrap().set_contracts(contracts);
 }
 let mut rows=0;
 for recording in &recordings {
  massive_feed::live::ingest_recording(&state,recording);rows+=recording.records.len();
 }
 for row in &raw {
   let e=&row["event"];let Some(ticker)=e.get("sym").and_then(|s|s.as_str()) else{continue};
   let Some(symbol)=symbols.iter().find(|s|ticker.starts_with(&format!("O:{s}"))&&ticker.len()==s.len()+17) else{continue};
   let engine=engines.get_mut(*symbol).unwrap();let t=e["t"].as_u64().ok_or("Missing timestamp")?;
   if e["ev"]=="Q"&&t<=now{engine.quote(ticker,e["bp"].as_f64().unwrap_or(0.),e["ap"].as_f64().unwrap_or(0.),t);}
   else if e["ev"]=="T"&&t>=results["startMs"].as_u64().unwrap()&&t<results["endMs"].as_u64().unwrap(){engine.trade(ticker,e["p"].as_f64().unwrap_or(0.),e["s"].as_f64().unwrap_or(0.),t,row["receivedMs"].as_u64().ok_or("Missing receive time")?);}
 }
 let frames:Vec<_>=symbols.iter().map(|s|massive_feed::live::bot_frame(&state,s,now)).collect();
 let options:BTreeMap<_,_>=engines.iter_mut().map(|(s,e)|(s.clone(),e.snapshot(now))).collect();
 let output=serde_json::json!({"historical":true,"rows":rows,"at":now,"frames":frames,"options":options,"ordersSent":0,"optionScope":"Received-contract union replay; metadata premium multiplier unverified, no dollar-premium claim; not live rotating contract membership"});
 std::fs::write(dir.join("native-replay.json"),serde_json::to_vec_pretty(&output)?)?;
 println!("rows={rows}");Ok(())
}
