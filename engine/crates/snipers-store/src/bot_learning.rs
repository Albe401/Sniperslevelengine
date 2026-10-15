//! Read-only bot results. Completed fills, not signals or submitted orders.
use rusqlite::{Connection,OpenFlags};
use serde_json::{json,Value};
use std::{path::Path,collections::BTreeMap};
fn number(v:&Value)->Option<f64>{v.as_f64().filter(|n|n.is_finite())}
fn time(v:&Value)->Option<u64>{v.as_str().and_then(snipers_core::calendar::rfc3339_ms)}
pub fn summarize(rows:Vec<Value>,filter:Option<&str>,now:u64)->Value{
 let mut selected=Vec::new();let mut symbols=std::collections::BTreeSet::new();
 for r in rows{let contract=r["symbol"].as_str().unwrap_or("");let tagged=format!("O:{}",contract.trim_start_matches("O:"));let root=snipers_core::options::underlying(&tagged).unwrap_or("");if root.is_empty(){continue;}symbols.insert(root.to_owned());if filter.is_some_and(|f|f!=root){continue;}selected.push((root.to_owned(),r));}
 let mut closed=Vec::new();let mut recent=Vec::new();let(mut pending,mut open,mut incomplete,mut cancelled)=(0,0,0,0);
 for(root,r)in selected{
  let phase=r["phase"].as_str().unwrap_or("unknown");let entry=time(&r["entry_time"]);let exit=time(&r["exit_time"]);
  let qty=r["last_order"]["filled_qty"].as_str().and_then(|v|v.parse::<f64>().ok()).or_else(||number(&r["last_order"]["filled_qty"]));
  let verified=phase=="done"&&entry.zip(exit).is_some_and(|(a,b)|a<=b&&b<=now)&&number(&r["entry_premium"]).is_some_and(|n|n>0.)&&number(&r["exit_premium"]).is_some_and(|n|n>=0.)&&qty==Some(1.);
  let pnl=if verified{Some((number(&r["exit_premium"]).unwrap()-number(&r["entry_premium"]).unwrap())*100.)}else{None};
  let state=if verified{"closed"}else if phase=="done"&&entry.is_none()&&number(&r["entry_premium"]).is_none(){cancelled+=1;"unfilled"}else if phase=="open"&&entry.is_some(){open+=1;"open"}else if phase=="buy_pending"||phase=="sell_pending"{pending+=1;"pending"}else{incomplete+=1;"incomplete"};
  let version=r["entry_context"]["management_version"].as_str().unwrap_or("Sin versión registrada");
  let overnight=entry.zip(exit).map(|(a,b)|snipers_core::calendar::date_at(a)!=snipers_core::calendar::date_at(b));
  if let Some(p)=pnl{closed.push((exit.unwrap(),p,version.to_owned(),overnight.unwrap_or(false)));}
  recent.push(json!({"symbol":root,"contract":r["symbol"],"side":r["side"],"status":state,"entryMs":entry,"exitMs":exit,"grossPnl":pnl,"version":version,"reason":r["exit_trigger"].as_str().unwrap_or_else(||r["reason"].as_str().unwrap_or("Sin motivo registrado")),"overnight":overnight,"sortMs":exit.or(entry).unwrap_or(0)}));
 }
 closed.sort_by_key(|r|r.0);let(mut equity,mut peak,mut drawdown)=(0_f64,0_f64,0_f64);let(mut gains,mut losses)=(0_f64,0_f64);let(mut wins,mut lost,mut flat,mut overnight)=(0,0,0,0);let mut versions:BTreeMap<String,(usize,f64)>=BTreeMap::new();
 for(_,p,v,n)in &closed{equity+=p;peak=peak.max(equity);drawdown=drawdown.max(peak-equity);if *p>0.{wins+=1;gains+=p;}else if *p<0.{lost+=1;losses-=p;}else{flat+=1;}if *n{overnight+=1;}let row=versions.entry(v.clone()).or_default();row.0+=1;row.1+=p;}
 recent.sort_by(|a,b|b["sortMs"].as_u64().cmp(&a["sortMs"].as_u64()));recent.truncate(25);
 json!({"available":true,"generatedMs":now,"symbols":symbols,"closed":closed.len(),"open":open,"pending":pending,"incomplete":incomplete,"unfilled":cancelled,"grossPnl":equity,"wins":wins,"losses":lost,"flat":flat,"winRate":if closed.is_empty(){None}else{Some(wins as f64/closed.len() as f64*100.)},"meanPnl":if closed.is_empty(){None}else{Some(equity/closed.len() as f64)},"profitFactor":if losses>0.{Some(gains/losses)}else{None},"closedDrawdown":drawdown,"overnightClosed":overnight,"versions":versions.into_iter().map(|(version,(count,pnl))|json!({"version":version,"closed":count,"grossPnl":pnl})).collect::<Vec<_>>(),"recent":recent,"basis":"paper-fills-before-fees","automaticUpdates":false})
}
pub fn read(path:&Path,filter:Option<&str>,now:u64)->Result<Value,String>{
 if !path.exists(){return Ok(summarize(vec![],filter,now));}
 let c=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|_|"No se pudo abrir el diario Paper")?;
 c.busy_timeout(std::time::Duration::from_millis(500)).map_err(|_|"Diario ocupado")?;
 let mut q=c.prepare("SELECT payload FROM alpaca_bot_sessions").map_err(|_|"Historial Paper no disponible")?;
 let rows=q.query_map([],|r|r.get::<_,String>(0)).map_err(|_|"No se pudo consultar Paper")?.map(|r|serde_json::from_str(&r.map_err(|_|"No se pudo leer una operación")?).map_err(|_|"Operación Paper dañada")).collect::<Result<Vec<_>,_>>()?;
 let mut result=summarize(rows,filter,now);
 // Global journal metadata only; no account payloads cross the dashboard.
 result["journalAudit"]=c.query_row("SELECT COUNT(*),MAX(at_ms),COALESCE(SUM(at_ms>?1),0),COALESCE(SUM(NOT json_valid(payload)),0),COALESCE(SUM(NOT EXISTS(SELECT 1 FROM alpaca_bot_sessions s WHERE s.buy_id=d.buy_id)),0) FROM alpaca_bot_decisions d",[now.saturating_add(1000) as i64],|r|Ok(json!({"available":true,"records":r.get::<_,i64>(0)?,"lastMs":r.get::<_,Option<i64>>(1)?,"future":r.get::<_,i64>(2)?,"invalidJson":r.get::<_,i64>(3)?,"orphaned":r.get::<_,i64>(4)?,"scope":"global"}))).unwrap_or(json!({"available":false}));
 Ok(result)
}
#[cfg(test)]mod tests{
 use super::*;
 fn row(p:f64,day:&str)->Value{json!({"symbol":"QQQ261016C00750000","side":"CALL","phase":"done","entry_time":"2026-10-01T15:00:00Z","exit_time":format!("{day}T16:00:00Z"),"entry_premium":2.,"exit_premium":2.+p/100.,"last_order":{"filled_qty":"1"},"entry_context":{"management_version":"test"}})}
 #[test]fn fills_only_separate_versions_and_overnight(){let now=time(&json!("2026-10-04T17:00:00Z")).unwrap();let mut pending=row(500.,"2026-10-02");pending["phase"]=json!("sell_pending");let r=summarize(vec![row(20.,"2026-10-01"),row(-40.,"2026-10-02"),pending],None,now);assert_eq!(r["closed"],2);assert_eq!(r["pending"],1);assert!((r["grossPnl"].as_f64().unwrap()+20.).abs()<1e-8);assert!((r["closedDrawdown"].as_f64().unwrap()-40.).abs()<1e-8);assert!((r["profitFactor"].as_f64().unwrap()-0.5).abs()<1e-8);assert_eq!(r["overnightClosed"],1);assert_eq!(r["versions"][0]["closed"],2);}
 #[test]fn missing_wrong_quantity_future_and_foreign_are_not_results(){let now=time(&json!("2026-10-04T17:00:00Z")).unwrap();for field in ["exit_time","entry_premium"]{let mut r=row(20.,"2026-10-02");r[field]=Value::Null;assert_eq!(summarize(vec![r],None,now)["closed"],0);}let mut r=row(20.,"2026-10-02");r["last_order"]["filled_qty"]=json!("0");assert_eq!(summarize(vec![r],None,now)["closed"],0);assert_eq!(summarize(vec![row(20.,"2026-10-05")],None,now)["closed"],0);let r=summarize(vec![row(20.,"2026-10-02")],Some("SPY"),now);assert_eq!(r["closed"],0);assert!(r["winRate"].is_null());}
 #[test]fn real_diary_roundtrip_is_read_only(){let d=tempfile::tempdir().unwrap();let p=d.path().join("paper.sqlite");let db=crate::broker_orders::BrokerOrders::open(&p).unwrap();let mut r=row(20.,"2026-10-02");r["buy_id"]=json!("trade");db.save_runtime(&r.to_string()).unwrap();let now=time(&json!("2026-10-04T17:00:00Z")).unwrap();assert_eq!(read(&p,None,now).unwrap()["closed"],1);assert_eq!(db.bot_history().unwrap().len(),1);}
 #[test]fn thousand_results_have_bounded_display_and_fast_summary(){let now=time(&json!("2026-10-04T17:00:00Z")).unwrap();let rows=vec![row(20.,"2026-10-02");1000];let start=std::time::Instant::now();let r=summarize(rows,None,now);let ms=start.elapsed().as_secs_f64()*1000.;println!("Bot learning 1000 trades: {ms:.2} ms");assert_eq!(r["closed"],1000);assert_eq!(r["recent"].as_array().unwrap().len(),25);assert!(ms<500.);}
}
