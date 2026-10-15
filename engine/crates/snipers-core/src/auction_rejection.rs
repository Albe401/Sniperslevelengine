//! Experimental rejection participation index. Not a bounce or arrival probability.
//! Frozen area/ATR; five-minute causal evaluation; no network or broker side effects.
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;
use crate::{Candle,auction_map::Trade};
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Input {
 pub symbol:String,pub upper:bool,pub frozen_val:f64,pub frozen_vah:f64,
 pub frozen_at_ms:u64,pub atr:f64,pub atr_at_ms:u64,pub test_bar_ms:u64,pub as_of_ms:u64,
 /// Adapter attests eligible underlying trades, corrections applied, and complete evaluation coverage.
 pub complete_window:bool,pub trades:Vec<Trade>,pub bars:Vec<Candle>,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase")]
pub struct Reading {
 pub version:String,pub symbol:String,pub status:String,pub reason:String,pub score:Option<f64>,
 pub distance:Option<f64>,pub permanence:Option<f64>,pub participation:Option<f64>,
 pub level:f64,pub window_start_ms:u64,pub window_end_ms:u64,pub calibrated:bool,
 pub probability:Option<f64>,
}
fn inside(p:f64,i:&Input)->bool{p>i.frozen_val&&p<i.frozen_vah}
/// D is last close's inward distance / ATR, clipped to [0,1]. T holds the last
/// eligible trade price between prints, with maximum 60s gaps. V is actual
/// inside-area trade volume / evaluation volume. T is a last-price occupancy proxy.
pub fn read(i:&Input)->Result<Reading,String>{
 if i.symbol.is_empty()||![i.frozen_val,i.frozen_vah,i.atr].iter().all(|v|v.is_finite()&&*v>0.)||i.frozen_val>=i.frozen_vah||i.frozen_at_ms>i.test_bar_ms||i.atr_at_ms>i.test_bar_ms||i.test_bar_ms%60_000!=0{return Err("Invalid frozen area, ATR or event".into());}
 let start=i.test_bar_ms.checked_add(120_000).ok_or("Timestamp overflow")?;
 let end=start.checked_add(300_000).ok_or("Timestamp overflow")?;
 let level=if i.upper{i.frozen_vah}else{i.frozen_val};
 let mut r=Reading{version:"auction-rejection-index-v1".into(),symbol:i.symbol.clone(),status:"pending".into(),reason:String::new(),score:None,distance:None,permanence:None,participation:None,level,window_start_ms:start,window_end_ms:end,calibrated:false,probability:None};
 macro_rules! pending {($s:expr)=>{{r.reason=$s.into();return Ok(r)}}}
 let mut bars=BTreeMap::new();
 for b in &i.bars {
  if b.timestamp_ms<i.test_bar_ms||b.timestamp_ms>=end||b.timestamp_ms.checked_add(60_000).is_none_or(|t|t>i.as_of_ms){continue;}
  if b.timestamp_ms%60_000!=0||![b.open,b.high,b.low,b.close].iter().all(|v|v.is_finite()&&*v>0.)||b.high<b.open.max(b.close)||b.low>b.open.min(b.close){return Err("Invalid closed candle".into());}
  if bars.insert(b.timestamp_ms,b).is_some(){return Err("Duplicate candle".into());}
 }
 let Some(test)=bars.get(&i.test_bar_ms)else{pending!("Awaiting closed test candle")};
 if if i.upper{test.high<level}else{test.low>level}{pending!("No observed level visit")}
 for t in [i.test_bar_ms+60_000,i.test_bar_ms+120_000]{
  match bars.get(&t){Some(b)if inside(b.close,i)=>{},_=>pending!("Awaiting two inside closes")}
 }
 if i.as_of_ms<end{pending!("Five-minute evaluation still forming")}
 let mut outside=0;
 for t in (start..end).step_by(60_000){
  let Some(b)=bars.get(&t)else{pending!("Missing evaluation candle")};
  let crossed=if i.upper{b.close>=level}else{b.close<=level};
  outside=if crossed{outside+1}else{0};
  if outside>=2{r.status="invalidated".into();pending!("Two closes beyond tested level")}
 }
 if !i.complete_window{pending!("Trade coverage not verified")}
 let mut ids:BTreeMap<&str,&Trade>=BTreeMap::new();
 for t in &i.trades {
  if t.at>end||t.at>i.as_of_ms||t.at<start.saturating_sub(60_000){continue;}
  if t.symbol!=i.symbol||t.id.is_empty()||!t.price.is_finite()||t.price<=0.||!t.volume.is_finite()||t.volume<=0.{return Err("Invalid eligible trade".into());}
  if let Some(old)=ids.insert(&t.id,t){if old.at!=t.at||old.price!=t.price||old.volume!=t.volume{return Err("Conflicting trade identity".into());}}
 }
 let mut ticks:Vec<_>=ids.values().copied().collect();ticks.sort_by(|a,b|a.at.cmp(&b.at).then_with(||match (a.id.parse::<u128>(),b.id.parse::<u128>()){(Ok(x),Ok(y))=>x.cmp(&y).then_with(||a.id.cmp(&b.id)),_=>a.id.cmp(&b.id)}));
 for pair in ticks.windows(2){if pair[0].at==pair[1].at&&pair[0].price!=pair[1].price{match (pair[0].id.parse::<u128>(),pair[1].id.parse::<u128>()){(Ok(a),Ok(b))if a!=b=>{},_=>pending!("Same-time trade order unavailable")}}}
 let Some(seed)=ticks.iter().rev().find(|t|t.at<=start)else{pending!("No evaluation seed price")};
 let (mut at,mut price,mut inside_ms)=(start,seed.price,0u64);
 let (mut total,mut volume_inside)=(0.,0.);
 for t in ticks.iter().filter(|t|t.at>=start&&t.at<end){
  if t.at-at>60_000{pending!("Price occupancy gap")}
  if inside(price,i){inside_ms+=t.at-at;}
  at=t.at;price=t.price;total+=t.volume;if inside(t.price,i){volume_inside+=t.volume;}
 }
 if end-at>60_000{pending!("Missing evaluation tail")}
 if inside(price,i){inside_ms+=end-at;}
 if !total.is_finite()||!volume_inside.is_finite()||total<=0.{pending!("No valid evaluation volume")}
 let close=bars[&(end-60_000)].close;
 let d=(if i.upper{level-close}else{close-level}/i.atr).clamp(0.,1.);
 let t=inside_ms as f64/300_000.;let v=volume_inside/total;
 r.status="observed".into();r.reason="Experimental participation index; not probability".into();
 r.distance=Some(d);r.permanence=Some(t);r.participation=Some(v);r.score=Some(100.*(d*t*v).cbrt());Ok(r)
}
#[cfg(test)]
mod tests{
 use super::*;
 fn input(upper:bool)->Input{
  let base=600_000;let price=if upper{101.}else{103.};
  Input{symbol:"QQQ".into(),upper,frozen_val:100.,frozen_vah:104.,frozen_at_ms:base,atr:4.,atr_at_ms:base,test_bar_ms:base,as_of_ms:base+420_000,complete_window:true,
   bars:(0..7).map(|n|Candle{timestamp_ms:base+n*60_000,open:price,high:if n==0{104.}else{price},low:if n==0{100.}else{price},close:price,volume:100}).collect(),
   trades:(0..=5).map(|n|Trade{symbol:"QQQ".into(),at:base+120_000+n*60_000,id:n.to_string(),price,volume:100.}).collect()}
 }
 #[test]fn symmetric_measured_rejection(){for upper in [true,false]{let r=read(&input(upper)).unwrap();assert_eq!(r.status,"observed");assert!((r.score.unwrap()-100.*0.75f64.cbrt()).abs()<1e-9);assert_eq!(r.permanence,Some(1.));assert_eq!(r.participation,Some(1.));assert_eq!(r.probability,None);assert!(!r.calibrated);}}
 #[test]fn future_does_not_help(){let mut i=input(true);i.as_of_ms-=1;assert_eq!(read(&i).unwrap().score,None);i.as_of_ms+=1;let baseline=read(&i).unwrap();i.trades.push(Trade{symbol:"QQQ".into(),at:i.as_of_ms+1,id:"future".into(),price:1.,volume:1e9});assert_eq!(read(&i).unwrap(),baseline);}
 #[test]fn missing_coverage_or_seed_is_pending(){let mut i=input(true);i.complete_window=false;assert_eq!(read(&i).unwrap().score,None);i.complete_window=true;i.trades.remove(0);assert_eq!(read(&i).unwrap().score,None);}
 #[test]fn invalidation_not_weak_rejection(){let mut i=input(true);for b in &mut i.bars[4..6]{b.close=105.;b.high=105.;}assert_eq!(read(&i).unwrap().status,"invalidated");assert_eq!(read(&i).unwrap().score,None);}
 #[test]fn duplicates_cannot_inflate_volume(){let mut i=input(true);let before=read(&i).unwrap();i.trades.push(i.trades[1].clone());assert_eq!(read(&i).unwrap(),before);let mut bad=i.trades[1].clone();bad.volume+=1.;i.trades.push(bad);assert!(read(&i).is_err());}
 #[test]fn freezes_evidence_and_requires_return(){let mut i=input(true);i.atr_at_ms+=1;assert!(read(&i).is_err());i.atr_at_ms-=1;i.bars[1].close=104.;i.bars[1].high=104.;assert_eq!(read(&i).unwrap().score,None);}
 #[test]fn scores_mixed_participation_without_candle_volume_substitution(){let mut i=input(true);i.atr=3.75;i.trades[1].price=105.;let r=read(&i).unwrap();assert!((r.distance.unwrap()-0.8).abs()<1e-10);assert!((r.permanence.unwrap()-0.8).abs()<1e-10);assert!((r.participation.unwrap()-0.8).abs()<1e-10);assert!((r.score.unwrap()-80.).abs()<1e-9);for b in &mut i.bars{b.volume=1_000_000;}assert_eq!(read(&i).unwrap(),r);}
 #[test]fn sparse_or_invalid_trades_do_not_get_a_score(){let mut i=input(true);i.trades.remove(2);assert_eq!(read(&i).unwrap().score,None);let mut i=input(true);i.trades[2].volume=f64::NAN;assert!(read(&i).is_err());}
 #[test]fn same_millisecond_prints_follow_numeric_provider_sequence(){let mut i=input(true);let mut a=i.trades[0].clone();a.id="9".into();a.price=105.;let mut b=a.clone();b.id="10".into();b.price=101.;i.trades.extend([a,b]);assert_eq!(read(&i).unwrap().permanence,Some(1.));i.trades.last_mut().unwrap().id="unknown".into();assert_eq!(read(&i).unwrap().score,None);}
}
