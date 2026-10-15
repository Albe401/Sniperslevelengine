//! Certified experimental context; never modifies entry, management or quantitative weights.
use serde_json::{json, Value};
use massive_feed::intraday_strategy::Reading;

pub fn attach(status:&Value,frame:&Value,reading:&mut Reading,now:u64){
    reading.quick_context["optionActivity"]=context(status,frame,&reading.symbol,now);
}
fn context(status:&Value,frame:&Value,symbol:&str,now:u64)->Value {
    let mut result=json!({"version":"bot-option-activity-v1","symbol":symbol,"at":now,
        "state":"pending","reason":"Esperando evidencia vigente","windowMinutes":5,
        "volume":null,"premium":null,"coincidence":null,"contracts":[],
        "direction":null,"experimental":true,"formulaChanged":false,"affectsExecution":false});
    let connected=frame["symbol"]==symbol&&frame["stocks"]["phase"]=="streaming"
        &&frame["publishedMs"].as_u64().is_some_and(|t|t<=now&&now-t<=3000);
    if symbol!="QQQ"{result["reason"]=json!("Motor disponible para QQQ");return result;}
    if !connected{return result;}
    let audit=&status["audit"];
    if status["status"]!="OBSERVING"||!audit["generatedMs"].as_u64().is_some_and(|t|t<=now&&now-t<=30000){return result;}
    let date=snipers_core::calendar::date_at(now).map(|d|d.to_string()).unwrap_or_default();
    let rows:Vec<_>=audit["reconciliation"].as_array().into_iter().flatten()
        .filter(|r|r["underlying"]==symbol&&r["date"]==date&&r["endMs"].as_u64().is_some_and(|t|t<=now)
            &&r["startMs"].as_u64().zip(r["endMs"].as_u64()).is_some_and(|(s,e)|e.checked_sub(s)==Some(300000)))
        .collect();
    let Some(end)=rows.iter().filter_map(|r|r["endMs"].as_u64()).max().filter(|t|now-t<=420000)else{return result;};
    let engine=|v:&Value|->Value {
        if !v.is_object()||!v["readingAvailableMs"].as_u64().is_some_and(|t|t<=now){Value::Null}else{v.clone()}
    };
    let contracts:Vec<_>=rows.into_iter().filter(|r|r["endMs"]==end).map(|r|json!({
        "ticker":r["ticker"],"startMs":r["startMs"],"endMs":r["endMs"],
        "priceContextCompatible":r["priceContextCompatible"],"volume":engine(&r["volume"]),"premium":engine(&r["premium"])
    })).collect();
    let counts=|kind:&str| {
        let observed:Vec<_>=contracts.iter().filter(|r|if kind=="coincidence"{r["volume"].is_object()&&r["premium"].is_object()&&r["priceContextCompatible"]==true}else{r[kind].is_object()}).collect();
        let comparable:Vec<_>=observed.iter().filter(|r|if kind=="coincidence"{r["volume"]["comparable"]==true&&r["premium"]["comparable"]==true}else{r[kind]["comparable"]==true}).collect();
        let unusual=comparable.iter().filter(|r|if kind=="coincidence"{r["volume"]["unusual"]==true&&r["premium"]["unusual"]==true}else{r[kind]["unusual"]==true}).count();
        json!({"observed":observed.len(),"comparable":comparable.len(),"withoutReference":observed.len()-comparable.len(),
            "unusual":if comparable.is_empty(){Value::Null}else{json!(unusual)},"state":if comparable.is_empty(){"pending_reference"}else{"available"}})
    };
    result["volume"]=counts("volume");result["premium"]=counts("premium");result["coincidence"]=counts("coincidence");
    result["state"]=json!(if result["volume"]["state"]=="available"||result["premium"]["state"]=="available"{"available_experimental"}else{"pending_reference"});
    result["reason"]=json!("Actividad descriptiva; prima bruta y volumen no identifican compra neta ni dirección");
    result["windowEndMs"]=json!(end);result["auditGeneratedMs"]=audit["generatedMs"].clone();result["contracts"]=json!(contracts);
    result
}

#[cfg(test)]mod tests {
 use super::*;
 fn fixture()->(Value,Value,u64){
  let now=snipers_core::calendar::legacy_eastern_ms("2026-10-12 10:40:20").unwrap();
  let engine=json!({"readingAvailableMs":now-1000,"comparable":true,"unusual":true,"sample":{"premium":50000,"kind":"CALL"}});
  (json!({"status":"OBSERVING","audit":{"generatedMs":now,"reconciliation":[{"underlying":"QQQ","date":"2026-10-12","ticker":"contract","startMs":now-320000,"endMs":now-20000,"priceContextCompatible":true,"volume":engine,"premium":engine}]}}),json!({"symbol":"QQQ","stocks":{"phase":"streaming"},"publishedMs":now}),now)
 }
 #[test]fn both_engines_and_evidence_reach_native_reading_without_changing_decisions(){
  let(s,f,now)=fixture();let mut r=Reading{symbol:"QQQ".into(),eligible:true,management_ready:true,entry_side:"PUT".into(),..Default::default()};
  r.quick_context=json!({"cvd":{"side":"PUT"}});let before=serde_json::to_value(&r).unwrap();attach(&s,&f,&mut r,now);
  let context=&r.quick_context["optionActivity"];assert_eq!(context["coincidence"]["unusual"],1);assert_eq!(context["contracts"][0]["premium"]["sample"]["premium"],50000);assert!(context["direction"].is_null());
  let mut after=serde_json::to_value(&r).unwrap();after["quick_context"].as_object_mut().unwrap().remove("optionActivity");assert_eq!(after,before);
  let report=massive_feed::intraday_strategy::Tracker::default().update(&r,0,None);assert_eq!(report["reading"]["quick_context"]["optionActivity"],*context);
 }
 #[test]fn insufficient_reference_is_pending_not_zero(){let(mut s,f,now)=fixture();s["audit"]["reconciliation"][0]["premium"]["comparable"]=json!(false);let c=context(&s,&f,"QQQ",now);assert_eq!(c["volume"]["unusual"],1);assert!(c["premium"]["unusual"].is_null());assert!(c["coincidence"]["unusual"].is_null());}
 #[test]fn stale_disconnected_foreign_future_and_old_windows_remain_pending(){
  for kind in 0..7 {let(mut s,mut f,now)=fixture();let mut symbol="QQQ";match kind{0=>f["stocks"]["phase"]=json!("disconnected"),1=>f["publishedMs"]=json!(now-3001),2=>s["audit"]["generatedMs"]=json!(now-30001),3=>symbol="SPY",4=>s["audit"]["generatedMs"]=json!(now+1),5=>s["audit"]["reconciliation"][0]["date"]=json!("2026-10-09"),_=>{s["audit"]["reconciliation"][0]["startMs"]=json!(now-800000);s["audit"]["reconciliation"][0]["endMs"]=json!(now-500000);}}let c=context(&s,&f,symbol,now);assert_eq!(c["state"],"pending","case {kind}");assert!(c["contracts"].as_array().unwrap().is_empty());}
 }
 #[test]fn mismatched_price_and_future_availability_cannot_confirm_coincidence(){let(mut s,f,now)=fixture();s["audit"]["reconciliation"][0]["priceContextCompatible"]=json!(false);assert!(context(&s,&f,"QQQ",now)["coincidence"]["unusual"].is_null());s["audit"]["reconciliation"][0]["premium"]["readingAvailableMs"]=json!(now+1);assert_eq!(context(&s,&f,"QQQ",now)["premium"]["observed"],0);}
 #[test]fn existing_decision_diary_preserves_new_engine_context(){
  let(s,f,now)=fixture();let dir=tempfile::tempdir().unwrap();let db=snipers_store::broker_orders::BrokerOrders::open(&dir.path().join("paper.sqlite")).unwrap();
  let payload=json!({"audit_version":"decision-evidence-v1","at_ms":now,"reading":{"reading":{"quick_context":{"optionActivity":context(&s,&f,"QQQ",now)}}}});
  db.record_bot_decision("trade","entry",now,&payload.to_string()).unwrap();let rows=db.bot_decisions("trade").unwrap();assert_eq!(rows[0]["reading"]["reading"]["quick_context"]["optionActivity"]["coincidence"]["unusual"],1);
 }
}
