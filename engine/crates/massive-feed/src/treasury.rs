//! Daily macro context only. No index entitlement or directional trading vote.
use crate::{credentials::Credentials, live::{SharedState,now_ms}};
use serde::Serialize;
use snipers_core::calendar;
use std::time::Duration;
#[derive(Clone, Default, Serialize)]
#[serde(rename_all="camelCase")]
pub struct Report {
    pub phase:String,
    pub date:Option<String>,
    pub previous_date:Option<String>,
    pub yield_percent:Option<f64>,
    pub change_bps:Option<f64>,
    pub checked_ms:Option<u64>,
    pub detail:String,
}
fn decode(value:&serde_json::Value,now:u64)->Result<Report,&'static str> {
    if value["status"]!="OK" {return Err("Respuesta Treasury no válida");}
    let today=calendar::date_at(now).ok_or("Fecha ET no disponible")?.to_string();
    let today_start=calendar::day_bounds(&today).ok_or("Fecha ET no disponible")?.0;
    let rows=value["results"].as_array().filter(|r|r.len()>=2&&r.len()<=10).ok_or("Faltan dos observaciones diarias comparables")?;
    let mut dates=std::collections::BTreeMap::new();
    for row in rows {
        let date=row["date"].as_str().ok_or("Fecha Treasury ausente")?;
        let at=calendar::day_bounds(date).ok_or("Fecha Treasury inválida")?.0;
        let rate=row["yield_10_year"].as_f64().filter(|n|n.is_finite()).ok_or("Rendimiento 10Y ausente")?;
        if date>today.as_str()||dates.insert(date.to_string(),(at,rate)).is_some(){return Err("Fechas futuras o duplicadas en Treasury");}
    }
    let mut ordered=dates.into_iter().rev();
    let (date,(at,rate))=ordered.next().unwrap();
    let (previous_date,(_,previous))=ordered.next().unwrap();
    let delta=(rate-previous)*100.;
    if !delta.is_finite() {return Err("Cambio Treasury inválido");}
    Ok(Report{phase:if today_start.saturating_sub(at)>7*86400000 {"stale"}else{"ready"}.into(),date:Some(date),previous_date:Some(previous_date),yield_percent:Some(rate),change_bps:Some(delta),checked_ms:Some(now),detail:"Massive Economy · rendimiento diario; no es cotización intradía ni señal CALL/PUT".into()})
}
fn due(r:&Report,now:u64)->bool {
    let interval=if r.phase=="failed" {900000}else{3600000};
    r.checked_ms.is_none_or(|at|at>now||now-at>=interval)
}
pub async fn worker(key:&Credentials,state:SharedState) {
    let Ok(client)=reqwest::Client::builder().timeout(Duration::from_secs(12)).redirect(reqwest::redirect::Policy::none()).build()else{return};
    let mut timer=tokio::time::interval(Duration::from_secs(60));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        timer.tick().await;
        let now=now_ms();
        if !due(&state.lock().unwrap().treasury,now){continue;}
        let result=async {
            let response=client.get("https://api.massive.com/fed/v1/treasury-yields").query(&[("limit","3"),("sort","date.desc")]).bearer_auth(&key.0).send().await.map_err(|_|"Consulta Treasury no disponible")?;
            if !response.status().is_success(){return Err("Acceso Treasury rechazado o servicio no disponible");}
            let bytes=response.bytes().await.map_err(|_|"Respuesta Treasury no disponible")?;
            if bytes.len()>100000{return Err("Respuesta Treasury excede límite");}
            let value=serde_json::from_slice(&bytes).map_err(|_|"Respuesta Treasury ilegible")?;
            decode(&value,now_ms())
        }.await;
        let mut s=state.lock().unwrap();
        match result {Ok(report)=>s.treasury=report,Err(detail)=>{s.treasury.phase="failed".into();s.treasury.checked_ms=Some(now_ms());s.treasury.detail=detail.into();}}
    }
}
#[cfg(test)]mod tests {
    use super::*;
    fn now()->u64{calendar::legacy_eastern_ms("2026-10-02 10:00:00").unwrap()}
    fn data()->serde_json::Value{serde_json::json!({"status":"OK","results":[{"date":"2026-10-01","yield_10_year":5.24},{"date":"2026-09-30","yield_10_year":5.29}]})}
    #[test]fn converts_percentage_points_to_basis_points(){let r=decode(&data(),now()).unwrap();assert!((r.change_bps.unwrap()+5.).abs()<1e-8);assert_eq!(r.date.as_deref(),Some("2026-10-01"));assert_eq!(r.phase,"ready");}
    #[test]fn future_duplicate_and_missing_rates_are_not_zero(){for kind in 0..3 {let mut d=data();if kind==0{d["results"][0]["date"]="2026-10-03".into();}if kind==1{d["results"][1]["date"]="2026-10-01".into();}if kind==2{d["results"][0]["yield_10_year"]=serde_json::Value::Null;}assert!(decode(&d,now()).is_err());}}
    #[test]fn old_observation_is_flagged(){let later=calendar::legacy_eastern_ms("2026-10-12 10:00:00").unwrap();assert_eq!(decode(&data(),later).unwrap().phase,"stale");}
    #[test]fn cache_and_failure_retries_are_bounded(){let mut r=decode(&data(),now()).unwrap();assert!(!due(&r,now()+3599999));assert!(due(&r,now()+3600000));r.phase="failed".into();assert!(!due(&r,now()+899999));assert!(due(&r,now()+900000));}
}
