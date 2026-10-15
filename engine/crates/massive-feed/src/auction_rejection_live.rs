//! Causal adapter for frozen rejection events; only real eligible ticks, never bin aggregates.
use snipers_core::{auction_map::{Reading as Profile,Trade},auction_rejection::{self,Input,Reading},Candle};
use std::collections::{BTreeMap,VecDeque};
#[derive(Clone,Default)]
pub struct Tracker{events:BTreeMap<u64,(f64,f64,f64,u64)>,completed:BTreeMap<(u64,bool),Reading>}
impl Tracker{
 pub fn snapshot(&mut self,p:&Profile,bars:&[Candle],ticks:&VecDeque<Trade>,since:Option<u64>,discarded:u64,now:u64)->(Option<Reading>,Option<Reading>){
  self.events.retain(|t,_|t.saturating_add(720_000)>=now);
  self.completed.retain(|(t,_),_|t.saturating_add(720_000)>=now);
  let mut output=[None,None];
  for (side,upper) in [true,false].into_iter().enumerate(){
   for (&test,&(val,vah,atr,frozen)) in self.events.iter().rev(){
    let Some(bar)=bars.iter().find(|b|b.timestamp_ms==test&&b.timestamp_ms+60000<=now)else{continue};
    if if upper{bar.high<vah}else{bar.low>val}{continue;}
    let start=test+120000;let end=start+300000;
    if end+300000<now{continue;}
    let covered=now>=end+15000&&since.is_some_and(|s|s<=start.saturating_sub(60000))&&discarded<start;
    let mut reading=if let Some(cached)=self.completed.get(&(test,upper)){cached.clone()}else{
    let input=Input{symbol:p.symbol.clone(),upper,frozen_val:val,frozen_vah:vah,frozen_at_ms:frozen,atr,atr_at_ms:frozen,test_bar_ms:test,as_of_ms:now,
     complete_window:covered,trades:if covered{ticks.iter().filter(|t|t.at>=start.saturating_sub(60000)&&t.at<=end).cloned().collect()}else{vec![]},bars:bars.iter().filter(|b|b.timestamp_ms>=test&&b.timestamp_ms<end).cloned().collect()};
    let Ok(reading)=auction_rejection::read(&input)else{continue};
    if reading.status!="pending"{self.completed.insert((test,upper),reading.clone());}reading};
    // Completed evidence expires, and a later failed retest invalidates its score.
    if reading.score.is_some(){let recent:Vec<_>=bars.iter().filter(|b|b.timestamp_ms>=end&&b.timestamp_ms+60000<=now).collect();if recent.windows(2).any(|w|w[1].timestamp_ms==w[0].timestamp_ms+60000&&w.iter().all(|b|if upper{b.close>=vah}else{b.close<=val})){reading.status="invalidated".into();reading.score=None;reading.reason="Later two-close invalidation".into();}}
    if reading.status!="pending"{output[side]=Some(reading);break;}
    if output[side].is_none(){output[side]=Some(reading);}
   }
  }
  // Freeze before the next test minute, using only completed contiguous ATR bars.
  let test=(now/60000+1)*60000;
  if !self.events.contains_key(&test){
   let mut closed:Vec<_>=bars.iter().filter(|b|b.timestamp_ms+60000<=now).collect();closed.sort_by_key(|b|b.timestamp_ms);
   if closed.len()>=15{let tail=&closed[closed.len()-15..];if tail.windows(2).all(|w|w[1].timestamp_ms==w[0].timestamp_ms+60000)&&tail.last().is_some_and(|b|now-b.timestamp_ms-60000<60000){let tuples:Vec<_>=tail.iter().map(|b|(b.high,b.low,b.close)).collect();if let Some(atr)=snipers_core::atr::sma14(&tuples){self.events.insert(test,(p.val,p.vah,atr,now));}}}
  }
  (output[0].take(),output[1].take())
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn freeze_is_causal_and_warmup_is_pending(){let mut t=Tracker::default();let p=Profile{version:"x".into(),symbol:"QQQ".into(),as_of_ms:900000,provisional:true,accepted_trades:1,duplicate_trades:0,excluded_trades:0,total_volume:1.,vwap:102.,poc_low:101.,poc_high:102.,val:100.,vah:104.,actual_value_fraction:0.7,bins:vec![],reaction:"pending".into(),routes:vec![],calibrated:false};assert_eq!(t.snapshot(&p,&[],&VecDeque::new(),Some(1),0,900000),(None,None));assert!(t.events.is_empty());let bars:Vec<_>=(0..15).map(|n|Candle{timestamp_ms:n*60000,open:102.,high:104.,low:100.,close:102.,volume:1}).collect();t.snapshot(&p,&bars,&VecDeque::new(),Some(1),0,900000);assert_eq!(t.events[&960000],(100.,104.,4.,900000));let mut changed=p;changed.vah=110.;t.snapshot(&changed,&bars,&VecDeque::new(),Some(1),0,900001);assert_eq!(t.events[&960000].1,104.);}
 #[test]fn full_event_runs_new_engine_and_loss_blocks_percentage(){let mut t=Tracker::default();t.events.insert(600000,(100.,104.,4.,599000));let p=Profile{version:"x".into(),symbol:"QQQ".into(),as_of_ms:1020000,provisional:true,accepted_trades:1,duplicate_trades:0,excluded_trades:0,total_volume:1.,vwap:101.,poc_low:101.,poc_high:102.,val:100.,vah:104.,actual_value_fraction:0.7,bins:vec![],reaction:"pending".into(),routes:vec![],calibrated:false};let bars:Vec<_>=(0..7).map(|n|Candle{timestamp_ms:600000+n*60000,open:101.,high:if n==0{104.}else{101.},low:101.,close:101.,volume:100}).collect();let ticks=(0..=5).map(|n|Trade{symbol:"QQQ".into(),at:720000+n*60000,id:n.to_string(),price:101.,volume:100.}).collect();let (r,_)=t.snapshot(&p,&bars,&ticks,Some(599000),0,1035000);assert!((r.unwrap().score.unwrap()-100.*0.75f64.cbrt()).abs()<1e-9);t.completed.clear();let (r,_)=t.snapshot(&p,&bars,&ticks,Some(599000),800000,1035000);assert_eq!(r.unwrap().score,None);}
}
