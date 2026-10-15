//! Official holdings cache, separate from trading and broker state.
use serde::{Deserialize,Serialize};
use std::sync::OnceLock;
pub const SOURCE:&str="https://dng-api.invesco.com/cache/v1/accounts/en_US/shareclasses/QQQ/holdings/fund?idType=ticker&interval=monthly&productType=ETF";
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Holding {pub symbol:String,pub weight:f64}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Holdings {pub as_of:String,pub source:String,pub rows:Vec<Holding>}
/// Shared ten-member pool. Open positions retain their slots and priority.
pub fn pool_symbols(mut existing:Vec<String>,held:&[String],bot_enabled:bool,components:&[String])->Vec<String>{
 if components.is_empty(){return existing;}
 if !bot_enabled {existing=held.to_vec();}
 for symbol in ["QQQ".to_owned(),"SPY".to_owned()].into_iter().chain(components.iter().cloned()){
  if existing.len()>=20{break;}
  if !existing.contains(&symbol)&&snipers_core::universe::stock_asset(&symbol).is_some(){existing.push(symbol);}
 }
 existing
}
pub fn parse(v:&serde_json::Value,now:u64)->Result<Holdings,String>{
 let date=v["effectiveBusinessDate"].as_str().ok_or("Fecha oficial ausente")?;
 let today=snipers_core::calendar::date_at(now).ok_or("Calendario no disponible")?;
 // ISO calendar dates are validated against the current and previous seven ET dates.
 let valid=(0..=7).any(|day|snipers_core::calendar::date_at(now.saturating_sub(day*86400000)).is_some_and(|d|d.to_string()==date));
 if !valid || date>today.to_string().as_str(){return Err("Pesos oficiales caducados o futuros".into());}
 let mut rows=Vec::new();let mut seen=std::collections::BTreeSet::new();
 for h in v["holdings"].as_array().ok_or("Holdings ausentes")? {
  if h["securityTypeCode"]!="COM" {continue;}
  let symbol=h["ticker"].as_str().ok_or("Ticker ausente")?;
  let weight=h["percentageOfTotalNetAssets"].as_f64().ok_or("Peso ausente")?;
  if symbol.is_empty()||!symbol.chars().all(|c|c.is_ascii_uppercase()||c=='.')||!weight.is_finite()||weight<=0.||weight>100.||!seen.insert(symbol.to_owned()){return Err("Pesos inválidos o repetidos".into());}
  rows.push(Holding{symbol:symbol.into(),weight});
 }
 let total:f64=rows.iter().map(|r|r.weight).sum();
 if !(90. ..=101.).contains(&total){return Err("Cobertura oficial incompleta".into());}
 rows.sort_by(|a,b|b.weight.total_cmp(&a.weight));rows.truncate(12);
 Ok(Holdings{as_of:date.into(),source:SOURCE.into(),rows})
}
pub async fn fetch()->Result<Holdings,String>{
 static CACHE:OnceLock<tokio::sync::Mutex<Option<(u64,Holdings)>>>=OnceLock::new();
 let mut cache=CACHE.get_or_init(||tokio::sync::Mutex::new(None)).lock().await;
 let now=crate::live::now_ms();
 if let Some((at,h))=cache.as_ref(){if *at<=now&&snipers_core::calendar::date_at(*at)==snipers_core::calendar::date_at(now){return Ok(h.clone());}}
 let client=reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().map_err(|_|"Cliente oficial no disponible")?;
 let response=client.get(SOURCE).header("Accept","application/json").header("Referer","https://www.invesco.com/").header("Origin","https://www.invesco.com").header("Accept-Language","en-US,en;q=0.9").send().await.map_err(|_|"Invesco no disponible")?;
 if !response.status().is_success(){return Err(format!("Consulta de pesos Invesco: HTTP {}",response.status().as_u16()));}
 let raw=response.json::<serde_json::Value>().await.map_err(|_|"Respuesta oficial inválida")?;
 let holdings=parse(&raw,now)?;*cache=Some((now,holdings.clone()));Ok(holdings)
}
#[cfg(test)]
mod tests {
 use super::*;
 fn fixture()->serde_json::Value{serde_json::from_str(include_str!("../tests/fixtures/qqq-invesco-2026-10-02.json")).unwrap()}
 const NOW:u64=1791201600000; // 2026-10-05 12:00 UTC
 #[test] fn official_weights_sorted_and_separate_classes(){let h=parse(&fixture(),NOW).unwrap();assert_eq!(h.rows.len(),12);assert_eq!(h.as_of,"2026-10-02");assert_eq!(h.rows[0].symbol,"NVDA");assert!(h.rows.iter().any(|r|r.symbol=="GOOG"));assert!(h.rows.iter().any(|r|r.symbol=="GOOGL"));assert!((h.rows.iter().map(|r|r.weight).sum::<f64>()-52.169059).abs()<0.00001);}
 #[test] fn rejects_bad_dates(){for d in ["2026-10-06","2026-09-01","invalid"]{let mut v=fixture();v["effectiveBusinessDate"]=d.into();assert!(parse(&v,NOW).is_err());}}
 #[test] fn rejects_duplicate_negative_missing_and_incomplete(){for kind in 0..4{let mut v=fixture();match kind{0=>v["holdings"][1]["ticker"]=v["holdings"][0]["ticker"].clone(),1=>v["holdings"][0]["percentageOfTotalNetAssets"]=(-1.).into(),2=>v["holdings"][0]["percentageOfTotalNetAssets"]=serde_json::Value::Null,_=>{v["holdings"].as_array_mut().unwrap().truncate(2);}}assert!(parse(&v,NOW).is_err());}}
 #[test] fn pool_is_bounded_deduplicated_and_weight_ordered(){let h=parse(&fixture(),NOW).unwrap();let symbols=h.rows.iter().map(|r|r.symbol.clone()).collect::<Vec<_>>();let pool=pool_symbols(vec!["AMD".into()],&[],false,&symbols);assert!(pool.len()<=14);assert_eq!(&pool[..3],&["QQQ","SPY","NVDA"]);assert_eq!(pool.iter().collect::<std::collections::BTreeSet<_>>().len(),pool.len());}
 #[test] fn held_and_bot_members_are_not_displaced(){let previous:Vec<_>=["AMD","NVDA","TSLA","QQQ","SPY","AAPL","MSFT","META","AMZN","GOOGL"].into_iter().map(str::to_owned).collect();assert_eq!(&pool_symbols(previous.clone(),&[],true,&["MU".into()])[..previous.len()],previous.as_slice());let held=vec!["TSLA".into(),"AMD".into()];assert_eq!(&pool_symbols(vec![],&held,false,&["NVDA".into()])[..2],held.as_slice());}
}
