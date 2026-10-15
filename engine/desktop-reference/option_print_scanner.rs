//! Local option-print history, separate from stock dark-pool blocks and bot orders.
use massive_feed::option_print_scanner::{self, Print, Report};
use std::{collections::BTreeSet, path::{Path,PathBuf}, sync::atomic::{AtomicBool,Ordering}};
use tauri::Manager;

static BUSY:AtomicBool=AtomicBool::new(false);

fn path(root:&Path,date:&str,symbol:&str)->Result<PathBuf,String>{
 if !option_print_scanner::valid_symbol(symbol)||date.len()!=10||!date.bytes().enumerate().all(|(i,b)|if i==4||i==7{b==b'-'}else{b.is_ascii_digit()}){
  return Err("Activo o fecha inválidos".into());
 }
 Ok(root.join("option-prints").join(date).join(format!("{symbol}.json")))
}
fn load(root:&Path,date:&str,symbol:&str)->Result<Option<Report>,String>{
 let file=path(root,date,symbol)?;
 if !file.is_file(){return Ok(None)}
 let metadata=std::fs::metadata(&file).map_err(|_|"No se pudo leer el historial de opciones")?;
 if metadata.len()>5_000_000{return Err("Historial de opciones demasiado grande".into());}
 let bytes=std::fs::read(file).map_err(|_|"No se pudo leer el historial de opciones")?;
 let report:Report=serde_json::from_slice(&bytes).map_err(|_|"Historial de opciones inválido")?;
 if report.date!=date||report.symbol!=symbol{return Err("Historial de opciones de otro activo o fecha".into());}
 Ok(Some(report))
}
fn print_key(row:&Print)->String{
 format!("{}:{}:{}:{}:{}:{}",row.ticker,row.observed_ms(),row.sequence.unwrap_or(0),row.size,row.price,row.exchange)
}
fn save(root:&Path,mut report:Report)->Result<Report,String>{
 let file=path(root,&report.date,&report.symbol)?;
 if let Some(old)=load(root,&report.date,&report.symbol)?{
  if old.min_contracts==report.min_contracts{
   report.rows.extend(old.rows);
   let mut seen=BTreeSet::new();report.rows.retain(|row|seen.insert(print_key(row)));
   report.rows.sort_by(|a,b|b.observed_ms().cmp(&a.observed_ms()));report.rows.truncate(1000);
  }
 }
 std::fs::create_dir_all(file.parent().ok_or("Ruta de opciones inválida")?).map_err(|_|"No se pudo crear el historial de opciones")?;
 let bytes=serde_json::to_vec(&report).map_err(|_|"No se pudo guardar el escáner de opciones")?;
 std::fs::write(file,bytes).map_err(|_|"No se pudo guardar el escáner de opciones".to_string())?;
 Ok(report)
}

#[tauri::command]
pub async fn scan_option_prints(app:tauri::AppHandle,symbol:String,min_contracts:u64)->Result<Report,String>{
 if BUSY.swap(true,Ordering::SeqCst){return Err("Espera a que termine la consulta de opciones actual".into());}
 struct Guard;impl Drop for Guard{fn drop(&mut self){BUSY.store(false,Ordering::SeqCst)}}
 let _guard=Guard;
 let report=option_print_scanner::scan(&symbol,min_contracts).await?;
 let root=app.path().app_local_data_dir().map_err(|_|"Carpeta local no disponible")?;
 tauri::async_runtime::spawn_blocking(move||save(&root,report)).await.map_err(|_|"No se pudo guardar la lectura de opciones".to_string())?
}

#[tauri::command]
pub async fn get_option_prints(app:tauri::AppHandle,symbol:String,previous:bool)->Result<Option<Report>,String>{
 let symbol=symbol.trim().to_ascii_uppercase();
 if !option_print_scanner::valid_symbol(&symbol){return Err("Indica un activo válido".into());}
 let root=app.path().app_local_data_dir().map_err(|_|"Carpeta local no disponible")?;
 let now=massive_feed::live::now_ms();
 tauri::async_runtime::spawn_blocking(move||{
  for offset in if previous{1..=7}else{0..=0}{
   let date=snipers_core::calendar::date_at(now.saturating_sub(offset*86_400_000)).ok_or("Fecha inválida")?.to_string();
   if let Some(report)=load(&root,&date,&symbol)?{return Ok(Some(report));}
  }
  Ok(None)
 }).await.map_err(|_|"No se pudo consultar el historial de opciones".to_string())?
}

#[cfg(test)]
mod tests{
 use super::*;
 fn sample(date:&str)->Report{Report{symbol:"QQQ".into(),date:date.into(),scanned_ms:1,min_contracts:50,chain_examined:100,
  selected_contracts:16,chain_complete:false,trades_complete:true,matching_prints:1,
  rows:vec![Print{ticker:"O:QQQ261008C00500000".into(),side:"CALL".into(),expiration:date.into(),strike:500.,executed_ms:Some(10),sip_ms:11,timestamp_source:option_print_scanner::TimestampSource::Participant,size:50,price:1.,premium:5000.,exchange:46,sequence:Some(1)}],warnings:vec![]}}
 #[test]
 fn saves_and_deduplicates_prints_by_day_and_symbol(){
  let root=tempfile::tempdir().unwrap();let r=sample("2026-10-08");save(root.path(),r.clone()).unwrap();
  let merged=save(root.path(),r).unwrap();assert_eq!(merged.rows.len(),1);
  assert!(load(root.path(),"2026-10-07","QQQ").unwrap().is_none());
  assert_eq!(load(root.path(),"2026-10-08","QQQ").unwrap().unwrap().rows.len(),1);
  assert!(path(root.path(),"2026-10-08","../QQQ").is_err());
 }
 #[test]
 fn sip_only_prints_roundtrip_and_merge_with_legacy_execution_clocks(){
  let root=tempfile::tempdir().unwrap();let r=sample("2026-10-08");save(root.path(),r.clone()).unwrap();
  let mut sip=r;sip.rows[0].executed_ms=None;sip.rows[0].sip_ms=12;
  sip.rows[0].timestamp_source=option_print_scanner::TimestampSource::Sip;
  let merged=save(root.path(),sip.clone()).unwrap();assert_eq!(merged.rows.len(),2);
  assert_eq!(merged.rows[0].observed_ms(),12);assert_eq!(merged.rows[0].executed_ms,None);
  assert_eq!(save(root.path(),sip).unwrap().rows.len(),2);
  let loaded=load(root.path(),"2026-10-08","QQQ").unwrap().unwrap();
  assert_eq!(loaded.rows[0].timestamp_source,option_print_scanner::TimestampSource::Sip);
 }
}
