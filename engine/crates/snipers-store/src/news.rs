//! Durable original article versions and lifecycle; no trading access.
use rusqlite::{Connection,params,OptionalExtension};
use serde_json::{Value,json};
use std::path::Path;
pub fn record(path:&Path,items:&mut [Value],now:u64)->rusqlite::Result<()> {
 let now=i64::try_from(now).map_err(|_|rusqlite::Error::InvalidQuery)?;
 let mut db=Connection::open(path)?;db.busy_timeout(std::time::Duration::from_millis(500))?;
 db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS news_articles(id TEXT PRIMARY KEY,first_seen INTEGER NOT NULL,last_seen INTEGER NOT NULL,revision INTEGER NOT NULL,original TEXT NOT NULL); CREATE TABLE IF NOT EXISTS news_versions(id TEXT NOT NULL,revision INTEGER NOT NULL,observed_ms INTEGER NOT NULL,original TEXT NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(id,revision)); CREATE TABLE IF NOT EXISTS news_lifecycle(id TEXT NOT NULL,revision INTEGER NOT NULL,stage TEXT NOT NULL,at INTEGER NOT NULL,PRIMARY KEY(id,revision,stage)); CREATE TABLE IF NOT EXISTS news_processing(id TEXT NOT NULL,revision INTEGER NOT NULL,stage TEXT NOT NULL,at INTEGER NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(id,revision,stage));")?;
 let tx=db.transaction()?;
 for item in items {
  let id=format!("Massive:Benzinga:{}",item["id"].as_str().unwrap_or(""));
  let original=json!([item["originalTitle"].as_str().or(item["title"].as_str()),item["originalSummary"].as_str().or(item["summary"].as_str()),item["tickerSymbols"],item["providerUpdatedAt"]]).to_string();
  let prev:Option<(i64,i64,String)>=tx.query_row("SELECT first_seen,revision,original FROM news_articles WHERE id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
  let first=prev.as_ref().map(|p|p.0).unwrap_or(now);
  // Queries and translation jobs can replay older snapshots. Reuse the
  // original version instead of creating another revision on every replay.
  let known:Option<i64>=tx.query_row("SELECT revision FROM news_versions WHERE id=?1 AND original=?2 ORDER BY revision DESC LIMIT 1",params![id,original],|r|r.get(0)).optional()?;
  let revision=known.unwrap_or_else(||prev.as_ref().map(|p|p.1+1).unwrap_or(1));
  let version_first:i64=tx.query_row("SELECT observed_ms FROM news_versions WHERE id=?1 AND revision=?2",params![id,revision],|r|r.get(0)).optional()?.unwrap_or(now);
  item["firstSeenRevisionMs"] = json!(version_first);
  item["firstSeenAtMs"] = json!(first);item["lastSeenAtMs"]=json!(now);item["revision"]=json!(revision);
  let published=item["publishedAt"].as_str().and_then(snipers_core::calendar::rfc3339_ms);
  item["observedDeliveryLatencyMs"]=json!(published.filter(|t|*t<=first as u64).map(|t|first as u64-t));
  let updated=item["providerUpdatedAt"].as_str().and_then(snipers_core::calendar::rfc3339_ms);
  item["observedUpdateLatencyMs"] = json!(updated.filter(|t|*t<=version_first as u64).map(|t|version_first as u64-t));
  tx.execute("INSERT INTO news_articles VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET last_seen=excluded.last_seen,revision=CASE WHEN excluded.revision>=news_articles.revision THEN excluded.revision ELSE news_articles.revision END,original=CASE WHEN excluded.revision>=news_articles.revision THEN excluded.original ELSE news_articles.original END",params![id,first,now,revision,original])?;
  tx.execute("INSERT OR IGNORE INTO news_versions VALUES(?1,?2,?3,?4,?5)",params![id,revision,now,original,item.to_string()])?;
  let stage=if item["translated"]==true{"ENRICHED"}else{"RECEIVED"};
  tx.execute("INSERT OR IGNORE INTO news_lifecycle VALUES(?1,?2,?3,?4)",params![id,revision,stage,now])?;
  tx.execute("INSERT OR IGNORE INTO news_processing VALUES(?1,?2,?3,?4,?5)",params![id,revision,stage,now,item.to_string()])?;
 } tx.commit()
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn replayed_query_versions_do_not_create_revisions_or_rollback_latest(){let dir=tempfile::tempdir().unwrap();let path=dir.path().join("news.sqlite");let mut rows=vec![json!({"id":"repeat","title":"Same","summary":"Text","tickerSymbols":["JPM"]})];record(&path,&mut rows,1000).unwrap();let old=rows.clone();rows[0]["tickerSymbols"]=json!(["JPM","QQQ"]);record(&path,&mut rows,2000).unwrap();assert_eq!(rows[0]["revision"],2);let new=rows.clone();for i in 0..20{let mut replay=if i%2==0{old.clone()}else{new.clone()};record(&path,&mut replay,3000+i).unwrap();assert_eq!(replay[0]["revision"],if i%2==0{1}else{2});}let db=Connection::open(path).unwrap();assert_eq!(db.query_row("SELECT count(*) FROM news_versions",[],|r|r.get::<_,i64>(0)).unwrap(),2);assert_eq!(db.query_row("SELECT revision FROM news_articles",[],|r|r.get::<_,i64>(0)).unwrap(),2);}
 #[test]fn revisions_preserve_original_first_seen_and_latency(){let dir=tempfile::tempdir().unwrap();let path=dir.path().join("news.sqlite");let at=snipers_core::calendar::rfc3339_ms("2026-10-13T14:00:00Z").unwrap();let mut rows=vec![json!({"id":"1","title":"Original","summary":"A","publishedAt":"2026-10-13T14:00:00Z"})];record(&path,&mut rows,at+1000).unwrap();assert_eq!(rows[0]["observedDeliveryLatencyMs"],1000);rows[0]["title"]=json!("Revised");record(&path,&mut rows,at+2000).unwrap();assert_eq!(rows[0]["revision"],2);assert_eq!(rows[0]["firstSeenAtMs"],at+1000);let db=Connection::open(path).unwrap();assert_eq!(db.query_row("SELECT count(*) FROM news_versions",[],|r|r.get::<_,i64>(0)).unwrap(),2);}
}




pub fn alerts(path:&Path,items:&[Value],now:u64)->rusqlite::Result<()> {
 if items.len()>64{return Err(rusqlite::Error::InvalidQuery);}
 let now=i64::try_from(now).map_err(|_|rusqlite::Error::InvalidQuery)?;
 let mut db=Connection::open(path)?;db.busy_timeout(std::time::Duration::from_millis(500))?;let tx=db.transaction()?;
 for item in items{
  let id=item["id"].as_str().filter(|s|!s.is_empty()&&s.len()<=64).ok_or(rusqlite::Error::InvalidQuery)?;
  let revision=item["revision"].as_i64().filter(|r|*r>0).ok_or(rusqlite::Error::InvalidQuery)?;
  let stage=if item["enriched"]==true{"ALERT_CREATED_ENRICHED"}else{"ALERT_CREATED_RAW"};
  let key=format!("Massive:Benzinga:{id}");
  tx.execute("INSERT OR IGNORE INTO news_lifecycle SELECT ?1,?2,?3,?4 WHERE EXISTS(SELECT 1 FROM news_versions WHERE id=?1 AND revision=?2)",params![key,revision,stage,now])?;
 }tx.commit()
}

#[cfg(test)]mod lifecycle_tests{
 use super::*;
 #[test]fn alert_creation_is_deduplicated_and_unknown_revision_is_not_fabricated(){let dir=tempfile::tempdir().unwrap();let path=dir.path().join("news.sqlite");let mut rows=vec![json!({"id":"7","title":"Trading Halt","summary":"Reported","publishedAt":"2026-10-13T14:00:00Z"})];let at=snipers_core::calendar::rfc3339_ms("2026-10-13T14:00:00Z").unwrap();record(&path,&mut rows,at+1000).unwrap();let notices=vec![json!({"id":"7","revision":1,"enriched":false}),json!({"id":"7","revision":5,"enriched":true})];alerts(&path,&notices,at+2000).unwrap();alerts(&path,&notices,at+3000).unwrap();let db=Connection::open(&path).unwrap();let n:i64=db.query_row("SELECT count(*) FROM news_lifecycle WHERE stage LIKE 'ALERT_%'",[],|r|r.get(0)).unwrap();assert_eq!(n,1);let t:i64=db.query_row("SELECT at FROM news_lifecycle WHERE stage='ALERT_CREATED_RAW'",[],|r|r.get(0)).unwrap();assert_eq!(t,at as i64+2000);}
}
