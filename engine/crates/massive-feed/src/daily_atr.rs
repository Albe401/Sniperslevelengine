//! Bounded per-symbol daily cache. REST never runs under the live-state lock.
use crate::{credentials::Credentials,live::{now_ms,SharedState}};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap,time::Duration};
#[derive(Clone,Default,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Read {pub symbol:String,pub through:String,pub value:Option<f64>,pub close:Option<f64>,pub detail:String}
fn calculate(symbol:&str,rows:&[Value],now:u64)->Read {
    let mut out=Read{symbol:symbol.into(),detail:"ATR diario: faltan 15 sesiones completas y consecutivas.".into(),..Read::default()};
    let Some(sessions)=snipers_core::atr::sessions(now) else {out.detail="ATR diario: calendario fuera de cobertura.".into();return out;};
    out.through=sessions.last().unwrap().date.clone();
    let mut bars=BTreeMap::new();
    for r in rows {
        let Some(t)=r["t"].as_u64() else {continue;};
        let Some(date)=snipers_core::calendar::date_at(t).map(|d|d.to_string()) else {continue;};
        if !sessions.iter().any(|s|s.date==date) {continue;}
        let Some((o,h,l,c))=r["o"].as_f64().zip(r["h"].as_f64()).zip(r["l"].as_f64()).zip(r["c"].as_f64()).map(|(((o,h),l),c)|(o,h,l,c)) else {return out;};
        if ![o,h,l,c].iter().all(|v|v.is_finite()) || l<=0. || h<o.max(c) || l>o.min(c) || bars.insert(date,(h,l,c)).is_some(){return out;}
    }
    let Some(values)=sessions.iter().map(|s|bars.get(&s.date).copied()).collect::<Option<Vec<_>>>() else {return out;};
    out.value=snipers_core::atr::sma14(&values);
    if out.value.is_some(){out.close=Some(values.last().unwrap().2);out.detail="ATR14 diario · media simple · precios ajustados por splits · 15 sesiones cerradas.".into();}
    out
}
async fn fetch(key:&Credentials,symbol:&str,now:u64)->Read {
    let failed=|message:&str|Read{symbol:symbol.into(),detail:message.into(),..Read::default()};
    let Some(sessions)=snipers_core::atr::sessions(now) else{return failed("ATR diario: calendario pendiente.");};
    let path=format!("/v2/aggs/ticker/{symbol}/range/1/day/{}/{}",sessions[0].date,sessions.last().unwrap().date);
    let Ok(url)=crate::history::checked_url(&format!("https://api.massive.com{path}?adjusted=true&sort=asc&limit=50000"),&path) else{return failed("ATR diario: URL inválida.");};
    let Ok(client)=reqwest::Client::builder().timeout(Duration::from_secs(15)).redirect(reqwest::redirect::Policy::none()).build() else{return failed("ATR diario: REST pendiente.");};
    let Ok(mut response)=client.get(url).bearer_auth(&key.0).send().await else{return failed("ATR diario: conexión pendiente.");};
    if !response.status().is_success(){return failed("ATR diario: proveedor no autorizó/completó la consulta.");}
    let mut bytes=Vec::new();
    loop {match response.chunk().await {Ok(Some(c)) if bytes.len()+c.len()<=1_000_000=>bytes.extend_from_slice(&c),Ok(None)=>break,_=>return failed("ATR diario: respuesta incompleta.")}}
    let Ok(page)=serde_json::from_slice::<Value>(&bytes) else{return failed("ATR diario: JSON inválido.");};
    if page["status"]!="OK" || page["ticker"]!=symbol || page["next_url"].as_str().is_some_and(|v|!v.is_empty()){return failed("ATR diario: cobertura no confirmada.");}
    let Some(rows)=page["results"].as_array() else{return failed("ATR diario: sin velas.");};
    calculate(symbol,rows,now)
}
fn cached(entry:Option<&(String,u64,Read)>,through:&str,now:u64)->Option<Read>{
    entry.filter(|(d,t,r)|d==through && *t<=now && (r.value.is_some() || now.saturating_sub(*t)<300_000)).map(|(_,_,r)|r.clone())
}
pub async fn worker(key:&Credentials,state:SharedState) {
    let mut cache:BTreeMap<String,(String,u64,Read)>=BTreeMap::new();
    loop {
        let now=now_ms();let symbol=state.lock().unwrap().symbol.clone();
        let through=snipers_core::atr::sessions(now).and_then(|v|v.last().map(|s|s.date.clone())).unwrap_or_default();
        let hit=cached(cache.get(&symbol),&through,now);
        let read=if let Some(r)=hit {r} else {let r=fetch(key,&symbol,now).await;if cache.len()>=256 {cache.clear();}cache.insert(symbol.clone(),(through,now,r.clone()));r};
        {let mut s=state.lock().unwrap();if s.symbol==symbol{s.daily_atr=read;}}
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
#[cfg(test)] mod tests {
    use super::*;
    fn fixture()->(u64,Vec<Value>){let now=snipers_core::calendar::rfc3339_ms("2026-10-03T12:00:00-04:00").unwrap();let rows=snipers_core::atr::sessions(now).unwrap().iter().map(|s|serde_json::json!({"t":snipers_core::calendar::day_bounds(&s.date).unwrap().0,"o":100.,"h":101.,"l":99.,"c":100.})).collect();(now,rows)}
    #[test]fn complete_and_missing(){let(n,mut r)=fixture();assert_eq!(calculate("QQQ",&r,n).value,Some(2.));r.pop();assert_eq!(calculate("QQQ",&r,n).value,None);}
    #[test]fn duplicate_and_bad_open(){let(n,mut r)=fixture();r.push(r[0].clone());assert_eq!(calculate("QQQ",&r,n).value,None);r.pop();r[0]["o"]=serde_json::json!(200.);assert_eq!(calculate("QQQ",&r,n).value,None);}
    #[test]fn excludes_future(){let(n,mut r)=fixture();r.push(serde_json::json!({"t":n,"o":1.,"h":1000.,"l":1.,"c":1.}));assert_eq!(calculate("QQQ",&r,n).value,Some(2.));}
    #[test]fn cache_refreshes_only_for_session_or_failed_retry(){let(n,r)=fixture();let good=("2026-10-02".into(),n,calculate("QQQ",&r,n));assert!(cached(Some(&good),"2026-10-02",n+86_400_000).is_some());assert!(cached(Some(&good),"2026-10-05",n+86_400_000).is_none());let bad=(good.0.clone(),n,Read::default());assert!(cached(Some(&bad),"2026-10-02",n+1000).is_some());assert!(cached(Some(&bad),"2026-10-02",n+300_000).is_none());assert!(cached(None,"2026-10-02",n).is_none());}
}
