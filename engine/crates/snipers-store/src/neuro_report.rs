//! Empirical directional outcomes, separate from model scores and trading decisions.
use rusqlite::{params, Connection, OpenFlags};
use serde_json::{json, Value};
use std::path::Path;

pub fn read(path: &Path, symbol: &str, variant: &str, now: u64) -> rusqlite::Result<Value> {
    let mut base = crate::learning_report::read(path, Some(symbol), now)?;
    base["scores"] = json!([]);
    if !path.exists() { return Ok(base); }
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.busy_timeout(std::time::Duration::from_millis(500))?;
    let mut scores = Vec::new();
    for source in ["neuro", "snipers"] {
        let query = "SELECT horizon_ms, json_extract(context_json,?3), COUNT(*),
          SUM(CASE WHEN return_bps * CASE json_extract(context_json,?3) WHEN 'CALL' THEN 1 ELSE -1 END > 5 THEN 1 ELSE 0 END),
          SUM(CASE WHEN return_bps * CASE json_extract(context_json,?3) WHEN 'CALL' THEN 1 ELSE -1 END < -5 THEN 1 ELSE 0 END)
          FROM outcomes JOIN observations ON observations.id=outcomes.observation_id
          WHERE symbol=?1 AND eligible=1 AND status='observed'
          AND json_extract(context_json,'$.variant')=?2
          AND COALESCE(json_extract(context_json,'$.synthetic'),0)=0
          AND json_extract(context_json,?3) IN ('CALL','PUT')
          GROUP BY horizon_ms,json_extract(context_json,?3)";
        let rows = db.prepare(query)?.query_map(params![symbol, variant, format!("$.{source}.side")], |r| {
            let n: i64 = r.get(2)?; let wins: i64 = r.get(3)?; let losses: i64 = r.get(4)?;
            Ok(json!({"source":source,"minutes":r.get::<_,i64>(0)?/60000,"side":r.get::<_,String>(1)?,
                "n":n,"wins":wins,"losses":losses,"flat":n-wins-losses,
                "rate":if wins+losses>=30 {Some(100.0*wins as f64/(wins+losses) as f64)} else {None}}))
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        scores.extend(rows);
    }
    base["scores"] = json!(scores);
    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::{LearningStore,Observation};
    #[test]
    fn separates_configuration_sides_quality_and_flat_results() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("neuro.sqlite");
        let t=snipers_core::calendar::rfc3339_ms("2026-09-24T15:00:00Z").unwrap();
        let mut store=LearningStore::open(&path).unwrap();
        for i in 0..35u64 {
            let now=t+i*30_000;
            store.record(&Observation{symbol:"QQQ".into(),observed_ms:now,price:Some(100.0),price_ms:Some(now),
                eligible:i!=33,quality:"TEST".into(),context:json!({"variant":if i==32{"other"}else{"v1"},
                    "synthetic":i==34,"neuro":{"side":"CALL"},"snipers":{"side":"PUT"}})}).unwrap();
        }
        let db=Connection::open(&path).unwrap();
        db.execute("UPDATE outcomes SET status='observed',return_bps=10 WHERE horizon_ms=300000",[]).unwrap();
        db.execute("UPDATE outcomes SET return_bps=-10 WHERE observation_id=1 AND horizon_ms=300000",[]).unwrap();
        db.execute("UPDATE outcomes SET return_bps=0 WHERE observation_id=2 AND horizon_ms=300000",[]).unwrap();
        let report=read(&path,"QQQ","v1",t+2_000_000).unwrap();
        let rows:Vec<_>=report["scores"].as_array().unwrap().iter().filter(|r|r["minutes"]==5).collect();assert_eq!(rows.len(),2);
        assert_eq!(rows[0]["n"],32);assert_eq!(rows[0]["wins"],30);assert_eq!(rows[0]["losses"],1);assert_eq!(rows[0]["flat"],1);
        assert!(rows[0]["rate"].as_f64().unwrap()>96.0);assert_eq!(rows[1]["wins"],1);
        assert!(read(&path,"QQQ","other",t).unwrap()["scores"][0]["rate"].is_null());
        assert!(read(&path,"SPY","v1",t).unwrap()["scores"].as_array().unwrap().is_empty());
    }
}
