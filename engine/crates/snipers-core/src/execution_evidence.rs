//! Observed stock executions and NBBO plus provisional absorption estimates. No orders.
//! In-memory rolling collection, reset on disconnect/date/session change.
//! Window completeness means an observed span without buffer drops, not full-market coverage.
//! Classification is inherited from the existing CVD quote test, not certified aggressor data.
//! Quote counts are observed updates, not identified replenishments of a single order.
use serde::Serialize;
use std::collections::{BTreeMap,VecDeque};
const CAP:usize=10_000;
const KEEP_MS:u64=65_000;
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Quote {pub at:u64,pub bid:f64,pub ask:f64,pub bid_shares:Option<f64>,pub ask_shares:Option<f64>,pub valid:bool}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Trade {pub at:u64,pub sequence:u64,pub price:f64,pub shares:f64,pub side:i32,pub conditions:Vec<u32>,pub eligible:bool,pub quote:Option<Quote>,pub quote_age_ms:Option<u64>,pub depth_available:bool}
#[derive(Clone,Default,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Zone {pub lower:f64,pub upper:f64,pub first_ms:u64,pub last_ms:u64,pub first_price:f64,pub last_price:f64,pub low:f64,pub high:f64,pub trades:u64,pub shares:f64,pub buy:f64,pub sell:f64,pub unknown:f64,pub eligible_shares:f64,pub quoted_trades:u64,pub depth_trades:u64,pub big_trades:u64}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Window {pub seconds:u64,pub start_ms:u64,pub end_ms:u64,pub complete:bool,pub trades:u64,pub quote_updates:u64,pub classified_pct:Option<f64>,pub excluded_zones:usize,pub zones:Vec<Zone>}
#[derive(Clone,Default,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Read {pub absorption:crate::execution_absorption::Read,pub version:String,pub status:String,pub source:String,pub grouping:String,pub observed_since_ms:Option<u64>,pub trade_drops:u64,pub quote_drops:u64,pub rejected_quotes:u64,pub windows:Vec<Window>,pub recent_trades:Vec<Trade>,pub recent_quotes:Vec<Quote>}
#[derive(Clone,Default)]
pub struct Engine {since:Option<u64>,quotes:VecDeque<Quote>,trades:VecDeque<Trade>,latest:Option<Quote>,trade_drops:u64,quote_drops:u64,dropped_trade_at:Option<u64>,dropped_quote_at:Option<u64>,rejected_quotes:u64}
fn pos(v:f64)->bool {v.is_finite()&&v>0.}
impl Engine {
 pub fn gap(&mut self){*self=Self::default();}
 pub fn segment(&mut self,at:u64){let q=self.latest.clone().filter(|q|q.at<=at&&at-q.at<=2000);self.gap();self.since=Some(at);if let Some(q)=q{self.quote(q.bid,q.ask,q.bid_shares,q.ask_shares,q.at);}}
 pub fn quote(&mut self,bid:f64,ask:f64,bs:Option<f64>,asz:Option<f64>,at:u64){
  if self.latest.as_ref().is_some_and(|q|at<q.at){self.rejected_quotes+=1;return;}
  if !bid.is_finite()||!ask.is_finite(){self.rejected_quotes+=1;self.quote(0.,0.,None,None,at);return;}
  let q=Quote{at,bid,ask,bid_shares:bs.filter(|v|pos(*v)),ask_shares:asz.filter(|v|pos(*v)),valid:pos(bid)&&pos(ask)&&ask>bid};
  self.since.get_or_insert(at);while self.quotes.front().is_some_and(|q|q.at<at.saturating_sub(KEEP_MS)){self.quotes.pop_front();}
  if self.quotes.len()>=CAP{if let Some(old)=self.quotes.pop_front(){self.dropped_quote_at=Some(old.at);self.quote_drops+=1;}}
  self.latest=Some(q.clone());self.quotes.push_back(q);
 }
 pub fn trade(&mut self,at:u64,sequence:u64,price:f64,shares:f64,side:i32,conditions:&[u32],eligible:bool){
  if !pos(price)||!pos(shares)||!pos(price*shares)||price*100.>=u64::MAX as f64{return;}
  if self.trades.back().is_some_and(|t|sequence<=t.sequence||at<t.at){return;}
  let quote=self.latest.clone().filter(|q|q.valid&&q.at<=at&&at-q.at<=2000);
  let depth_available=quote.as_ref().is_some_and(|q|q.bid_shares.is_some()&&q.ask_shares.is_some());
  let t=Trade{at,sequence,price,shares,side,conditions:conditions.to_vec(),eligible,quote_age_ms:quote.as_ref().map(|q|at-q.at),quote,depth_available};
  self.since.get_or_insert(at);while self.trades.front().is_some_and(|t|t.at<at.saturating_sub(KEEP_MS)){self.trades.pop_front();}
  if self.trades.len()>=CAP{if let Some(old)=self.trades.pop_front(){self.dropped_trade_at=Some(old.at);self.trade_drops+=1;}}
  self.trades.push_back(t);
 }
 pub fn snapshot(&self,now:u64,connected:bool)->Read{
  let mut out=Read{version:"execution-evidence-v1".into(),status:if connected{"observing"}else{"disconnected"}.into(),source:"Massive Stocks T/Q · NBBO; no order IDs or hidden reserve".into(),grouping:"USD 0.01 price buckets, provisional grouping; not exchange tick size or structural levels".into(),observed_since_ms:self.since,trade_drops:self.trade_drops,quote_drops:self.quote_drops,rejected_quotes:self.rejected_quotes,..Default::default()};
  if self.trades.back().is_none_or(|t|t.at>now||now-t.at>15000){out.status=if connected{"waiting-for-trades"}else{"disconnected"}.into();}
  for seconds in [10,30,60]{let start=now.saturating_sub(seconds*1000);let mut zones:BTreeMap<u64,Zone>=BTreeMap::new();let mut count=0;
   for t in self.trades.iter().filter(|t|t.at>=start&&t.at<=now){let key=(t.price*100.+1e-8).floor() as u64;let z=zones.entry(key).or_insert_with(||Zone{lower:key as f64/100.,upper:(key+1) as f64/100.,first_ms:t.at,last_ms:t.at,first_price:t.price,last_price:t.price,low:t.price,high:t.price,..Default::default()});count+=1;z.trades+=1;z.last_ms=t.at;z.last_price=t.price;z.low=z.low.min(t.price);z.high=z.high.max(t.price);z.shares+=t.shares;match t.side{1=>z.buy+=t.shares,-1=>z.sell+=t.shares,_=>z.unknown+=t.shares};if t.eligible{z.eligible_shares+=t.shares;}if t.quote.is_some(){z.quoted_trades+=1;}if t.depth_available{z.depth_trades+=1;}if t.eligible&&t.price*t.shares>=100000.{z.big_trades+=1;}}
   let total:f64=zones.values().map(|z|z.shares).sum();let classified:f64=zones.values().map(|z|z.buy+z.sell).sum();let mut zs:Vec<_>=zones.into_values().collect();zs.sort_by(|a,b|b.shares.total_cmp(&a.shares).then(a.lower.total_cmp(&b.lower)));let excluded=zs.len().saturating_sub(100);zs.truncate(100);
   out.windows.push(Window{seconds,start_ms:start,end_ms:now,complete:connected&&self.since.is_some_and(|t|t<=start)&&self.dropped_trade_at.is_none_or(|t|t<start)&&self.dropped_quote_at.is_none_or(|t|t<start),trades:count,quote_updates:self.quotes.iter().filter(|q|q.at>=start&&q.at<=now).count() as u64,classified_pct:(total>0.&&total.is_finite()&&classified.is_finite()).then(||classified/total*100.),excluded_zones:excluded,zones:zs});
  }
  out.absorption=crate::execution_absorption::evaluate(&self.trades,&self.quotes,now,out.windows[1].complete);
  out.recent_trades=self.trades.iter().rev().filter(|t|t.at<=now&&t.at>=now.saturating_sub(60_000)).take(20).cloned().collect();out.recent_quotes=self.quotes.iter().rev().filter(|q|q.at<=now&&q.at>=now.saturating_sub(60_000)).take(20).cloned().collect();out
 }
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn windows_preserve_actual_price_size_quote_and_unknowns(){let mut e=Engine::default();e.quote(100.,100.01,Some(500.),Some(700.),1000);e.trade(1000,1,100.01,2000.,1,&[0],true);e.trade(1001,2,100.015,30.,0,&[0],true);let r=e.snapshot(11000,true);let w=&r.windows[0];assert!(w.complete);assert_eq!(w.trades,2);assert_eq!(w.zones.len(),1);let z=&w.zones[0];assert_eq!(z.buy,2000.);assert_eq!(z.unknown,30.);assert_eq!(z.big_trades,1);assert_eq!(z.high,100.015);assert_eq!(r.recent_trades[1].quote.as_ref().unwrap().ask_shares,Some(700.));assert_eq!(r.recent_trades[1].conditions,vec![0]);assert!(!r.windows[1].complete);}
 #[test]fn missing_stale_future_and_crossed_quotes_do_not_supply_depth(){for (b,a,q) in [(100.,100.01,1),(100.,100.01,10001),(100.01,100.,10000)]{let mut e=Engine::default();e.quote(b,a,Some(10.),Some(20.),q);e.trade(10000,1,100.01,10.,0,&[0],true);let r=e.snapshot(10000,true);assert!(r.recent_trades[0].quote.is_none());assert!(!r.recent_trades[0].depth_available);}let mut e=Engine::default();e.quote(100.,100.01,None,Some(20.),10000);e.trade(10000,1,100.,10.,-1,&[0],true);assert!(!e.snapshot(10000,true).recent_trades[0].depth_available);}
 #[test]fn duplicate_trade_and_out_of_order_quote_do_not_inflate_evidence(){let mut e=Engine::default();e.quote(100.,100.01,Some(1.),Some(2.),1000);e.quote(99.,100.,Some(1.),Some(2.),999);for _ in 0..2{e.trade(1000,1,100.,10.,-1,&[0],true);}let r=e.snapshot(1000,true);assert_eq!(r.windows[0].trades,1);assert_eq!(r.windows[0].quote_updates,1);assert_eq!(r.rejected_quotes,1);}
 #[test]fn gap_and_segment_clear_evidence_and_restart_window_coverage(){let mut e=Engine::default();e.quote(100.,100.01,Some(1.),Some(2.),1000);e.trade(1000,1,100.,10.,-1,&[0],true);assert!(e.snapshot(11000,true).windows[0].complete);e.gap();assert!(e.snapshot(11000,true).recent_trades.is_empty());e.segment(11000);assert!(!e.snapshot(11000,true).windows[0].complete);assert!(!e.snapshot(22000,false).windows[0].complete);}
 #[test]fn windows_exclude_future_and_expired_events(){let mut e=Engine::default();e.trade(10000,1,100.,10.,0,&[],true);e.trade(11000,2,100.,20.,0,&[],true);let r=e.snapshot(10000,true);assert_eq!(r.windows[0].trades,1);assert_eq!(r.recent_trades.len(),1);assert_eq!(e.snapshot(80000,true).windows[2].trades,0);}
 #[test]fn capacity_is_bounded_and_drops_invalidate_observed_completeness(){let mut e=Engine::default();for i in 0..CAP+2{let t=i as u64;e.quote(100.,100.01,Some(1.),Some(2.),t);e.trade(t,t+1,100.,1.,0,&[],true);}assert_eq!(e.trades.len(),CAP);assert_eq!(e.quotes.len(),CAP);let r=e.snapshot(10001,true);assert_eq!(r.trade_drops,2);assert_eq!(r.quote_drops,2);assert!(!r.windows[1].complete);assert_eq!(r.recent_trades.len(),20);assert_eq!(r.recent_quotes.len(),20);}
 #[test]fn serialization_is_evidence_only_without_trading_decisions(){let mut e=Engine::default();e.trade(1000,1,100.,20.,0,&[2],false);let v=serde_json::to_value(e.snapshot(1000,true)).unwrap();assert!(v.get("windows").is_some());assert!(v.get("signal").is_none());assert_eq!(v["windows"][0]["zones"][0]["eligibleShares"],0.);assert!(v["windows"][0]["zones"][0]["unknown"].as_f64().unwrap()>0.);}
}

#[cfg(test)]mod absorption_integration {
 use super::*;
 #[test]fn live_snapshot_exposes_estimates_and_gap_clears_them(){
 let mut e=Engine::default();for i in 0..31 {let at=1000+i*1000;e.quote(100.,100.02,Some(100.),Some(100.),at);e.trade(at,i+1,100.02,if i>=21{1000.}else{100.},1,&[0],true);}
 let r=e.snapshot(31000,true);assert_eq!(r.absorption.status,"possible-absorption");assert_eq!(r.absorption.candidates[0].absorbing_side,"sell");assert!(e.snapshot(31000,false).absorption.candidates.is_empty());e.gap();assert_eq!(e.snapshot(31000,true).absorption.status,"pending");
 }
 #[test]fn nonfinite_new_quote_revokes_a_previous_hold(){let mut e=Engine::default();for i in 0..31{let at=1000+i*1000;e.quote(100.,100.02,Some(100.),Some(100.),at);e.trade(at,i+1,100.02,if i>=21{1000.}else{100.},1,&[0],true);}assert_eq!(e.snapshot(31000,true).absorption.candidates.len(),1);e.quote(f64::NAN,100.02,Some(100.),Some(100.),31000);let r=e.snapshot(31000,true);assert!(r.absorption.candidates.is_empty());assert_eq!(r.rejected_quotes,1);}

}
