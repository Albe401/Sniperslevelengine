//! Coverage of identified option contracts. Never a directional probability.
//! Independent reference population is mandatory for receipt percentages.
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap,BTreeSet};

#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Metric { pub passed:usize, pub denominator:usize, pub percent:Option<f64> }
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Report {
 pub schema:&'static str,pub symbol:String,pub scope:String,pub checked_ms:u64,
 pub expected_contracts:Option<usize>,pub received_unique_contracts:usize,
 pub duplicate_rows:usize,pub unidentified_rows:usize,pub unexpected_contracts:usize,
 pub metrics:BTreeMap<String,Metric>,pub failed_contracts:BTreeMap<String,Vec<String>>,
 pub live_tick_reception_percent:Option<f64>,pub engine_retention_percent:Option<f64>,
 pub execution_enabled:bool,
}
pub fn percent(passed:usize,total:usize)->Option<f64>{
 (total>0&&passed<=total).then(||100.*passed as f64/total as f64)
}
fn number(v:&Value)->Option<f64>{v.as_f64().filter(|x|x.is_finite())}
/// Input uses preserved Massive REST rows. `reference_complete` certifies pagination,
/// not all market coverage. Thresholds are explicit project conventions.
pub fn evaluate(symbol:&str,scope:&str,reference:Option<&[Value]>,reference_complete:bool,
 rows:&[Value],now:u64,max_quote_age_ms:u64)->Result<Report,&'static str>{
 if symbol.is_empty()||scope.is_empty()||now==0{return Err("Identity, scope and timestamp required");}
 let mut unique=BTreeMap::new();let mut duplicates=0;let mut unidentified=0;
 for row in rows {if let Some(t)=row["details"]["ticker"].as_str().filter(|t|!t.is_empty()){
  if unique.insert(t.to_owned(),row).is_some(){duplicates+=1;}
 }else{unidentified+=1;}}
 let expected=if reference_complete {let reference=reference.ok_or("Complete reference missing")?;
  let mut set=BTreeSet::new();for r in reference{
   let t=r["ticker"].as_str().filter(|t|!t.is_empty()).ok_or("Reference identity missing")?;
   set.insert(t.to_owned());
  }Some(set)
 }else{None};
 let population=expected.clone().unwrap_or_else(||unique.keys().cloned().collect());
 let names=["received","bidAskValid","quoteFresh","ivPresent","openInterestPresent","deltaGammaPresent","premiumMultiplierVerified","pricingUsable","structuralUsable"];
 let mut counts:BTreeMap<String,usize>=names.iter().map(|n|(n.to_string(),0)).collect();let mut failures=BTreeMap::new();
 for ticker in &population {
  let empty=Value::Null;let row=unique.get(ticker).copied().unwrap_or(&empty);let q=&row["last_quote"];
  let bid=number(&q["bid"]);let ask=number(&q["ask"]);
  let valid_quote=bid.zip(ask).is_some_and(|(b,a)|b>0.&&a>=b);
  let fresh=q["last_updated"].as_u64().map(|n|n/1_000_000).is_some_and(|t|t>0&&t<=now&&now-t<=max_quote_age_ms);
  let iv=number(&row["implied_volatility"]).is_some_and(|v|v>0.&&v<=10.);
  let oi=number(&row["open_interest"]).is_some_and(|v|v>=0.);
  let greeks=number(&row["greeks"]["delta"]).is_some_and(|v|(-1. ..=1.).contains(&v))&&number(&row["greeks"]["gamma"]).is_some_and(|v|v>=0.);
  // Use reference certification; never treat deliverable shares as the premium multiplier.
  let multiplier=reference.and_then(|rs|rs.iter().find(|r|r["ticker"]==ticker.as_str())).and_then(|r|crate::options::verified_multiplier(r,symbol)).is_some();
  let received=unique.contains_key(ticker);
  let values=[received,valid_quote,fresh,iv,oi,greeks,multiplier,received&&valid_quote&&fresh&&iv,received&&oi&&greeks&&multiplier];
  let mut reasons=vec![];for (name,valid) in names.iter().zip(values){if valid{*counts.get_mut(*name).unwrap()+=1;}else{reasons.push(name.to_string());}}
  if !reasons.is_empty(){failures.insert(ticker.clone(),reasons);}
 }
 let metrics=counts.into_iter().map(|(name,passed)|{
  let value=Metric{passed,denominator:population.len(),percent:if name=="received"&&expected.is_none(){None}else{percent(passed,population.len())}};(name,value)
 }).collect();
 let unexpected=expected.as_ref().map_or(0,|set|unique.keys().filter(|t|!set.contains(*t)).count());
 Ok(Report{schema:"option-data-coverage-v1",symbol:symbol.into(),scope:scope.into(),checked_ms:now,expected_contracts:expected.as_ref().map(BTreeSet::len),received_unique_contracts:unique.len(),duplicate_rows:duplicates,unidentified_rows:unidentified,unexpected_contracts:unexpected,metrics,failed_contracts:failures,live_tick_reception_percent:None,engine_retention_percent:None,execution_enabled:false})
}
#[cfg(test)]mod tests{
 use super::*;use serde_json::json;
 #[test]fn missing_and_duplicates_keep_denominator(){let r=evaluate("QQQ","test",Some(&[json!({"ticker":"a"}),json!({"ticker":"b"})]),true,&[json!({"details":{"ticker":"a"}}),json!({"details":{"ticker":"a"}})],1000,120000).unwrap();assert_eq!(r.metrics["received"].percent,Some(50.));assert_eq!(r.duplicate_rows,1);assert_eq!(r.metrics["pricingUsable"].percent,Some(0.));}
 #[test]fn no_reference_no_receipt_percentage(){let r=evaluate("QQQ","test",None,false,&[json!({"details":{"ticker":"a"}})],1000,120000).unwrap();assert_eq!(r.metrics["received"].percent,None);assert_eq!(r.expected_contracts,None);assert_eq!(r.engine_retention_percent,None);}
 #[test]fn empty_and_impossible_have_no_percent(){assert_eq!(percent(0,0),None);assert_eq!(percent(2,1),None);assert_eq!(percent(1,2),Some(50.));}
 #[test]fn stale_future_quotes_fail(){for stamp in [1_u64,1_000_001_000_000]{let rows=[json!({"details":{"ticker":"a"},"last_quote":{"bid":1,"ask":2,"last_updated":stamp},"implied_volatility":0.2})];let r=evaluate("QQQ","test",Some(&[json!({"ticker":"a"})]),true,&rows,1_000_000,120000).unwrap();assert_eq!(r.metrics["pricingUsable"].percent,Some(0.));}}
}
