//! Gamma map observations, isolated from the trading and generic learning ledgers.
use serde_json::{json,Value};
use snipers_store::learning::LearningStore;
use massive_feed::live;
use std::path::Path;

pub fn validate(p:&Value,symbol:&str,now:u64)->Result<(),String>{
 let fail=||"Lectura gamma inválida o caducada".to_owned();
 if p["schema"]!="gamma-map-v1"||p["symbol"]!=symbol||p.to_string().len()>65536{return Err(fail());}
 let ts=p["capturedMs"].as_u64().ok_or_else(fail)?;
 if ts>now||now-ts>3000{return Err(fail());}
 let kind=p["movement"]["kind"].as_str().ok_or_else(fail)?;
 if !["waiting","mixed","forming","slowing","reversal","aligned"].contains(&kind){return Err(fail());}
 let direction=p["movement"]["direction"].as_i64().ok_or_else(fail)?;
 if !(-1..=1).contains(&direction)||(!["aligned","reversal"].contains(&kind)&&direction!=0){return Err(fail());}
 let levels=p["levels"].as_array().ok_or_else(fail)?;
 if levels.len()!=4{return Err(fail());}
 let mut ids=std::collections::HashSet::new();
 let mut dominant=std::collections::HashSet::new();
 for l in levels {
  let id=l["id"].as_str().ok_or_else(fail)?;
  if !["put","call","main","liquidity"].contains(&id)||!ids.insert(id){return Err(fail());}
  for field in ["importance","force","liquidity"]{
   if !l[field].is_null()&&!l[field].as_f64().is_some_and(|v|v.is_finite()&&(0.0..=100.000001).contains(&v)){return Err(fail());}
  }
  if !l["strike"].is_null()&&!l["strike"].as_f64().is_some_and(|v|v.is_finite()&&v>0.0){return Err(fail());}
  if l["dominant"].as_bool().ok_or_else(fail)?{
   if !p["complete"].as_bool().unwrap_or(false)||l["force"].is_null()||l["strike"].is_null(){return Err(fail());}
   dominant.insert(l["strike"].to_string());
  }
 }
 if dominant.len()>2{return Err(fail());} Ok(())
}
pub fn record(directory:&Path,s:&live::LiveState,p:Value,now:u64)->Result<(),String>{
 validate(&p,&s.symbol,now)?;
 if s.published_ms>now||now-s.published_ms>3000{return Err("Feed gamma caducado".into());}
 let mut o=super::learning::observation(s,now);
 let comparable=s.option_snapshot_status.horizons.iter().find(|h|h.mode=="DAY")
  .is_some_and(|h|Some(h.observed_ms)==p["snapshotMs"].as_u64()&&h.structure.complete&&h.exposure.gex_complete);
 record_observation(directory,o,p,comparable,now)
}
pub fn record_observation(directory:&Path,mut o:snipers_store::learning::Observation,p:Value,comparable:bool,now:u64)->Result<(),String>{
 validate(&p,&o.symbol,now)?;
 if o.observed_ms>now||now-o.observed_ms>3000{return Err("Observación gamma caducada".into());}
 o.eligible &= comparable&&p["complete"]==true&&["aligned","reversal"].contains(&p["movement"]["kind"].as_str().unwrap_or(""));
 if !o.eligible && o.quality=="READY" {o.quality="GAMMA_READING_PENDING".into();}
 o.context["gamma_map"]=p;
 o.context["collector_version"]=json!("gamma-map-v1");
 std::fs::create_dir_all(directory).map_err(|_|"Carpeta gamma no disponible")?;
 let result=LearningStore::open(&directory.join("gamma-map-learning-v1.sqlite"))
  .and_then(|mut store|store.record_interval(&o,1000));
 super::learning::write_json(directory,"gamma-learning-status.json",&json!({"observed_ms":now,"symbol":o.symbol,"recorded":result.is_ok(),"eligible":o.eligible,"quality":o.quality}))
  .map_err(|_|"No se pudo guardar el estado gamma")?;
 result.map_err(|_|"No se pudo guardar aprendizaje gamma".into())
}

#[cfg(test)]
mod tests {
 use super::*;
 fn payload(now:u64)->Value {json!({"schema":"gamma-map-v1","symbol":"QQQ","capturedMs":now,"complete":true,"movement":{"kind":"reversal","direction":-1,"headline":"CALL A PUT"},"levels":[
  {"id":"put","strike":100,"force":80,"importance":70,"liquidity":50,"dominant":true},
  {"id":"call","strike":105,"force":60,"importance":60,"liquidity":50,"dominant":false},
  {"id":"main","strike":100,"force":80,"importance":70,"liquidity":50,"dominant":true},
  {"id":"liquidity","strike":102,"force":79,"importance":70,"liquidity":50,"dominant":true}]})}
 #[test] fn shared_roles_are_valid_but_stale_foreign_and_invalid_scores_are_rejected(){
  let now=10000;assert!(validate(&payload(now),"QQQ",now).is_ok());
  assert!(validate(&payload(now),"AAPL",now).is_err());
  assert!(validate(&payload(now),"QQQ",now+3001).is_err());
  assert!(validate(&payload(now+1),"QQQ",now).is_err());
  let mut p=payload(now);p["levels"][0]["force"]=json!(101);assert!(validate(&p,"QQQ",now).is_err());
  let mut p=payload(now);p["levels"][1]["dominant"]=json!(true);assert!(validate(&p,"QQQ",now).is_err());
  let mut p=payload(now);p["complete"]=json!(false);assert!(validate(&p,"QQQ",now).is_err());
 }
 #[test] fn gamma_context_persists_and_delayed_outcomes_are_observed(){
  let dir=tempfile::tempdir().unwrap();let path=dir.path().join("gamma.sqlite");
  let now=snipers_core::calendar::rfc3339_ms("2026-10-01T15:00:00Z").unwrap();
  let mut store=LearningStore::open(&path).unwrap();
  let observation=|t,price|snipers_store::learning::Observation{symbol:"QQQ".into(),observed_ms:t,price:Some(price),price_ms:Some(t),eligible:true,quality:"READY".into(),context:json!({"gamma_map":payload(t),"collector_version":"gamma-map-v1","automatic_model_updates":false})};
  store.record_interval(&observation(now,100.0),1000).unwrap();
  store.record_interval(&observation(now+2000,99.9),1000).unwrap();
  store.record_interval(&observation(now+300000,99.0),1000).unwrap();
  let report=snipers_store::learning_report::read(&path,Some("QQQ"),now+300000).unwrap();
  assert_eq!(report["observations"],3);assert_eq!(report["outcomes"][0]["observed"],1);
  assert_eq!(report["recent"][0]["side"],"CALL A PUT");
  assert!(store.record_interval(&observation(now,100.0),0).is_err());
 }
}

#[cfg(test)]
mod integration_tests {
 use super::*;
 #[test] fn backend_records_incomplete_reading_without_making_it_eligible(){
  let directory=tempfile::tempdir().unwrap();let shared=live::new_state();
  let mut s=live::snapshot(&shared);let now=snipers_core::calendar::rfc3339_ms("2026-10-01T15:00:00Z").unwrap();
  s.symbol="QQQ".into();s.published_ms=now;
  let levels:Vec<Value>=["put","main","liquidity","call"].iter().map(|id|json!({"id":id,"strike":null,"force":null,"importance":null,"liquidity":null,"dominant":false})).collect();
  let p=json!({"schema":"gamma-map-v1","symbol":"QQQ","capturedMs":now,"snapshotMs":0,"complete":false,"movement":{"kind":"waiting","direction":0,"headline":"LECTURA PENDIENTE"},"levels":levels});
  record(directory.path(),&s,p.clone(),now).unwrap();
  let report=snipers_store::learning_report::read(&directory.path().join("gamma-map-learning-v1.sqlite"),None,now).unwrap();
  assert_eq!(report["observations"],1);assert_eq!(report["eligible"],0);
  assert_eq!(report["recent"][0]["side"],"LECTURA PENDIENTE");
  assert!(record(directory.path(),&s,p,now+3001).is_err());
 }
}
