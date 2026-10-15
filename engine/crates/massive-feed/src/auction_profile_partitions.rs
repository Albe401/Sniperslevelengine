//! Quantitative descriptive partition, not provider-standard volume or probability.
//! Apply ONLY after the consolidated population, timing and identity admission.
use snipers_core::auction_map::{self,Input,Trade};
use std::collections::{BTreeMap,BTreeSet};
pub const ID:&str="amt-continuous-partition-20261013-v1";
/// These flags describe reported circumstances, not proof of trade intent.
pub fn separation(conditions:&[u32])->Option<&'static str>{
 if conditions.iter().any(|c|matches!(c,52|53)){Some("contingent")}
 else if conditions.contains(&2){Some("average_price")}
 else if conditions.contains(&9){Some("cross")}
 else if conditions.iter().any(|c|matches!(c,17|18|19|25|28|55)){Some("auction")}
 else{None}
}
#[derive(Clone,Default)]
pub struct State{bins:BTreeMap<(i64,u64),f64>,largest:BTreeMap<i64,f64>,counts:BTreeMap<i64,usize>,minutes:BTreeMap<i64,BTreeSet<u64>>,pub events:usize,pub shares:f64,weighted:f64}
impl State{
 fn add(&mut self,price:f64,size:f64,at:u64,open:u64){
  let row=(price/0.01+1e-9).floor() as i64;let bracket=(at-open)/1_800_000;
  *self.bins.entry((row,bracket)).or_default()+=size;
  self.largest.entry(row).and_modify(|v|*v=v.max(size)).or_insert(size);
  *self.counts.entry(row).or_default()+=1;self.minutes.entry(row).or_default().insert((at-open)/60_000);
  self.events+=1;self.shares+=size;self.weighted+=price*size;
 }
 fn diagnostic(&self,row:i64)->serde_json::Value{
  let shares:f64=self.bins.range((row,0)..=(row,u64::MAX)).map(|(_,v)|v).sum();let largest=self.largest.get(&row).copied().unwrap_or(0.);
  serde_json::json!({"shares":shares,"events":self.counts.get(&row).copied().unwrap_or(0),"activeMinutes":self.minutes.get(&row).map(|v|v.len()).unwrap_or(0),"largestTradeShares":largest,"largestTradeConcentrationPct":if shares>0.{Some(100.*largest/shares)}else{None}})
 }
 fn reading(&self,symbol:&str,open:u64,close:u64,now:u64)->Option<serde_json::Value>{
  if self.events==0{return None;}
  let trades=self.bins.iter().map(|(&(row,bracket),&volume)|Trade{symbol:symbol.into(),at:open+bracket*1_800_000,id:format!("partition:{row}:{bracket}"),price:(row as f64+0.5)*0.01,volume}).collect();
  let mut r=auction_map::read(&Input{symbol:symbol.into(),start_ms:open,end_ms:close,as_of_ms:now,bin_size:0.01,bracket_ms:1_800_000,value_fraction:0.7,complete_window:false,trades,bars:vec![]}).ok()?;
  r.accepted_trades=self.events;r.vwap=self.weighted/r.total_volume;
  let row=(r.poc_low/0.01+1e-9).floor() as i64;
  let mut v=serde_json::to_value(r).ok()?;v["populationId"]=serde_json::json!(ID);v["pocDiagnostics"]=self.diagnostic(row);v["methodologyClass"]=serde_json::json!("project_descriptive_convention");Some(v)
 }
}
#[derive(Clone,Default)]
pub struct Partitions{continuous:State,separated:State,reasons:BTreeMap<String,(usize,f64)>}
impl Partitions{
 pub fn add(&mut self,price:f64,size:f64,at:u64,open:u64,reason:Option<&str>){
  if let Some(reason)=reason{self.separated.add(price,size,at,open);let r=self.reasons.entry(reason.into()).or_default();r.0+=1;r.1+=size;}
  else{self.continuous.add(price,size,at,open);}
 }
 pub fn snapshot(&self,symbol:&str,open:u64,close:u64,now:u64,poc_low:f64,total:f64)->serde_json::Value{
  let row=(poc_low/0.01+1e-9).floor() as i64;let a=self.continuous.diagnostic(row);let b=self.separated.diagnostic(row);
  let shares=a["shares"].as_f64().unwrap_or(0.)+b["shares"].as_f64().unwrap_or(0.);let largest=a["largestTradeShares"].as_f64().unwrap_or(0.).max(b["largestTradeShares"].as_f64().unwrap_or(0.));
  serde_json::json!({"version":ID,"definition":"Consolidated admitted trades partitioned by explicit conditions; all venues including TRF retained; no size cap","excludedFromContinuousConditions":[2,9,17,18,19,25,28,52,53,55],"methodologyClass":"project_descriptive_convention","consolidated":{"populationId":snipers_core::stock_volume_policy::ID,"shares":total,"pocDiagnostics":{"shares":shares,"events":a["events"].as_u64().unwrap_or(0)+b["events"].as_u64().unwrap_or(0),"largestTradeShares":largest,"largestTradeConcentrationPct":if shares>0.{Some(100.*largest/shares)}else{None}}},"continuous":self.continuous.reading(symbol,open,close,now),"separated":{"events":self.separated.events,"shares":self.separated.shares,"reasons":self.reasons},"continuousShares":self.continuous.shares,"partitionSumShares":self.continuous.shares+self.separated.shares,"volumeBalanceDifference":total-self.continuous.shares-self.separated.shares,"calibrated":false,"marketCoveragePercent":null})
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn giant_trade_is_separated_by_condition_not_by_size_or_venue(){assert_eq!(separation(&[41,53]),Some("contingent"));assert_eq!(separation(&[41]),None);assert_eq!(separation(&[37]),None);assert_eq!(separation(&[]),None);let mut p=Partitions::default();p.add(100.,650000.,0,0,None);p.add(101.,650000.,0,0,Some("contingent"));let r=p.snapshot("QQQ",0,600000,1000,100.,1300000.);assert_eq!(r["continuous"]["pocLow"],100.);assert_eq!(r["continuousShares"],650000.);assert_eq!(r["separated"]["shares"],650000.);assert_eq!(r["volumeBalanceDifference"],0.);}
 #[test]fn no_continuous_print_is_pending_not_a_consolidated_fallback(){let mut p=Partitions::default();p.add(100.,100.,0,0,Some("auction"));assert!(p.snapshot("QQQ",0,600000,1000,100.,100.)["continuous"].is_null());}
 #[test]fn cent_assignment_ties_and_time_presence_are_quantitative(){let mut p=Partitions::default();p.add(100.009,0.25,0,0,None);p.add(100.01,0.25,60_000,0,None);let r=p.snapshot("QQQ",0,600000,120000,100.,0.5);assert_eq!(r["continuous"]["pocLow"],100.);assert_eq!(r["continuous"]["totalVolume"],0.5);assert_eq!(r["continuous"]["pocDiagnostics"]["activeMinutes"],1);}
}
