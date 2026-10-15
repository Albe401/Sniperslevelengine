#[tauri::command]
pub fn set_etf_scanner_enabled(task:tauri::State<'_,crate::LiveTask>,enabled:bool)->Result<(),String>{
 if task.remote.load(std::sync::atomic::Ordering::SeqCst){massive_feed::service_ipc::request(serde_json::json!({"method":"etf_enabled","enabled":enabled}))?;task.etf_scanner_enabled.store(enabled,std::sync::atomic::Ordering::SeqCst);return Ok(());}

    let current=task.state.lock().map_err(|_|"Sesión no disponible")?;
    task.etf_scanner_enabled.store(enabled,std::sync::atomic::Ordering::SeqCst);
    if let Some(state)=current.as_ref(){
        let mut s=state.lock().map_err(|_|"Estado no disponible")?;
        s.set_etf_scanner_enabled(enabled);
    }
    Ok(())
}
#[tauri::command]
pub async fn get_etf_scanner(app:tauri::AppHandle,task:tauri::State<'_,crate::LiveTask>,symbol:String,days:u32)->Result<massive_feed::etf_scanner::Report,String>{
    use tauri::Manager;
    if !matches!(symbol.as_str(),"SPY"|"QQQ") || !matches!(days,0|7|28|30|183){return Err("Universo ETF inválido".into());}
    if !task.etf_scanner_enabled.load(std::sync::atomic::Ordering::SeqCst){return Err("Abre el scanner SPY/QQQ para consultar su cadena".into());}
    let directory=app.path().app_local_data_dir().ok().map(|p|p.join("etf-oi-observations"));
    massive_feed::etf_scanner::fetch(&symbol,days,directory).await.map_err(str::to_owned)
}

#[tauri::command]
pub async fn get_contract_inventory(app:tauri::AppHandle,symbol:String)->Result<massive_feed::etf_scanner::Report,String>{
 use tauri::Manager;
 let directory=app.path().app_local_data_dir().ok().map(|p|p.join("contract-inventory-oi"));
 massive_feed::etf_scanner::fetch_stock(&symbol,183,directory).await.map_err(str::to_owned)
}

#[tauri::command]
pub async fn get_weekly_range_snapshot(symbol:String)->Result<serde_json::Value,String>{
 use snipers_core::calendar::{self,Day};
 let now=massive_feed::live::now_ms();
 let today=calendar::date_at(now).ok_or("Fecha no disponible")?;
 let active=matches!(calendar::schedule(today),Day::Open(ref s) if now>=s.open_ms&&now<s.close_ms);
 let mut report=massive_feed::etf_scanner::fetch_stock(&symbol,7,None).await.map_err(str::to_owned)?;
 let mut historical=None;
 if !active {
  let mut date=today;
  for _ in 0..14 {
   match calendar::schedule(date){
    Day::Open(s) if s.close_ms<=now=>{historical=Some(s);break;},
    Day::Unknown=>return Err("Calendario fuera de cobertura".into()),
    _=>{}
   }
   date=date.pred_opt().ok_or("Calendario inválido")?;
  }
  let session=historical.as_ref().ok_or("Última sesión no disponible")?;
  let credentials=massive_feed::credentials::Credentials::load().map_err(str::to_owned)?;
  let closing=massive_feed::history::fetch_unadjusted(&credentials,&symbol,session.close_ms-60000,session.close_ms-1).await.map_err(str::to_owned)?;
  let bar=closing.iter().find(|b|b.timestamp_ms.checked_add(60_000)==Some(session.close_ms)).ok_or("Cierre de la última sesión no disponible")?;
  report.spot=bar.close;
  report.price_at=Some(session.close_ms);
 }
 let expiration_closes:std::collections::BTreeMap<_,_>=report.expirations.iter().filter_map(|date|calendar::rfc3339_ms(&format!("{date}T12:00:00Z")).and_then(calendar::date_at).and_then(|d|match calendar::schedule(d){Day::Open(s)=>Some((date.clone(),s.close_ms)),_=>None})).collect();
 let mut value=serde_json::to_value(report).map_err(|_|"Snapshot inválido")?;
 value["expirationCloses"]=serde_json::to_value(expiration_closes).map_err(|_|"Calendario de vencimiento inválido")?;
 value["historicalSession"]=serde_json::to_value(historical).map_err(|_|"Sesión inválida")?;
 Ok(value)
}
