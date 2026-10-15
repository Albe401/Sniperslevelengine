use massive_feed::analyst_targets::{self as fmp,Targets,TTL};
use serde::{Deserialize,Serialize};
use std::{collections::BTreeMap,path::Path,sync::{Arc,Mutex}};
use tauri::Manager;

#[derive(Default)]
pub struct Task(pub Arc<Mutex<Option<Report>>>);
#[derive(Clone,Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Report {
 pub configured:bool, pub running:bool, pub status:String, pub total:usize,
 pub rows:BTreeMap<String,Targets>, pub attempts:Vec<u64>, pub finished_ms:u64,
}
fn save(path:&Path,report:&Report)->Result<(),String> {
 let mut saved=report.clone();saved.running=false;
 let bytes=serde_json::to_vec(&saved).map_err(|_|"No se pudo guardar la caché FMP")?;
 let tmp=path.with_extension("tmp");
 std::fs::write(&tmp,bytes).map_err(|_|"No se pudo guardar la caché FMP")?;
 std::fs::rename(tmp,path).map_err(|_|"No se pudo guardar la caché FMP".to_string())
}
fn recent(attempts:&[u64],now:u64)->usize {attempts.iter().filter(|t|**t>now||now.saturating_sub(**t)<TTL).count()}

#[tauri::command]
pub fn get_fmp_targets(app:tauri::AppHandle,task:tauri::State<'_,Task>)->Result<Report,String> {
 let dir=app.path().app_local_data_dir().map_err(|_|"Directorio FMP no disponible")?;
 std::fs::create_dir_all(&dir).map_err(|_|"Directorio FMP no disponible")?;
 let path=dir.join("fmp-basic-targets-v1.json");
 let mut guard=task.0.lock().map_err(|_|"Estado FMP ocupado")?;
 if guard.is_none() {
  let report=match std::fs::read(&path) {
   Ok(bytes)=>serde_json::from_slice(&bytes).map_err(|_|"Caché FMP inválida; revisar antes de consumir la cuota")?,
   Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Report::default(),
   Err(_)=>return Err("No se pudo leer la caché FMP".into()),
  };*guard=Some(report);
 }
 let report=guard.as_mut().unwrap();report.configured=fmp::configured();report.total=fmp::companies().len();
 if !report.configured {report.status="Configura FMP_API_KEY para consultar la cobertura Basic".into();return Ok(report.clone());}
 let now=massive_feed::live::now_ms();
 if !report.running && (report.finished_ms==0 || (report.finished_ms<=now&&!fmp::same_day(report.finished_ms,now))) {
  report.running=true;report.status="Consultando cobertura · mega cap primero".into();
  let shared=task.0.clone();
  tauri::async_runtime::spawn(async move {
   let mut failures=0;
   for symbol in fmp::companies() {
    let now=massive_feed::live::now_ms();
    {
     let Ok(mut guard)=shared.lock() else {return};let r=guard.as_mut().unwrap();
     if r.rows.get(&symbol).is_some_and(|t|t.fresh(now)){continue;}
     r.attempts.retain(|t|*t>now||now.saturating_sub(*t)<TTL);
     // Reserve 50 of Basic's 250 daily calls for other FMP uses.
     if recent(&r.attempts,now)>=200 {r.status="Presupuesto diario de objetivos alcanzado".into();break;}
     r.attempts.push(now);
     r.rows.insert(symbol.clone(),Targets::unavailable(&symbol,"Consulta pendiente de completar",now));
     if let Err(e)=save(&path,r){r.status=e;r.running=false;return;}
    }
    let row=fmp::fetch(&symbol).await;
    let stop=matches!(row.status.as_str(),"FMP: clave no válida"|"FMP: cuota agotada");
    if row.status.starts_with("FMP:") {failures+=1;} else {failures=0;}
    {
     let Ok(mut guard)=shared.lock() else {return};let r=guard.as_mut().unwrap();
     r.status=format!("Cobertura comprobada: {} / {}",r.rows.len(),r.total);
     if stop||failures>=3 {r.status=row.status.clone();}
     r.rows.insert(symbol,row);
     if let Err(e)=save(&path,r){r.status=e;r.running=false;return;}
    }
    if stop||failures>=3 {break;}
    tokio::time::sleep(std::time::Duration::from_millis(350)).await;
   }
   if let Ok(mut guard)=shared.lock(){let r=guard.as_mut().unwrap();r.running=false;r.finished_ms=massive_feed::live::now_ms();if let Err(e)=save(&path,r){r.status=e;}}
  });
 }
 Ok(report.clone())
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn rolling_budget_survives_midnight_and_future_clock() {
  assert_eq!(recent(&[100,TTL+100],TTL+99),2);
  assert_eq!(recent(&[100,TTL+100],TTL+100),1);
 }
 #[test] fn cache_replacement_keeps_attempts_across_restart() {
  let path=std::env::temp_dir().join(format!("snipers-fmp-test-{}-{}.json",std::process::id(),massive_feed::live::now_ms()));
  let mut r=Report::default();r.running=true;r.attempts.push(100);save(&path,&r).unwrap();
  r.attempts.push(200);
  let at=snipers_core::calendar::rfc3339_ms("2026-10-02T18:00:00Z").unwrap();
  let mut target=Targets::unavailable("NVDA","Disponible",at);target.low=Some(100.);target.average=Some(120.);target.high=Some(140.);
  r.rows.insert("NVDA".into(),target);save(&path,&r).unwrap();
  let restored:Report=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
  assert_eq!(restored.attempts,vec![100,200]);assert!(!restored.running);
  let target=&restored.rows["NVDA"];assert_eq!(target.average,Some(120.));assert!(target.fresh(at+1000));assert!(!target.fresh(at+TTL));
  std::fs::remove_file(path).unwrap();
 }
}
