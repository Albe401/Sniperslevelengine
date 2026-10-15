//! Bounded OI preparation, never an inference of buying or selling.
use snipers_core::options::Observation;
pub fn oi(o: &Observation) -> serde_json::Value {
    let expiry=o.contracts.values().filter(|c|c.expiration>=o.observed_date).map(|c|c.expiration.as_str()).min();
    let Some(expiry)=expiry else {return serde_json::Value::Null};
    let mut rows:Vec<_>=o.contracts.values().filter(|c|c.expiration==expiry).collect();
    let missing=rows.iter().filter(|c|c.open_interest.is_none_or(|n|!n.is_finite()||n<0.)).count();
    let total=rows.len();
    let sum=|kind:&str|rows.iter().filter(|c|c.kind==kind).filter_map(|c|c.open_interest.filter(|n|n.is_finite()&&*n>=0.)).sum::<f64>();
    let call=sum("call");let put=sum("put");
    if !call.is_finite()||!put.is_finite(){return serde_json::Value::Null;}
    rows.sort_by(|a,b|b.open_interest.unwrap_or(-1.).total_cmp(&a.open_interest.unwrap_or(-1.)).then(a.ticker.cmp(&b.ticker)));
    let top=|kind:&str|rows.iter().filter(|c|c.kind==kind&&c.open_interest.is_some_and(|n|n.is_finite()&&n>=0.)).take(3).map(|c|serde_json::json!({"ticker":c.ticker,"strike":c.strike,"oi":c.open_interest})).collect::<Vec<_>>();
    serde_json::json!({"symbol":o.symbol,"observedMs":o.observed_ms,"observedDate":o.observed_date,"referenceDate":o.oi_reference_date,"expiration":expiry,"contracts":total,"missing":missing,"call":call,"put":put,"callTop":top("call"),"putTop":top("put")})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observation()->Observation {
        let c=snipers_core::options::normalize(&serde_json::json!({"details":{"ticker":"O:QQQ261005C00745000","strike_price":745,"expiration_date":"2026-10-05","contract_type":"call"},"open_interest":100}),"QQQ").unwrap();
        Observation{schema:1,symbol:"QQQ".into(),observed_ms:1000,observed_date:"2026-10-05".into(),oi_reference_date:None,contracts:[(c.ticker.clone(),c)].into()}
    }
    #[test] fn retains_missing_greeks_and_never_claims_a_reference_date(){let o=observation();let d=oi(&o);assert_eq!(d["call"].as_f64(),Some(100.));assert_eq!(d["contracts"],1);assert!(d["referenceDate"].is_null());assert_eq!(d["callTop"][0]["strike"].as_f64(),Some(745.));}
    #[test] fn missing_is_not_observed_zero(){let mut o=observation();o.contracts.values_mut().next().unwrap().open_interest=None;let d=oi(&o);assert_eq!(d["missing"],1);assert_eq!(d["callTop"],serde_json::json!([]));o.contracts.values_mut().next().unwrap().open_interest=Some(0.);assert_eq!(oi(&o)["missing"],0);}
    #[test] fn expired_contracts_never_seed_preparation(){let mut o=observation();o.contracts.values_mut().next().unwrap().expiration="2026-10-02".into();assert!(oi(&o).is_null());}
}
