//! Actual candidate AMT/CVD engines versus recorded official aggregates; offline only.
use massive_feed::{auction_live,auction_session};use serde_json::{json,Value};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();if args.len()!=3{return Err("Uso: minute-reconciliation.json resultado-nuevo".into());}
 let report:Value=serde_json::from_slice(&std::fs::read(&args[1])?)?;let mut results=vec![];
 for window in report["windows"].as_array().ok_or("Ventanas ausentes")?{
  let replay_started=std::time::Instant::now();
  let symbol=window["symbol"].as_str().ok_or("Activo ausente")?;let start=window["startMs"].as_u64().ok_or("Inicio ausente")?;let end=window["endMsExclusive"].as_u64().ok_or("Fin ausente")?;
  let mut profile=auction_live::Engine::default();let mut cvd=snipers_core::cvd::Engine::analysis();let mut excluded=0;
  for page in window["pages"]["trades"].as_array().unwrap(){let data:Value=serde_json::from_slice(&std::fs::read(page["file"].as_str().unwrap())?)?;
   for raw in data["results"].as_array().unwrap(){let mut row=raw.clone();for key in ["sip_timestamp","participant_timestamp","trf_timestamp"]{if let Some(text)=row[key].as_str(){row[key]=json!(text.parse::<u64>()?);}}
    let Some((price,size,at,sequence))=auction_session::audited_reference_trade(&row,start,end-1)?else{excluded+=1;continue};
    let conditions:Vec<u32>=row["conditions"].as_array().into_iter().flatten().map(|c|c.as_u64().unwrap() as u32).collect();
    profile.trade(symbol,price,size,at,Some(sequence),&conditions,at);cvd.trade(symbol,price,size,at,Some(sequence),&conditions,at);
   }
  }
  let day=snipers_core::calendar::date_at(start).ok_or("Fecha inválida")?;let snipers_core::calendar::Day::Open(session)=snipers_core::calendar::schedule(day)else{return Err("Calendario de ventana no confirmado".into())};
  let regular=start>=session.open_ms&&start<session.close_ms;let pre=start<session.open_ms;
  let amt=profile.snapshot(symbol,end-1,true,vec![]);let delta=cvd.snapshot(symbol,end-1,true);let period=if pre{&delta.premarket}else{&delta.regular};let volume=period.buy+period.sell+period.unknown;
  let official: f64=window["officialVolume"].as_str().ok_or("Volumen ausente")?.parse()?;let tolerance=(official.abs()*1e-12).max(1e-8);
  let amt_volume=amt.as_ref().and_then(|a|a["totalVolume"].as_f64()).unwrap_or(0.);let expected_amt=if regular{official}else{0.};let expected_cvd=if start<session.close_ms{official}else{0.};let unknown=period.unknown;let passed=(amt_volume-expected_amt).abs()<=tolerance&&(volume-expected_cvd).abs()<=tolerance;
  results.push(json!({"symbol":symbol,"startMs":start,"replayElapsedMs":replay_started.elapsed().as_secs_f64()*1000.,"officialVolume":official,"expectedAmtForSession":expected_amt,"expectedCvdForSession":expected_cvd,"amtVolume":amt_volume,"cvdVolume":volume,"cvdUnknown":unknown,"excludedMarkedOrPolicyRows":excluded,"absoluteTolerance":tolerance,"passed":passed,"amt":amt,"cvd":delta,"scope":"Actual Rust parser and candidate engines on complete REST minute; AMT RTH only, CVD premarket/RTH only; no quotes in this probe, so no aggressor-direction validation"}));
 }
 let all=results.iter().all(|r|r["passed"]==true);let value=json!({"version":snipers_core::stock_volume_policy::ID,"windows":results,"status":if all{"passed"}else{"failed"},"runtimeModified":false,"scope":"Full minute candidate volume validation only"});
 let output=std::fs::OpenOptions::new().write(true).create_new(true).open(&args[2])?;serde_json::to_writer_pretty(output,&value)?;println!("{}",json!({"status":value["status"],"windows":value["windows"].as_array().unwrap().len()}));if !all{return Err("Candidate differs from official volume; evidence saved".into());}Ok(())
}
