//! Optional FMP enrichment. Fixed hosts, bounded responses, no secrets in IPC/errors.
use serde_json::Value;
use snipers_core::{investment::{Company,Metric},opportunity::Analysts};
use std::{path::PathBuf,time::Duration};
pub(crate) fn key()->Option<String>{
 if let Ok(k)=std::env::var("FMP_API_KEY"){if !k.trim().is_empty(){return Some(k);}}
 let mut paths=vec![];
 if let Some(p)=std::env::var_os("SNIPERS_ENV_FILE"){paths.push(PathBuf::from(p));}else{
  if let Ok(d)=std::env::current_dir(){crate::credentials::candidates(&d,&mut paths);}
  if let Ok(d)=std::env::current_exe(){if let Some(d)=d.parent(){crate::credentials::candidates(d,&mut paths);}}
 }
 for p in paths {if let Ok(entries)=dotenvy::from_path_iter(p){for (name,value) in entries.flatten(){if name=="FMP_API_KEY"&&!value.trim().is_empty(){return Some(value);}}}}
 None
}
async fn get(client:&reqwest::Client,key:&str,path:&str,symbol:&str)->Result<Value,String>{
 let mut response=client.get(format!("https://financialmodelingprep.com/stable/{path}")).query(&[("symbol",symbol),("apikey",key)]).send().await.map_err(|_|"FMP: fallo de red")?;
 if !response.status().is_success(){return Err(format!("FMP HTTP {}: comprobar permisos o cuota",response.status().as_u16()));}
 let mut bytes=vec![];while let Some(chunk)=response.chunk().await.map_err(|_|"FMP: respuesta incompleta")?{if bytes.len()+chunk.len()>1_000_000{return Err("FMP: respuesta demasiado grande".into());}bytes.extend_from_slice(&chunk);}
 serde_json::from_slice(&bytes).map_err(|_|"FMP: JSON inválido".into())
}
fn row<'a>(data:&'a Value,symbol:&str)->Option<&'a Value>{let rows=data.as_array()?;let matching:Vec<_>=rows.iter().filter(|r|r["symbol"]==symbol).collect();(matching.len()==1).then(||matching[0])}
fn positive(r:&Value,key:&str)->Option<f64>{r[key].as_f64().filter(|v|v.is_finite()&&*v>0.)}
pub fn consensus(symbol:&str,price:Option<f64>,targets:&Value,grades:&Value,now:u64)->Analysts {
 let t=row(targets,symbol);let g=row(grades,symbol);
 let average=t.and_then(|r|positive(r,"targetConsensus"));let low=t.and_then(|r|positive(r,"targetLow"));let high=t.and_then(|r|positive(r,"targetHigh"));
 let consistent=low.zip(average).is_none_or(|(l,a)|l<=a)&&average.zip(high).is_none_or(|(a,h)|a<=h)&&low.zip(high).is_none_or(|(l,h)|l<=h);
 let average=average.filter(|_|consistent);
 let count=|key:&str|g.and_then(|r|r[key].as_u64());
 let buy=count("strongBuy").zip(count("buy")).and_then(|(a,b)|a.checked_add(b));let hold=count("hold");let sell=count("strongSell").zip(count("sell")).and_then(|(a,b)|a.checked_add(b));
 Analysts{source:"FMP · price-target-consensus / grades-consensus".into(),status:if average.is_some(){"Disponible; fecha original no certificada"}else{"Objetivos no disponibles o incoherentes"}.into(),fetched_ms:now,provider_date:None,average,low:low.filter(|_|consistent),high:high.filter(|_|consistent),upside:average.zip(price.filter(|p|p.is_finite()&&*p>0.)).map(|(a,p)|(a/p-1.)*100.),consensus:g.and_then(|g|g["consensus"].as_str()).filter(|s|s.len()<80).map(str::to_owned),buy,hold,sell,total:buy.zip(hold).zip(sell).and_then(|((b,h),s)|b.checked_add(h)?.checked_add(s))}
}
pub async fn enrich(c:&mut Company)->Analysts {
 let Some(key)=key() else {return Analysts{source:"FMP".into(),status:"FMP_API_KEY no configurada; Massive sigue activo".into(),..Default::default()};};
 let Ok(client)=reqwest::Client::builder().timeout(Duration::from_secs(12)).redirect(reqwest::redirect::Policy::none()).build() else{return Analysts::default()};
 let (km,ratios)=tokio::join!(get(&client,&key,"key-metrics-ttm",&c.ticker),get(&client,&key,"ratios-ttm",&c.ticker));
 for (data,key,id,label,scale) in [(&km,"returnOnInvestedCapitalTTM","roic","ROIC TTM · FMP",100.),(&ratios,"priceToEarningsGrowthRatioTTM","peg","PEG TTM · FMP",1.)] {
  let value=data.as_ref().ok().and_then(|d|row(d,&c.ticker)).and_then(|r|r[key].as_f64()).filter(|v|v.is_finite()).map(|v|v*scale);
  c.metrics.push(Metric{id:id.into(),label:label.into(),group:"Oportunidad".into(),value,unit:if scale==100.{"pct"}else{"x"}.into(),source:"FMP stable TTM; fecha de publicación por ratio no certificada".into()});
  if let Err(e)=data {c.source_errors.push(e.clone());}
 }
 let (targets,grades)=tokio::join!(get(&client,&key,"price-target-consensus",&c.ticker),get(&client,&key,"grades-consensus",&c.ticker));
 let mut out=consensus(&c.ticker,snipers_core::opportunity::get(c,"last_close"),targets.as_ref().unwrap_or(&Value::Null),grades.as_ref().unwrap_or(&Value::Null),crate::live::now_ms());
 for result in [&targets,&grades] {if let Err(e)=result {c.source_errors.push(e.clone());out.status=format!("{} · consulta parcial",out.status);}}
 out
}
#[cfg(test)] mod tests {
 use super::*;use serde_json::json;
 #[test] fn missing_wrong_symbol_and_inverted_ranges_are_not_targets(){
  for data in [json!([]),json!([{"symbol":"SPY","targetConsensus":200}]),json!([{"symbol":"QQQ","targetConsensus":200,"targetLow":250,"targetHigh":300}])] {assert!(consensus("QQQ",Some(100.),&data,&Value::Null,1).average.is_none());}
 }
 #[test] fn absent_grades_are_unknown_not_zero(){let a=consensus("QQQ",Some(100.),&json!([{"symbol":"QQQ","targetConsensus":120}]),&json!([]),1);assert!((a.upside.unwrap()-20.).abs()<1e-9);assert!(a.total.is_none());}
}
