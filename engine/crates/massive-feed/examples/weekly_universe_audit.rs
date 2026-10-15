use snipers_core::calendar::{self,Day};
#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();let symbols:Vec<String>=serde_json::from_str(&std::fs::read_to_string(&args[1])?)?;let mut runs=vec![];
 for symbol in symbols{
 let result=async{
 let mut report=massive_feed::etf_scanner::fetch_stock(&symbol,7,None).await.map_err(str::to_owned)?;
 let now=massive_feed::live::now_ms();let today=calendar::date_at(now).ok_or("Fecha no disponible")?;
 let mut historical=None;
 if !matches!(calendar::schedule(today),Day::Open(ref s) if now>=s.open_ms&&now<s.close_ms){
 let mut date=today;for _ in 0..14{match calendar::schedule(date){Day::Open(s) if s.close_ms<=now=>{historical=Some(s);break;},Day::Unknown=>return Err("Calendario fuera de cobertura".into()),_=>{}}date=date.pred_opt().ok_or("Calendario inválido")?;}
 let session=historical.as_ref().ok_or("Última sesión no disponible")?;
 let credentials=massive_feed::credentials::Credentials::load().map_err(str::to_owned)?;
 let closing=massive_feed::history::fetch_unadjusted(&credentials,&symbol,session.close_ms-60000,session.close_ms-1).await.map_err(str::to_owned)?;
 let bar=closing.iter().find(|b|b.timestamp_ms.checked_add(60000)==Some(session.close_ms)).ok_or("Cierre de la última sesión no disponible")?;
 report.spot=bar.close;report.price_at=Some(session.close_ms);
 }
 let closes:std::collections::BTreeMap<_,_>=report.expirations.iter().filter_map(|date|calendar::rfc3339_ms(&format!("{date}T12:00:00Z")).and_then(calendar::date_at).and_then(|d|match calendar::schedule(d){Day::Open(s)=>Some((date.clone(),s.close_ms)),_=>None})).collect();
 let mut value=serde_json::to_value(&mut report).map_err(|_|"Snapshot inválido")?;value["expirationCloses"]=serde_json::to_value(closes).map_err(|_|"Calendario inválido")?;value["historicalSession"]=serde_json::to_value(historical).map_err(|_|"Sesión inválida")?;
 Ok::<_,String>(serde_json::json!({"symbol":symbol,"checkedMs":now,"report":value}))
 }.await;
 let value=match result{Ok(v)=>v,Err(e)=>serde_json::json!({"symbol":symbol,"checkedMs":massive_feed::live::now_ms(),"error":e})};
 println!("{} {}",symbol,if value.get("report").is_some(){"consultado"}else{"pendiente"});runs.push(value);
 std::fs::write(&args[2],serde_json::to_vec_pretty(&serde_json::json!({"runs":runs}))?)?;
 }
 Ok(())
}
