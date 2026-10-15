//! Closed, unadjusted 09:30–16:00 ET stock session bar for overnight research.
use crate::credentials::Credentials;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ClosingBar {pub symbol:String,pub date:String,pub open:f64,pub high:f64,pub low:f64,pub close:f64,pub volume:f64}

fn parse(symbol:&str,date:&str,open_ms:u64,page:&Value)->Result<ClosingBar,String>{
 if page["status"]!="OK"||page["ticker"]!=symbol||page["next_url"].as_str().is_some_and(|s|!s.is_empty()){return Err("La vela diaria no tiene cobertura verificable".into());}
 let rows=page["results"].as_array().ok_or("Falta la vela diaria")?;
 if rows.len()<380||rows.len()>390{return Err("Cobertura RTH insuficiente".into());}
 let (mut open,mut high,mut low,mut close,mut volume,mut previous)=(0.,f64::NEG_INFINITY,f64::INFINITY,0.,0.,None);
 for (index,row) in rows.iter().enumerate(){let stamp=row["t"].as_u64().ok_or("Minuto sin fecha")?;
  if stamp<open_ms||stamp>=open_ms+390*60_000||previous.is_some_and(|p|stamp<=p){return Err("Minutos RTH inválidos o repetidos".into());}previous=Some(stamp);
  let values=["o","h","l","c","v"].map(|key|row[key].as_f64());let [Some(o),Some(h),Some(l),Some(c),Some(v)]=values else{return Err("OHLCV de minuto incompleto".into())};
  if ![o,h,l,c,v].iter().all(|n|n.is_finite())||l<=0.||h<o.max(c)||l>o.min(c)||v<0.{return Err("OHLCV de minuto inválido".into());}
  if index==0{if stamp!=open_ms{return Err("Falta apertura RTH".into());}open=o;}high=high.max(h);low=low.min(l);close=c;volume+=v;
 }
 if previous!=Some(open_ms+389*60_000)||volume<=0.{return Err("Falta cierre RTH 15:59 ET".into());}
 Ok(ClosingBar{symbol:symbol.into(),date:date.into(),open,high,low,close,volume})
}

pub async fn closing_bar(symbol:&str,date:&str)->Result<ClosingBar,String>{
 if symbol.is_empty()||symbol.len()>12||!symbol.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit()||b==b'.'||b==b'-'){return Err("Símbolo inválido".into());}
 if date.len()!=10||!date.bytes().enumerate().all(|(i,b)|if i==4||i==7{b==b'-'}else{b.is_ascii_digit()}){return Err("Fecha inválida".into());}
 let now=crate::live::now_ms();let today=snipers_core::calendar::date_at(now).ok_or("Calendario no disponible")?.to_string();
 if date>today.as_str(){return Err("La sesión aún no ha ocurrido".into());}
 let open_at=snipers_core::calendar::legacy_eastern_ms(&format!("{date} 09:30:00")).ok_or("Sesión inválida")?;
 let close_at=snipers_core::calendar::legacy_eastern_ms(&format!("{date} 16:00:00")).ok_or("Sesión inválida")?;
 if now<close_at+5*60_000{return Err("La vela diaria aún no está cerrada o publicada".into());}
 let key=Credentials::load().map_err(str::to_owned)?;
 let path=format!("/v2/aggs/ticker/{symbol}/range/1/minute/{open_at}/{}",close_at-1);
 let url=crate::history::checked_url(&format!("https://api.massive.com{path}?adjusted=false&sort=asc&limit=50000"),&path).map_err(|_|"URL de velas inválida")?;
 let client=reqwest::Client::builder().timeout(Duration::from_secs(15)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_|"Cliente de mercado no disponible")?;
 let response=client.get(url).bearer_auth(&key.0).send().await.map_err(|_|"No se pudo consultar el cierre")?.error_for_status().map_err(|_|"Massive no publicó el cierre")?;
 let mut bytes=Vec::new();let mut response=response;
 loop{match response.chunk().await{Ok(Some(chunk)) if bytes.len()+chunk.len()<=1_000_000=>bytes.extend_from_slice(&chunk),Ok(None)=>break,_=>return Err("Respuesta de cierre incompleta".into())}}
 let page:Value=serde_json::from_slice(&bytes).map_err(|_|"Respuesta de cierre inválida")?;
 parse(symbol,date,open_at,&page)
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn requires_complete_rth_open_and_close(){let stamp=snipers_core::calendar::legacy_eastern_ms("2026-10-07 09:30:00").unwrap();let rows=(0..390).map(|i|serde_json::json!({"t":stamp+i*60000,"o":600.,"h":610.,"l":590.,"c":608.,"v":1000.})).collect::<Vec<_>>();let valid=serde_json::json!({"status":"OK","ticker":"QQQ","results":rows});assert!(parse("QQQ","2026-10-07",stamp,&valid).is_ok());let mut wrong=valid.clone();wrong["results"].as_array_mut().unwrap().pop();assert!(parse("QQQ","2026-10-07",stamp,&wrong).is_err());}
}
