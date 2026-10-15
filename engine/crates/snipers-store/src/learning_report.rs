//! Bounded, read-only view of the learning database for the desktop panel.
use rusqlite::{params, Connection, OpenFlags};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};

pub fn read(path: &Path, symbol: Option<&str>, now: u64) -> rusqlite::Result<Value> {
    let now =
        i64::try_from(now).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let mut out = json!({"exists":false,"symbols":[],"observations":0,"eligible":0,"sessions":0,
        "buildSessions":0,"outcomes":[],"recent":[],"quality":[],"directionalCohorts":[],"cohortScope":"Últimos 30 días, máximo 10000 lecturas por consulta; grupos por activo, versión, lado y ventana"});
    if !path.exists() {
        return Ok(out);
    }
    let mut db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.busy_timeout(Duration::from_millis(500))?;
    let db = db.transaction()?;
    let schema: String = db.query_row("SELECT schema FROM learning_meta", [], |r| r.get(0))?;
    if schema != crate::learning::SCHEMA {
        return Err(rusqlite::Error::InvalidQuery);
    }
    out["exists"] = true.into();
    let symbols = db
        .prepare("SELECT DISTINCT symbol FROM observations ORDER BY symbol LIMIT 200")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    out["symbols"] = json!(symbols);
    let (total,eligible,sessions):(i64,i64,i64)=db.query_row(
        "SELECT COUNT(*),COALESCE(SUM(eligible=1 AND COALESCE(json_extract(context_json,'$.synthetic'),0)=0),0),COUNT(DISTINCT session)
         FROM observations WHERE (?1 IS NULL OR symbol=?1)", [symbol],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    out["observations"] = total.into();
    out["eligible"] = eligible.into();
    out["sessions"] = sessions.into();
    let build_sessions:i64=db.query_row("SELECT COALESCE(MAX(n),0) FROM (
        SELECT COUNT(DISTINCT session) n FROM observations WHERE eligible=1 AND COALESCE(json_extract(context_json,'$.synthetic'),0)=0
        AND (?1 IS NULL OR symbol=?1) GROUP BY json_extract(context_json,'$.engine_build'))",[symbol],|r|r.get(0))?;
    out["buildSessions"] = build_sessions.into();
    out["quality"]=json!(db.prepare("SELECT quality,COUNT(*) FROM observations WHERE (?1 IS NULL OR symbol=?1) GROUP BY quality ORDER BY COUNT(*) DESC")?
        .query_map([symbol],|r|Ok(json!({"reason":r.get::<_,String>(0)?,"count":r.get::<_,i64>(1)?})))?.collect::<rusqlite::Result<Vec<_>>>()?);
    let mut horizons = Vec::new();
    for horizon in crate::learning::HORIZONS {
        let value = db.query_row(
            "SELECT COALESCE(SUM(status='observed'),0),
            COALESCE(SUM(status='pending' AND target_ms+60000>=?2),0),
            COALESCE(SUM(status='missing' OR (status='pending' AND target_ms+60000<?2)),0),
            AVG(CASE WHEN status='observed' THEN return_bps END)
            FROM outcomes JOIN observations ON observations.id=outcomes.observation_id
            WHERE horizon_ms=?1 AND (?3 IS NULL OR symbol=?3)",
            params![horizon, now, symbol],
            |r| {
                Ok(json!({
                "minutes":horizon/60_000,"observed":r.get::<_,i64>(0)?,"pending":r.get::<_,i64>(1)?,
                "missing":r.get::<_,i64>(2)?,"meanBps":r.get::<_,Option<f64>>(3)?}))
            },
        )?;
        horizons.push(value);
    }
    out["outcomes"] = json!(horizons);
    out["directionalCohorts"]=json!(db.prepare(
        "WITH sample AS (
          SELECT id,symbol,session,eligible,
          json_extract(context_json,'$.analyst.quick_read.quick_read_side') side,
          json_extract(context_json,'$.engine_build') build,
          COALESCE(json_extract(context_json,'$.synthetic'),0) synthetic FROM observations
          WHERE (?1 IS NULL OR symbol=?1) AND observed_ms<=?2 AND observed_ms>=?3
          ORDER BY observed_ms DESC,id DESC LIMIT 10000
        ), labels AS (
          SELECT * FROM sample WHERE eligible=1 AND synthetic=0
        )
        SELECT symbol,build,side,horizon_ms/60000,COUNT(*),COUNT(DISTINCT session),
        SUM(CASE WHEN (side='CALL' AND return_bps>0) OR (side='PUT' AND return_bps<0) THEN 1 ELSE 0 END),
        SUM(return_bps=0),AVG(CASE WHEN side='PUT' THEN -return_bps ELSE return_bps END)
        FROM labels JOIN outcomes ON labels.id=outcomes.observation_id
        WHERE side IN ('CALL','PUT') AND typeof(build)='text' AND LENGTH(build)>0
        AND status='observed' AND return_bps IS NOT NULL AND outcome_ms<=?2
        GROUP BY symbol,build,side,horizon_ms ORDER BY symbol,build,side,horizon_ms LIMIT 200")?
        .query_map(params![symbol,now,now.saturating_sub(30*86_400_000)],|r|Ok(json!({
            "symbol":r.get::<_,String>(0)?,"build":r.get::<_,String>(1)?,"side":r.get::<_,String>(2)?,
            "minutes":r.get::<_,i64>(3)?,"observed":r.get::<_,i64>(4)?,"sessions":r.get::<_,i64>(5)?,
            "aligned":r.get::<_,i64>(6)?,"flat":r.get::<_,i64>(7)?,"meanDirectionalBps":r.get::<_,Option<f64>>(8)?
        })))?.collect::<rusqlite::Result<Vec<_>>>()?);
    let mut recent=db.prepare("SELECT id,symbol,session,observed_ms,price,eligible,quality,
        COALESCE(json_extract(context_json,'$.gamma_map.movement.headline'),json_extract(context_json,'$.analyst.quick_read.quick_read_side')),json_extract(context_json,'$.engine_build')
        FROM observations WHERE (?1 IS NULL OR symbol=?1) ORDER BY observed_ms DESC,id DESC LIMIT 25")?
        .query_map([symbol],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"symbol":r.get::<_,String>(1)?,"session":r.get::<_,String>(2)?,
            "observedMs":r.get::<_,i64>(3)?,"price":r.get::<_,Option<f64>>(4)?,"eligible":r.get::<_,bool>(5)?,
            "quality":r.get::<_,String>(6)?,"side":r.get::<_,Option<String>>(7)?,"build":r.get::<_,Option<String>>(8)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    for row in &mut recent {
        row["outcomes"]=json!(db.prepare("SELECT horizon_ms,status,target_ms,outcome_ms,return_bps FROM outcomes WHERE observation_id=?1 ORDER BY horizon_ms")?
            .query_map([row["id"].as_i64().unwrap()],|r|{
                let status:String=r.get(1)?;let target:i64=r.get(2)?;
                Ok(json!({"minutes":r.get::<_,i64>(0)?/60_000,"status":if status=="pending" && target.saturating_add(60_000)<now {"missing"}else{&status},
                    "outcomeMs":r.get::<_,Option<i64>>(3)?,"returnBps":r.get::<_,Option<f64>>(4)?}))
            })?.collect::<rusqlite::Result<Vec<_>>>()?);
    }
    out["recent"] = json!(recent);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::{LearningStore, Observation};
    #[test]
    fn directional_groups_keep_versions_separate_and_exclude_unverified_results() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("cohorts.sqlite");
        let now=snipers_core::calendar::rfc3339_ms("2026-10-05T15:00:00Z").unwrap();
        let mut store=LearningStore::open(&path).unwrap();
        for (i,(symbol,build,side,eligible,synthetic)) in [
            ("QQQ","v1","CALL",true,false),("QQQ","v1","CALL",true,false),
            ("QQQ","v2","PUT",true,false),("SPY","v1","CALL",true,false),
            ("QQQ","v1","CALL",false,false),("QQQ","v1","CALL",true,true),
            ("QQQ","","CALL",true,false),("QQQ","v1","NEUTRAL",true,false),
        ].into_iter().enumerate(){
            let at=now+i as u64*1000;
            store.record_interval(&Observation{symbol:symbol.into(),observed_ms:at,price:Some(100.),price_ms:Some(at),eligible,quality:"READY".into(),context:json!({"engine_build":build,"synthetic":synthetic,"analyst":{"quick_read":{"quick_read_side":side}}})},1000).unwrap();
        }
        let raw=Connection::open(&path).unwrap();
        raw.execute("UPDATE outcomes SET status='observed',outcome_ms=target_ms,return_bps=CASE WHEN observation_id=2 THEN 0 WHEN observation_id=3 THEN -20 ELSE 10 END WHERE horizon_ms=300000",[]).unwrap();
        let r=read(&path,Some("QQQ"),now+400000).unwrap();let groups=r["directionalCohorts"].as_array().unwrap();
        assert_eq!(groups.len(),2);assert_eq!(groups[0]["build"],"v1");assert_eq!(groups[0]["observed"],2);assert_eq!(groups[0]["aligned"],1);assert_eq!(groups[0]["flat"],1);assert_eq!(groups[0]["meanDirectionalBps"],5.);
        assert_eq!(groups[1]["build"],"v2");assert_eq!(groups[1]["meanDirectionalBps"],20.);
        assert!(read(&path,Some("QQQ"),now).unwrap()["directionalCohorts"].as_array().unwrap().is_empty());
        assert!(read(&path,Some("QQQ"),now+31*86400000).unwrap()["directionalCohorts"].as_array().unwrap().is_empty());
    }
    #[test]
    fn report_is_bounded_and_measured_with_ten_thousand_contexts() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("speed.sqlite");
        let now=snipers_core::calendar::rfc3339_ms("2026-10-05T15:00:00Z").unwrap();
        let _store=LearningStore::open(&path).unwrap();
        let mut raw=Connection::open(&path).unwrap();let tx=raw.transaction().unwrap();
        let context=json!({"engine_build":"v1","analyst":{"quick_read":{"quick_read_side":"CALL"}},"padding":"x".repeat(4096)}).to_string();
        for i in 0..10000_i64 {
            let at=now as i64-i*1000;
            tx.execute("INSERT INTO observations(id,symbol,session,bucket,observed_ms,price,price_ms,eligible,quality,context_json) VALUES(?1,'QQQ','2026-10-05',?1,?2,100,?2,1,'READY',?3)",params![i+1,at,context]).unwrap();
            tx.execute("INSERT INTO outcomes(observation_id,horizon_ms,target_ms,status,outcome_ms,return_bps) VALUES(?1,300000,?2,'observed',?2,10)",params![i+1,now as i64]).unwrap();
        }
        tx.commit().unwrap();let start=std::time::Instant::now();
        let report=read(&path,Some("QQQ"),now).unwrap();
        let elapsed=start.elapsed();println!("Local analyst DB: {:.1} ms / 10000 contexts",elapsed.as_secs_f64()*1000.);
        assert!(elapsed<std::time::Duration::from_secs(5));
        assert_eq!(report["directionalCohorts"][0]["observed"],10000);
        assert_eq!(report["recent"].as_array().unwrap().len(),25);
    }
    #[test]
    fn missing_database_is_not_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent.sqlite");
        assert_eq!(read(&path, None, 0).unwrap()["exists"], false);
        assert!(!path.exists());
    }
    #[test]
    fn filters_and_expired_outcomes_are_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.sqlite");
        let mut db = LearningStore::open(&path).unwrap();
        let now = snipers_core::calendar::rfc3339_ms("2026-09-24T15:00:00Z").unwrap();
        for symbol in ["QQQ", "SPY"] {
            db.record(&Observation {
                symbol: symbol.into(),
                observed_ms: now,
                price: Some(100.),
                price_ms: Some(now),
                eligible: true,
                quality: "READY".into(),
                context: json!({"engine_build":"v1"}),
            })
            .unwrap();
        }
        let out = read(&path, Some("QQQ"), now + 400_000).unwrap();
        assert_eq!(out["observations"], 1);
        assert_eq!(out["recent"][0]["symbol"], "QQQ");
        assert_eq!(out["outcomes"][0]["missing"], 1);
        assert_eq!(out["buildSessions"], 1);
        assert_eq!(
            read(&path, Some("QQQ' OR 1=1"), now).unwrap()["observations"],
            0
        );
        let raw = Connection::open(&path).unwrap();
        let pending: i64 = raw
            .query_row(
                "SELECT COUNT(*) FROM outcomes WHERE status='pending'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pending, 6);
    }
}

