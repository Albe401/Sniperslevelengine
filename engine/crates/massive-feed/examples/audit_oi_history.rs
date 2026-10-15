//! Read-only replay of stored OI observations and daily change calculations.
use std::{collections::BTreeMap,path::PathBuf};
use snipers_core::options::{Observation,changes};
use serde_json::json;
fn collect(dir:&std::path::Path,files:&mut Vec<PathBuf>)->std::io::Result<()>{
 for entry in std::fs::read_dir(dir)?{let p=entry?.path();if p.is_dir(){collect(&p,files)?;}else if p.extension().is_some_and(|e|e=="json"){files.push(p);}}Ok(())
}
fn main()->Result<(),Box<dyn std::error::Error>>{
 let dir=PathBuf::from(std::env::args().nth(1).ok_or("OI directory required")?);
 let mut files=Vec::new();collect(&dir,&mut files)?;files.sort_by(|a,b|a.file_name().cmp(&b.file_name()));
 let mut counts:BTreeMap<String,u64>=BTreeMap::new();let mut by_symbol:BTreeMap<String,u64>=BTreeMap::new();
 let mut latest:BTreeMap<String,Observation>=BTreeMap::new();let mut prior:BTreeMap<String,Observation>=BTreeMap::new();let mut errors=vec![];
 for p in &files{
  // This directory also contains caches for gap/news/other workers, not OI observations.
  if !p.file_name().and_then(|n|n.to_str()).is_some_and(|n|n.starts_with("oi-")){
   *counts.entry("otherWorkerCacheFiles".into()).or_default()+=1;
   if p.file_name().and_then(|n|n.to_str()).is_some_and(|n|n.starts_with("dex-")){
    let bytes=std::fs::read(p)?;let value:serde_json::Value=serde_json::from_slice(&bytes)?;
    let root=value["symbol"].as_str().unwrap_or("");let at=value["written_ms"].as_u64().unwrap_or(0);
    match snipers_core::dex_change::Tracker::restore(&bytes,root,at){
     Ok(_)=>{*counts.entry("dexBaselinesRestored".into()).or_default()+=1;}
     Err(e)=>errors.push(json!({"file":p.file_name().and_then(|v|v.to_str()),"reason":e})),
    }
    *counts.entry("dexStoredSamples".into()).or_default()+=value["samples"].as_array().map(|v|v.len() as u64).unwrap_or(0);
   }
   continue;
  }
  let obs=match massive_feed::oi_store::read(p){Ok(v)=>v,Err(e)=>{errors.push(json!({"file":p.file_name().and_then(|v|v.to_str()),"reason":e}));continue}};
  *counts.entry("observations".into()).or_default()+=1;*by_symbol.entry(obs.symbol.clone()).or_default()+=1;
  if let Some(old)=latest.get(&obs.symbol){
   if old.observed_ms>obs.observed_ms{errors.push(json!({"file":p.file_name().and_then(|v|v.to_str()),"reason":"out of chronological order"}));continue;}
   if old.observed_date<obs.observed_date{prior.insert(obs.symbol.clone(),old.clone());}
  }
  let history=prior.get(&obs.symbol).map(std::slice::from_ref).unwrap_or(&[]);
  for change in changes(&obs,history){
   *counts.entry("changesCalculated".into()).or_default()+=u64::from(change.change.is_some());
   *counts.entry("changesWaitingPriorDay".into()).or_default()+=u64::from(change.change.is_none());
  }
  for c in obs.contracts.values(){
   *counts.entry("contracts".into()).or_default()+=1;
   *counts.entry("missingGamma".into()).or_default()+=u64::from(c.gamma.is_none());
   *counts.entry("missingDelta".into()).or_default()+=u64::from(c.delta.is_none());
   *counts.entry("missingOi".into()).or_default()+=u64::from(c.open_interest.is_none());
   *counts.entry("missingMultiplier".into()).or_default()+=u64::from(c.premium_multiplier.is_none());
   *counts.entry("freshUnderlying5s".into()).or_default()+=u64::from(c.underlying_ms.is_some_and(|t|t<=obs.observed_ms&&obs.observed_ms-t<=5000));
   *counts.entry("futureUnderlying".into()).or_default()+=u64::from(c.underlying_ms.is_some_and(|t|t>obs.observed_ms));
   if snipers_core::options::underlying(&c.ticker)!=Some(obs.symbol.as_str()){
    *counts.entry("foreignRoot".into()).or_default()+=1;
   }
  }
  latest.insert(obs.symbol.clone(),obs);
 }
 println!("{}",serde_json::to_string_pretty(&json!({"version":"oi-history-replay-v1","files":files.len(),"counts":counts,"bySymbol":by_symbol,"errors":errors,"executionEnabled":false,"validated":false,"scope":"All saved OI snapshots: native schema validation, identity/Greek coverage and native changes against previously observed days. No tick OI inferred; repeated contracts are not independent samples. No exposure or price-direction performance claim."}))?);Ok(())
}
