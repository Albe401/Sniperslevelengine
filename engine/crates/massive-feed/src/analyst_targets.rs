//! Basic-compatible targets. Never return credentials, vendor errors, or request URLs.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub const TTL: u64 = 86_400_000;
/// Daily target reuse follows New York dates; the API budget remains rolling 24 hours.
pub fn same_day(at:u64,now:u64)->bool {at<=now&&snipers_core::calendar::date_at(at).is_some_and(|day|Some(day)==snipers_core::calendar::date_at(now))}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Targets {
 pub symbol:String, pub status:String, pub fetched_ms:u64,
 pub low:Option<f64>, pub average:Option<f64>, pub high:Option<f64>,
}
impl Targets {
 pub fn unavailable(symbol:&str,status:&str,now:u64)->Self {
  Self{symbol:symbol.into(),status:status.into(),fetched_ms:now,low:None,average:None,high:None}
 }
 pub fn fresh(&self,now:u64)->bool {same_day(self.fetched_ms,now)}
}
pub fn configured()->bool {crate::investment_fmp::key().is_some()}
pub fn parse(symbol:&str,data:&Value,now:u64)->Targets {
 let mut out=Targets::unavailable(symbol,"Sin objetivos disponibles",now);
 let Some(rows)=data.as_array() else {
  let message=data.get("Error Message").or_else(||data.get("message")).and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
  out.status=if message.contains("invalid api")||message.contains("invalid key") {"FMP: clave no válida"}
   else if message.contains("limit reach")||message.contains("limit has been") {"FMP: cuota agotada"}
   else if message.contains("premium")||message.contains("restricted")||message.contains("upgrade") {"Sin acceso con el plan FMP actual"}
   else {"Respuesta FMP no válida"}.into();return out;
 };
 let matches:Vec<_>=rows.iter().filter(|r|r["symbol"]==symbol).collect();
 if matches.len()!=1 {return out;}
 let number=|key:&str|matches[0][key].as_f64().filter(|n|n.is_finite()&&*n>0.);
 if let (Some(low),Some(average),Some(high))=(number("targetLow"),number("targetConsensus"),number("targetHigh")) {
  if low<=average&&average<=high {out.low=Some(low);out.average=Some(average);out.high=Some(high);out.status="Disponible".into();}
  else {out.status="Objetivos incoherentes".into();}
 }
 out
}
pub async fn fetch(symbol:&str)->Targets {
 let now=crate::live::now_ms();
 let Some(key)=crate::investment_fmp::key() else {return Targets::unavailable(symbol,"FMP_API_KEY no configurada",now)};
 let result=async {
  let client=reqwest::Client::builder().timeout(Duration::from_secs(10)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_|"No se pudo conectar con FMP")?;
  let mut response=client.get("https://financialmodelingprep.com/stable/price-target-consensus").query(&[("symbol",symbol),("apikey",key.as_str())]).send().await.map_err(|_|"FMP: conexión no disponible")?;
  match response.status().as_u16() {
   401=>return Err("FMP: clave no válida"),
   402|403=>return Err("Sin acceso con el plan FMP actual"),
   429=>return Err("FMP: cuota agotada"),
   200=>(), _=>return Err("FMP: servicio no disponible"),
  }
  let mut bytes=vec![];
  while let Some(chunk)=response.chunk().await.map_err(|_|"FMP: respuesta incompleta")? {
   if bytes.len()+chunk.len()>1_000_000 {return Err("FMP: respuesta demasiado grande")}
   bytes.extend_from_slice(&chunk);
  }
  let data:Value=serde_json::from_slice(&bytes).map_err(|_|"FMP: JSON inválido")?;
  Ok(parse(symbol,&data,now))
 }.await;
 result.unwrap_or_else(|status|Targets::unavailable(symbol,status,now))
}
pub fn companies()->Vec<String> {
 let priority=["NVDA","AAPL","MSFT","AMZN","GOOGL","GOOG","META","TSLA","AVGO"];
 let mut symbols:Vec<_>=snipers_core::universe::assets().iter().filter(|(_,a)|matches!(a.kind,snipers_core::universe::AssetKind::Stock)).map(|(s,_)|s.clone()).collect();
 symbols.sort_by_key(|s|(priority.iter().position(|p|p==s).unwrap_or(priority.len()),s.clone()));symbols
}
#[cfg(test)] mod tests {
 use super::*;use serde_json::json;
 #[test] fn validates_symbol_range_and_completeness() {
  let good=json!([{"symbol":"AAPL","targetLow":100,"targetConsensus":120,"targetHigh":150}]);
  assert_eq!(parse("AAPL",&good,1).average,Some(120.));
  assert!(parse("MSFT",&good,1).average.is_none());
  for bad in [json!([]),json!({"Error Message":"restricted"}),json!([{"symbol":"AAPL","targetLow":150,"targetConsensus":120,"targetHigh":100}]),json!([{"symbol":"AAPL","targetConsensus":120}])] {assert!(parse("AAPL",&bad,1).average.is_none());}
 }
 #[test] fn daily_cache_handles_utc_midnight_and_dst() {
  let ms=|s:&str|snipers_core::calendar::rfc3339_ms(s).unwrap();
  let t=ms("2026-10-02T23:00:00Z");
  assert!(same_day(t,ms("2026-10-03T03:59:59Z")));
  assert!(!same_day(t,ms("2026-10-03T04:00:00Z")));
  let t=ms("2026-11-01T04:05:00Z");
  assert!(same_day(t,ms("2026-11-02T04:55:00Z")));
  assert!(!same_day(t,ms("2026-11-02T05:00:00Z")));
 }
 #[test] fn preserves_cache_for_new_york_day_and_excludes_funds() {
  let t=Targets::unavailable("AAPL","Sin acceso",100);
  assert!(t.fresh(101));assert!(!t.fresh(TTL));assert!(!t.fresh(99));
  let companies=companies();assert_eq!(companies[0],"NVDA");assert!(companies.contains(&"MSFT".into()));assert!(!companies.contains(&"QQQ".into()));assert!(!companies.contains(&"SPX".into()));
 }
}
