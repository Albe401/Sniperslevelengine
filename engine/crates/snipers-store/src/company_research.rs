//! Independent company-research catalog, revisions and durable addition queue.
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::{json, Value};
use std::path::Path;
fn open(path:&Path)->Result<Connection,String>{
 let c=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|_|"No se pudo abrir la base de compañías")?;
 c.busy_timeout(std::time::Duration::from_secs(3)).map_err(|_|"Base ocupada")?;Ok(c)
}
pub fn catalog(path:&Path)->Result<Value,String>{
 if !path.is_file(){return Ok(json!({"rows":[],"databaseAvailable":false}));}
 let c=open(path)?;
 let mut q=c.prepare("SELECT c.symbol,c.name,c.sector,c.status,(SELECT payload_json FROM analyses a WHERE a.symbol=c.symbol ORDER BY revision DESC LIMIT 1) FROM companies c WHERE in_watchlist=1 ORDER BY c.symbol LIMIT 500").map_err(|_|"Esquema de compañías no disponible")?;
 let records=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?))).map_err(|_|"No se pudo leer el catálogo")?;
 let mut rows=vec![];
 for record in records{let(symbol,name,sector,status,raw)=record.map_err(|_|"Registro no disponible")?;
  let a:Value=raw.as_deref().map(serde_json::from_str).transpose().map_err(|_|"Análisis guardado inválido")?.unwrap_or(Value::Null);
  if !a.is_null()&&a["symbol"].as_str()!=Some(symbol.as_str()){return Err("Identidad del análisis incompatible".into());}
  rows.push(json!({"symbol":symbol,"name":name,"sector":sector,"status":status,"asOf":a["asOf"],"annualPeriods":a["fundamentals"]["annualPeriods"],"observedYears":a["fundamentals"]["observedYears"],"revenueCagrPct":a["fundamentals"]["revenueCagrPct"],"peTtm":a["valuation"]["peTtm"],"dailyEma21":a["technical"]["daily"]["emas"]["21"]["value"],"weeklyEma21":a["technical"]["weekly"]["emas"]["21"]["value"],"hasAnalysis":!a.is_null()}));
 }
 Ok(json!({"databaseAvailable":true,"rows":rows}))
}
pub fn detail(path:&Path,symbol:&str)->Result<Option<Value>,String>{
 if symbol.is_empty()||symbol.len()>12||!symbol.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit()||b==b'.'||b==b'-'){return Err("Símbolo inválido".into());}
 if !path.is_file(){return Ok(None);}
 let c=open(path)?;
 let raw:Option<String>=c.query_row("SELECT payload_json FROM analyses WHERE symbol=?1 ORDER BY revision DESC LIMIT 1",[symbol],|r|r.get(0)).optional().map_err(|_|"No se pudo leer la ficha")?;
 raw.map(|raw|{if raw.len()>2_000_000{return Err("Ficha excede límite".into());}let a:Value=serde_json::from_str(&raw).map_err(|_|"Ficha inválida")?;if a["symbol"].as_str()!=Some(symbol){return Err("Ficha de otro activo".into());}Ok(a)}).transpose()
}
pub fn enqueue(path:&Path,symbol:&str,name:&str)->Result<(),String>{
 if symbol.is_empty()||symbol.len()>12||!symbol.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit()||b==b'.'||b==b'-')||name.len()>300{return Err("Compañía inválida".into());}
 let mut c=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|_|"Base de compañías no disponible")?;
 c.busy_timeout(std::time::Duration::from_secs(5)).map_err(|_|"Base ocupada")?;
 let tx=c.transaction().map_err(|_|"Cola ocupada")?;
 tx.execute("INSERT INTO companies(symbol,name,sector,kind,in_watchlist,status,updated_at) VALUES(?1,?2,NULL,'STOCK',1,'queued',datetime('now')) ON CONFLICT(symbol) DO UPDATE SET in_watchlist=1",[symbol,name]).map_err(|_|"No se pudo registrar compañía")?;
 tx.execute("INSERT INTO jobs(symbol,state,attempts,error,updated_at) VALUES(?1,'queued',0,NULL,datetime('now')) ON CONFLICT(symbol) DO NOTHING",[symbol]).map_err(|_|"No se pudo registrar tarea")?;
 tx.commit().map_err(|_|"No se pudo guardar cola".to_string())
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn missing_database_is_pending_and_never_created(){let d=tempfile::tempdir().unwrap();let p=d.path().join("missing.sqlite");assert_eq!(catalog(&p).unwrap()["databaseAvailable"],false);assert!(!p.exists());}
 #[test]fn reads_latest_revision_and_preserves_removed_company_detail(){let d=tempfile::tempdir().unwrap();let p=d.path().join("companies.sqlite");let c=Connection::open(&p).unwrap();c.execute_batch("CREATE TABLE companies(symbol TEXT,name TEXT,sector TEXT,status TEXT,in_watchlist INTEGER);CREATE TABLE analyses(symbol TEXT,revision TEXT,payload_json TEXT);INSERT INTO companies VALUES('AAPL','Apple','Tech','saved',1),('OLD','Old',NULL,'saved',0);INSERT INTO analyses VALUES('AAPL','1','{\"symbol\":\"AAPL\",\"asOf\":\"old\"}'),('AAPL','2','{\"symbol\":\"AAPL\",\"asOf\":\"new\"}'),('OLD','1','{\"symbol\":\"OLD\"}');").unwrap();drop(c);let r=catalog(&p).unwrap();assert_eq!(r["rows"].as_array().unwrap().len(),1);assert_eq!(r["rows"][0]["asOf"],"new");assert!(detail(&p,"OLD").unwrap().is_some());assert!(detail(&p,"../AAPL").is_err());}
 #[test]fn refuses_foreign_payload(){let d=tempfile::tempdir().unwrap();let p=d.path().join("companies.sqlite");let c=Connection::open(&p).unwrap();c.execute_batch("CREATE TABLE analyses(symbol TEXT,revision TEXT,payload_json TEXT);INSERT INTO analyses VALUES('AAPL','1','{\"symbol\":\"SPY\"}');").unwrap();drop(c);assert!(detail(&p,"AAPL").is_err());}
 #[test]fn enqueue_is_idempotent_and_preserves_existing_analysis(){let d=tempfile::tempdir().unwrap();let p=d.path().join("companies.sqlite");let c=Connection::open(&p).unwrap();c.execute_batch("CREATE TABLE companies(symbol TEXT PRIMARY KEY,name TEXT,sector TEXT,kind TEXT,in_watchlist INTEGER,status TEXT,updated_at TEXT);CREATE TABLE jobs(symbol TEXT PRIMARY KEY,state TEXT,attempts INTEGER,error TEXT,updated_at TEXT);CREATE TABLE analyses(symbol TEXT,revision TEXT,payload_json TEXT);INSERT INTO companies VALUES('AAPL','Apple','Tech','STOCK',0,'saved','old');INSERT INTO jobs VALUES('AAPL','saved',1,NULL,'old');INSERT INTO analyses VALUES('AAPL','1','{\"symbol\":\"AAPL\"}');").unwrap();drop(c);enqueue(&p,"AAPL","New name").unwrap();enqueue(&p,"AAPL","New name").unwrap();enqueue(&p,"NEW","New company").unwrap();let r=catalog(&p).unwrap();assert_eq!(r["rows"].as_array().unwrap().len(),2);assert_eq!(r["rows"][0]["status"],"saved");assert_eq!(r["rows"][0]["name"],"Apple");assert!(detail(&p,"AAPL").unwrap().is_some());}
}
