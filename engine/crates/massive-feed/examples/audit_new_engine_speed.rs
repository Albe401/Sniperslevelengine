//! Offline performance audit. Synthetic eligible tapes only; no network/credentials/orders.
use massive_feed::{auction_live::Engine,auction_rejection_live::Tracker};
use snipers_core::{auction_map::{Trade,Reading as Profile},auction_rejection::{self,Input},Candle};
use std::{collections::VecDeque,time::Instant};
fn stats(mut x:Vec<f64>)->serde_json::Value{x.sort_by(|a,b|a.total_cmp(b));serde_json::json!({"samples":x.len(),"medianMs":x[x.len()/2],"p95Ms":x[((x.len() as f64*0.95).ceil() as usize-1).min(x.len()-1)],"maxMs":x[x.len()-1]})}
fn main(){
 let base=snipers_core::calendar::legacy_eastern_ms("2026-10-09 10:00:00").unwrap();
 let bars:Vec<_>=(0..7).map(|n|Candle{timestamp_ms:base+n*60000,open:101.,high:if n==0{104.}else{101.},low:101.,close:101.,volume:100}).collect();
 let trades:Vec<_>=(0..100000).map(|n|Trade{symbol:"QQQ".into(),at:base+120000+n*3,id:n.to_string(),price:101.,volume:1.}).collect();
 let input=Input{symbol:"QQQ".into(),upper:true,frozen_val:100.,frozen_vah:104.,frozen_at_ms:base-60000,atr:4.,atr_at_ms:base-60000,test_bar_ms:base,as_of_ms:base+420000,complete_window:true,trades:trades.clone(),bars:bars.clone()};
 let mut raw=[] .to_vec();for _ in 0..7{let t=Instant::now();let r=auction_rejection::read(&input).unwrap();assert!(r.score.unwrap()>90.);raw.push(t.elapsed().as_secs_f64()*1000.);}
 let mut engine=Engine::default();let t=Instant::now();for n in 0..100000u64{let at=base+n*4;engine.trade("QQQ",101.+(n%100) as f64*0.01,1.,at,Some(n+1),&[0],at);}let ingestion=t.elapsed().as_secs_f64()*1000.;
 let now=base+399996;let past:Vec<_>=(0..25).map(|n|Candle{timestamp_ms:base-15*60000+n*60000,open:101.,high:104.,low:100.,close:101.,volume:100}).collect();
 let mut profile=[] .to_vec();for _ in 0..500{let t=Instant::now();assert!(engine.snapshot("QQQ",now,true,past.clone()).is_some());profile.push(t.elapsed().as_secs_f64()*1000.);}
 let p=Profile{version:"x".into(),symbol:"QQQ".into(),as_of_ms:base-60000,provisional:true,accepted_trades:1,duplicate_trades:0,excluded_trades:0,total_volume:1.,vwap:101.,poc_low:101.,poc_high:102.,val:100.,vah:104.,actual_value_fraction:0.7,bins:vec![],reaction:"pending".into(),routes:vec![],calibrated:false};
 let prior:Vec<_>=(0..15).map(|n|Candle{timestamp_ms:base-16*60000+n*60000,open:101.,high:104.,low:100.,close:101.,volume:100}).collect();
 let ticks:VecDeque<_>=trades.into();let mut tracker=Tracker::default();tracker.snapshot(&p,&prior,&ticks,Some(base-60000),0,base-60000);
 let t=Instant::now();let (r,_)=tracker.snapshot(&p,&bars,&ticks,Some(base-60000),0,base+435000);assert!(r.unwrap().score.unwrap()>90.);let first=t.elapsed().as_secs_f64()*1000.;
 let mut cached=[] .to_vec();for _ in 0..500{let t=Instant::now();let (r,_)=tracker.snapshot(&p,&bars,&ticks,Some(base-60000),0,base+435000);assert!(r.unwrap().score.is_some());cached.push(t.elapsed().as_secs_f64()*1000.);}
 println!("{}",serde_json::json!({"mode":"release offline synthetic benchmark","ticks":100000,"rawRejection":stats(raw),"ingest100kMs":ingestion,"profileSnapshot":stats(profile),"firstRejectionMs":first,"cachedRejection":stats(cached)}));
}
