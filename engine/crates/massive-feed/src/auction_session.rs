//! All-venue RTH backfill for the AMT display only. No order or bot inputs.
use crate::{auction_live::{Engine,trade_size},credentials::Credentials,history::checked_url,live::{SharedState,now_ms}};
use serde_json::Value;
use std::{collections::BTreeSet,time::Duration};
use snipers_core::calendar::{self,Day};

fn parse(row:&Value,open:u64,through:u64)->Result<Option<(f64,f64,u64,u64)>,&'static str>{
 let at=row["sip_timestamp"].as_u64().ok_or("REST sin timestamp SIP")?/1_000_000;
 if at<open||at>through{return Err("REST de otra ventana");}
 if row.get("correction").is_some_and(|v|!v.is_null()&&v.as_u64()!=Some(0)){return Ok(None);}
 if let Some(c)=row.get("conditions").filter(|c|!c.is_null()){
  let c=c.as_array().ok_or("Condiciones REST inválidas")?;
  let codes:Vec<u32>=c.iter().map(|v|v.as_u64().and_then(|n|u32::try_from(n).ok()).ok_or("Código REST inválido")).collect::<Result<_,_>>()?;
  if !snipers_core::stock_volume_policy::permits_volume_at(&codes,at){return Ok(None);}
 }
 let p=row["price"].as_f64().filter(|p|p.is_finite()&&*p>0.).ok_or("Precio REST inválido")?;
 let v=trade_size(row["size"].as_f64().unwrap_or(0.),row.get("decimal_size").filter(|v|!v.is_null()).map(|v|v.as_str().ok_or("Tamaño decimal REST inválido")).transpose()?).ok_or("Volumen REST inválido")?;
 let q=row["sequence_number"].as_u64().ok_or("REST sin secuencia")?;Ok(Some((p,v,at,q)))
}
/// Offline/reference adapter exposes the same parsing and policy as REST backfill.
pub fn audited_reference_trade(row:&Value,open:u64,through:u64)->Result<Option<(f64,f64,u64,u64)>,&'static str>{parse(row,open,through)}
async fn page(client:&reqwest::Client,key:&Credentials,url:reqwest::Url)->Result<Value,&'static str>{
 for attempt in 0..3{
  match client.get(url.clone()).bearer_auth(&key.0).send().await{
   Ok(mut r)if r.status().is_success()=>{let mut bytes=vec![];while let Some(chunk)=r.chunk().await.map_err(|_|"Respuesta AMT interrumpida")?{if bytes.len()+chunk.len()>32*1024*1024{return Err("Página AMT demasiado grande");}bytes.extend_from_slice(&chunk);}return serde_json::from_slice(&bytes).map_err(|_|"Respuesta AMT inválida");},
   Ok(r)if [401,403].contains(&r.status().as_u16())=>return Err("Acceso REST AMT rechazado (401/403)"),
   Ok(r)if r.status().as_u16()==429||r.status().is_server_error()=>{},
   _=>{},
  }
  if attempt<2{tokio::time::sleep(Duration::from_secs(1<<attempt)).await;}
 }Err("No se pudo recuperar REST AMT")
}
#[derive(serde::Serialize)]
pub struct WindowData {
 pub trades:Vec<snipers_core::auction_map::Trade>,
 pub excluded_rows:usize,
 pub unresolved_corrections:usize,
 pub pages:usize,
 pub raw_rows:Vec<Value>,
}
/// Independent REST reference, half-open SIP window, same explicit condition population.
pub async fn fetch_window(key:&Credentials,request:&crate::auction_reconciliation::Request,mut current:impl FnMut()->bool)->Result<WindowData,&'static str>{
 if snipers_core::universe::stock_asset(&request.symbol).is_none(){return Err("Activo de cotejo AMT no habilitado");}
 let Some(date)=calendar::date_at(request.start_ms)else{return Err("Fecha de cotejo inválida")};
 let Day::Open(session)=calendar::schedule(date)else{return Err("Cotejo fuera de sesión")};
 if request.start_ms<session.open_ms||request.end_ms>session.close_ms||request.start_ms>=request.end_ms||request.end_ms-request.start_ms>60_000{return Err("Ventana de cotejo AMT inválida");}
 let path=format!("/v3/trades/{}",request.symbol);
 let from=request.start_ms.checked_mul(1_000_000).ok_or("Timestamp inválido")?;
 let until=request.end_ms.checked_mul(1_000_000).ok_or("Timestamp inválido")?;
 let mut url=checked_url(&format!("https://api.massive.com{path}?timestamp.gte={from}&timestamp.lt={until}&order=asc&sort=timestamp&limit=5000"),&path)?;
 let client=reqwest::Client::builder().timeout(Duration::from_secs(20)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_|"Cliente de cotejo no disponible")?;
 let mut data=WindowData{trades:vec![],excluded_rows:0,unresolved_corrections:0,pages:0,raw_rows:vec![]};let mut visited=BTreeSet::new();
 for pages in 1..=10 {
  if !current(){return Err("Cotejo AMT cancelado por cambio de estado");}
  if !visited.insert(url.to_string()){return Err("Paginación de cotejo AMT repetida");}
  let result=page(&client,key,url).await?;
  if result["status"]!="OK"{return Err("Referencia REST AMT no confirmada");}
  let rows=result.get("results").filter(|v|!v.is_null()).map(|v|v.as_array().ok_or("Filas de cotejo inválidas")).transpose()?.cloned().unwrap_or_default();
  if rows.len()>5000||data.raw_rows.len()+rows.len()>crate::auction_reconciliation::MAX_ROWS{return Err("Referencia de cotejo excede capacidad");}
  for row in &rows {match parse(row,request.start_ms,request.end_ms-1)? {
   Some((price,volume,at,q))=>data.trades.push(snipers_core::auction_map::Trade{symbol:request.symbol.clone(),at,id:q.to_string(),price,volume}),
   None=>{data.excluded_rows+=1;if row.get("correction").is_some_and(|v|!v.is_null()&&v.as_u64()!=Some(0)){data.unresolved_corrections+=1;}},
  }}
  data.raw_rows.extend(rows);data.pages=pages;
  match result.get("next_url").filter(|v|!v.is_null()) {
   None=>{if !current(){return Err("Cotejo AMT cancelado al finalizar");}return Ok(data);},
   Some(next)=>url=checked_url(next.as_str().ok_or("Paginación de cotejo inválida")?,&path)?,
  }
 }
 Err("Referencia REST AMT truncada: cotejo pendiente")
}
pub async fn fetch(key:&Credentials,symbol:&str,open:u64,through:u64,mut progress:impl FnMut(usize,usize)->bool)->Result<(Engine,usize,usize),&'static str>{
 if snipers_core::universe::stock_asset(symbol).is_none(){return Err("Activo AMT no habilitado");}
 let Some(date)=calendar::date_at(open)else{return Err("Fecha AMT inválida")};
 let Day::Open(session)=calendar::schedule(date)else{return Err("Sin sesión AMT")};
 if open!=session.open_ms||through<open||through>=session.close_ms{return Err("Ventana RTH inválida");}
 let path=format!("/v3/trades/{symbol}");
 let from=open.checked_mul(1_000_000).ok_or("Timestamp fuera de rango")?;
 let until=through.checked_add(1).and_then(|t|t.checked_mul(1_000_000)).ok_or("Timestamp fuera de rango")?;
 let mut url=checked_url(&format!("https://api.massive.com{path}?timestamp.gte={from}&timestamp.lt={until}&order=asc&sort=timestamp&limit=50000"),&path)?;
 let client=reqwest::Client::builder().timeout(Duration::from_secs(45)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_|"Cliente REST AMT no disponible")?;
 let mut visited=BTreeSet::new();let mut engine=Engine::default();let mut count=0;let mut previous=None;
 for pages in 1..=100{
  if !progress(pages-1,count){return Err("Carga AMT cancelada por cambio de activo o conexión");}
  if !visited.insert(url.to_string()){return Err("Paginación AMT repetida");}
  let result=page(&client,key,url).await?;if result["status"]!="OK"{return Err("Historial AMT no confirmado");}
  let rows=result.get("results").filter(|v|!v.is_null()).map(|v|v.as_array().ok_or("Filas AMT inválidas")).transpose()?.cloned().unwrap_or_default();
  if rows.len()>50000{return Err("Página AMT excede límite");}count+=rows.len();
  let mut parsed=Vec::new();
  for row in &rows {
   match parse(row,open,through)? {
    Some(t)=>{let conditions:Vec<u32>=row["conditions"].as_array().into_iter().flatten().filter_map(|v|v.as_u64().and_then(|n|u32::try_from(n).ok())).collect();parsed.push((t,conditions));},
    None=>{
     let size=trade_size(row["size"].as_f64().unwrap_or(0.),row["decimal_size"].as_str()).unwrap_or(0.);
     let conditions:Vec<u32>=row["conditions"].as_array().into_iter().flatten().filter_map(|v|v.as_u64().and_then(|n|u32::try_from(n).ok())).collect();
     let reason=if row.get("correction").is_some_and(|v|!v.is_null()&&v.as_u64()!=Some(0)){"rest_correction_pending"}else{"condition_policy"};
     engine.record_rest_exclusion(reason,size,&conditions);
    }
   }
  }
  parsed.sort_by_key(|(t,_)|(t.2,t.3));
  for (t,conditions) in parsed{if previous==Some(t){continue;}engine.history_trade_with_conditions(symbol,t.0,t.1,t.2,t.3,through,&conditions)?;previous=Some(t);}
  if !progress(pages,count){return Err("Carga AMT cancelada por cambio de activo o conexión");}
  match result.get("next_url").filter(|v|!v.is_null()){
   None=>{if engine.history_count()==0{return Err("Sin operaciones RTH admisibles");}return Ok((engine,pages,count));},
   Some(next)=>url=checked_url(next.as_str().ok_or("Paginación AMT inválida")?,&path)?,
  }
 }Err("Historial AMT excede 5 millones de operaciones: cobertura pendiente")
}
pub async fn worker(key:&Credentials,state:SharedState){
 loop{
  tokio::time::sleep(Duration::from_secs(5)).await;let now=now_ms();
  let request={let mut s=state.lock().unwrap();if s.stocks.phase!="streaming"{None}else{let symbol=s.symbol.clone();s.auction_engine.begin_history(now).map(|r|(symbol,r))}};
  let Some((symbol,(open,through,epoch)))=request else{
   let request={let mut s=state.lock().unwrap();if s.stocks.phase!="streaming"{None}else{s.auction_engine.begin_reconciliation(now)}};
   if let Some(request)=request {
    let data=fetch_window(key,&request,||{let s=state.lock().unwrap();s.stocks.phase=="streaming"&&s.symbol==request.symbol&&s.auction_engine.reconciliation_current(&request)}).await;
    let result=data.and_then(|d|crate::auction_reconciliation::compare(&request,&d.trades,d.excluded_rows,d.unresolved_corrections,now_ms()));
    let mut s=state.lock().unwrap();s.auction_engine.finish_reconciliation(&request,result);
   }
   continue;
  };
  let result=fetch(key,&symbol,open,through,|pages,rows|{let mut s=state.lock().unwrap();s.symbol==symbol&&s.stocks.phase=="streaming"&&s.auction_engine.history_progress(epoch,pages,rows)}).await;
  {let mut s=state.lock().unwrap();if s.symbol!=symbol{continue;}
   match result{Ok((engine,pages,rows))=>{if let Err(e)=s.auction_engine.apply_history(epoch,engine,pages,rows,now_ms()){s.auction_engine.history_failed(epoch,e);}},Err(e)=>s.auction_engine.history_failed(epoch,e)}
  }tokio::time::sleep(Duration::from_secs(60)).await;
 }
}
#[cfg(test)]mod tests{
 use super::*;use serde_json::json;
 #[test]fn all_venues_decimal_conditions_corrections_and_causal_window(){
  let row=json!({"sip_timestamp":100_000_000,"sequence_number":7,"price":100.,"size":0,"decimal_size":"0.25","exchange":11,"conditions":[37]});
  assert_eq!(parse(&row,100,101).unwrap(),Some((100.,0.25,100,7)));
  let mut r=row.clone();r["exchange"]=json!(4);assert_eq!(parse(&r,100,101).unwrap(),parse(&row,100,101).unwrap());
  r["correction"]=json!(1);assert!(parse(&r,100,101).unwrap().is_none());
  r=row.clone();r["conditions"]=json!([2]);assert!(parse(&r,100,101).unwrap().is_some());
  r["conditions"]=json!([15]);assert!(parse(&r,100,101).unwrap().is_none());
  assert!(parse(&row,100,99).is_err());r=row.clone();r["decimal_size"]=json!("NaN");assert!(parse(&r,100,101).is_err());
 }
}
