//! Share-volume quote test, not option Greek delta or exchange-certified aggressor.
use serde::Serialize;
use std::collections::{BTreeMap,VecDeque};
#[derive(Clone,Default,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Recent {pub buy:f64,pub sell:f64,pub unknown:f64,pub delta:f64,pub start_ms:u64,pub end_ms:u64,pub complete:bool}
#[derive(Clone,Default,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Period {
 pub buy:f64,pub sell:f64,pub unknown:f64,pub delta:f64,
 pub coverage_pct:Option<f64>,pub since_ms:Option<u64>,pub last_ms:Option<u64>,
 pub change_one:f64,pub change_five:f64,pub gaps:u64,
 pub history:Vec<HistoryPoint>,
 #[serde(skip)] minutes:BTreeMap<u64,f64>,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct HistoryPoint {pub at:u64,pub value:f64}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct BigTrade {pub at:u64,pub price:f64,pub shares:f64,pub notional:f64,pub side:i32,pub sequence:u64}
#[derive(Clone,Default,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Read {pub volume_policy:serde_json::Value,pub execution_evidence:crate::execution_evidence::Read,pub symbol:String,pub date:String,pub at:u64,pub available:bool,pub premarket:Period,pub regular:Period,pub rejected:u64,pub recent_five:Option<Recent>,pub big_trades:Vec<BigTrade>,pub big_trade_drops:u64}
#[derive(Clone,Default)]
pub struct Engine {evidence:crate::execution_evidence::Engine,symbol:String,date:String,quote:Option<(f64,f64,u64)>,periods:[Period;2],last_sequence:Option<u64>,last_trade:u64,rejected:u64,gaps:u64,recent:VecDeque<(u64,usize,f64)>,totals:[f64;3],recent_period:usize,dropped_at:Option<u64>,big_trades:VecDeque<BigTrade>,big_trade_drops:u64,shared_volume_policy:bool,policy_excluded:u64,policy_excluded_shares:f64,last_eligible_trade:u64}
fn positive(n:f64)->bool {n.is_finite()&&n>0.}
impl Engine {
 pub fn analysis()->Self{Self{shared_volume_policy:true,..Self::default()}}
 pub fn quote(&mut self,bid:f64,ask:f64,at:u64) {
  self.quote_with_depth(bid,ask,None,None,at);
 }
 pub fn quote_with_depth(&mut self,bid:f64,ask:f64,bs:Option<f64>,asz:Option<f64>,at:u64){
  self.evidence.quote(bid,ask,bs,asz,at);
  if self.quote.is_none_or(|q|at>=q.2){self.quote=Some((bid,ask,at));}
 }
 pub fn gap(&mut self){self.evidence.gap();self.quote=None;self.periods=Default::default();self.recent.clear();self.totals=[0.;3];self.dropped_at=None;self.last_eligible_trade=0;self.gaps+=1;}
 pub fn trade(&mut self,symbol:&str,price:f64,size:f64,at:u64,sequence:Option<u64>,conditions:&[u32],now:u64) {
  if !positive(price)||!positive(size)||at>now.saturating_add(1000)||now.saturating_sub(at)>15000 {self.rejected+=1;return;}
  let Some(date)=crate::calendar::date_at(at) else {return};
  if crate::calendar::date_at(now)!=Some(date){self.rejected+=1;return;}
  let crate::calendar::Day::Open(day)=crate::calendar::schedule(date) else {return};
  let date=date.to_string();
  let Some(start)=crate::calendar::legacy_eastern_ms(&format!("{date} 04:00:00")) else {return};
  if at<start||at>=day.close_ms {return;}
  if self.date!=date||self.symbol!=symbol {
   self.evidence.segment(at);
   self.big_trades.clear();self.big_trade_drops=0;
   self.periods=Default::default();self.recent.clear();self.totals=[0.;3];self.dropped_at=None;self.last_sequence=None;self.last_trade=0;self.last_eligible_trade=0;self.policy_excluded=0;self.policy_excluded_shares=0.;self.gaps=0;self.date=date;self.symbol=symbol.into();
  }
  let Some(sequence)=sequence else {self.rejected+=1;return};
  if self.last_sequence.is_some_and(|old|sequence<=old)||at<self.last_trade {self.rejected+=1;return;}
  self.last_sequence=Some(sequence);self.last_trade=at;
  if self.shared_volume_policy&&!crate::stock_volume_policy::permits_volume_at(conditions,at){self.policy_excluded+=1;self.policy_excluded_shares+=size;self.rejected+=1;return;}
  let index=usize::from(at>=day.open_ms);
  let eligible=conditions.iter().all(|c|matches!(c,0|12|37))&&(index==1||conditions.contains(&12));
  let side=if eligible {self.quote.filter(|(b,a,t)|positive(*b)&&positive(*a)&&a>b&&*t<=at&&at-*t<=2000&&(a-b)/a<=0.01).map_or(0,|(b,a,_)| {
   let epsilon=((a-b)*0.05).min(0.001);let mid=b+(a-b)/2.;
   if price>mid&&(price-a).abs()<=epsilon {1} else if price<mid&&(price-b).abs()<=epsilon {-1} else {0}
  })} else {0};
  if self.recent_period!=index {self.evidence.segment(at);}
  let p=&mut self.periods[index];
  if !(p.buy+p.sell+p.unknown+size).is_finite(){self.rejected+=1;return;}
  self.evidence.trade(at,sequence,price,size,side,conditions,eligible);
  let notional=price*size;
  if eligible&&notional.is_finite()&&notional>=100000. {
   if self.big_trades.len()>=1000 {self.big_trades.pop_front();self.big_trade_drops+=1;}
   self.big_trades.push_back(BigTrade{at,price,shares:size,notional,side,sequence});
  }
  p.since_ms.get_or_insert(at);p.last_ms=Some(at);p.gaps=self.gaps;
  match side {1=>p.buy+=size,-1=>p.sell+=size,_=>p.unknown+=size};
  p.delta=p.buy-p.sell;
  self.last_eligible_trade=at;
  *p.minutes.entry(at/60000).or_default()+=f64::from(side)*size;
  if self.recent_period!=index {self.recent.clear();self.totals=[0.;3];self.dropped_at=None;self.recent_period=index;}
  while self.recent.front().is_some_and(|v|v.0<at.saturating_sub(305000)) {let (_,k,v)=self.recent.pop_front().unwrap();self.totals[k]-=v;}
  if self.recent.len()>=200000 {let (t,k,v)=self.recent.pop_front().unwrap();self.totals[k]-=v;self.dropped_at=Some(t);}
  let k=if side==1{0}else if side==-1{1}else{2};self.recent.push_back((at,k,size));self.totals[k]+=size;
 }
 pub fn recent_at(&self,now:u64)->Option<Recent> {
  let start=now.saturating_sub(300000);let mut totals=self.totals;
  for &(_,k,v) in self.recent.iter().take_while(|v|v.0<start){totals[k]-=v;}
  for &(_,k,v) in self.recent.iter().rev().take_while(|v|v.0>now){totals[k]-=v;}
  for v in &mut totals{*v=v.max(0.);}
  Some(Recent{buy:totals[0],sell:totals[1],unknown:totals[2],delta:totals[0]-totals[1],start_ms:start,end_ms:now,complete:self.periods[self.recent_period].since_ms.is_some_and(|t|t<=start)&&self.dropped_at.is_none_or(|t|t<start)})

 }
 pub fn snapshot(&self,symbol:&str,now:u64,connected:bool)->Read {
  let mut r=Read{volume_policy:if self.shared_volume_policy{let mut m=crate::stock_volume_policy::metadata();m["excludedEvents"]=serde_json::json!(self.policy_excluded);m["excludedShares"]=serde_json::json!(self.policy_excluded_shares);m["classificationPolicy"]=serde_json::json!("conservative-nbbo-0-12-37-v1");m}else{serde_json::json!({"id":"legacy-cvd-volume-policy","scope":"legacy_strategy_inputs"})},symbol:symbol.into(),at:now,rejected:self.rejected,..Default::default()};
  if self.symbol!=symbol||crate::calendar::date_at(now).is_none_or(|d|d.to_string()!=self.date){return r;}
  r.execution_evidence=self.evidence.snapshot(now,connected);
  r.date=self.date.clone();r.premarket=self.periods[0].clone();r.regular=self.periods[1].clone();
  r.big_trades=self.big_trades.iter().filter(|p|p.at<=now.saturating_add(1000)).cloned().collect();r.big_trade_drops=self.big_trade_drops;
  let last=if self.shared_volume_policy{self.last_eligible_trade}else{self.last_trade};
  r.available=connected&&last>0&&last<=now.saturating_add(1000)&&now.saturating_sub(last)<=15000;
  r.recent_five=self.recent_at(now);
  for p in [&mut r.premarket,&mut r.regular] {
   // Recover the observed curve even when the chart was opened later.
   // No pre-connection prices/volumes are turned into synthetic delta.
   let mut cumulative=0.;
   p.history=p.minutes.iter().map(|(&minute,&delta)| {
    cumulative+=delta;
    HistoryPoint{at:(minute*60000+59999).min(p.last_ms.unwrap_or(now)),value:cumulative}
   }).collect();
   let total=p.buy+p.sell+p.unknown;p.coverage_pct=(total>0.).then(||(p.buy+p.sell)/total*100.);
   // Whole current minute / last five minute buckets; bounded by this observed segment.
   p.change_one=p.minutes.range(now/60000..=now/60000).map(|(_,v)|v).sum();
   p.change_five=p.minutes.range((now/60000).saturating_sub(4)..=now/60000).map(|(_,v)|v).sum();
  }r
 }
}
#[cfg(test)] mod tests {
 #[test]fn evidence_enriches_cvd_without_changing_delta(){let mut e=super::Engine::default();let t=at("10:00:00");e.quote_with_depth(99.99,100.,Some(300.),Some(500.),t);e.trade("QQQ",100.,2000.,t,Some(1),&[0],t);let r=e.snapshot("QQQ",t,true);assert_eq!(r.regular.delta,2000.);assert_eq!(r.big_trades.len(),1);assert_eq!(r.execution_evidence.windows[0].trades,1);assert_eq!(r.execution_evidence.recent_trades[0].quote.as_ref().unwrap().bid_shares,Some(300.));e.gap();assert!(e.snapshot("QQQ",t,true).execution_evidence.recent_trades.is_empty());}

 use super::*;
 fn at(t:&str)->u64 {crate::calendar::legacy_eastern_ms(&format!("2026-10-02 {t}")).unwrap()}
 #[test] fn bounded_provider_lead_keeps_original_order_and_rejects_large_lead(){
  let mut e=Engine::default();let now=at("10:00:00");e.quote(99.,100.,now+400);
  e.trade("QQQ",100.,10.,now+600,Some(1),&[0],now);
  let r=e.snapshot("QQQ",now,true);assert!(r.available);assert_eq!(r.regular.buy,10.);assert_eq!(r.regular.last_ms,Some(now+600));
  e.trade("QQQ",100.,20.,now+1001,Some(2),&[0],now);assert_eq!(e.snapshot("QQQ",now,true).rejected,1);
 }
 #[test] fn chart_history_recovers_minutes_without_inventing_pre_connection_data(){
  let mut e=Engine::default();let t=at("10:00:30");
  for (i,dt,price,size) in [(1,0,100.,10.),(2,60000,99.,30.),(3,120000,100.,5.)] {
   e.quote(99.,100.,t+dt);e.trade("QQQ",price,size,t+dt,Some(i),&[0],t+dt);
  }
  let r=e.snapshot("QQQ",t+120000,true);
  assert_eq!(r.regular.history.iter().map(|p|p.value).collect::<Vec<_>>(),vec![10.,-20.,-15.]);
  assert!(r.regular.history.iter().all(|p|p.at>=t&&p.at<=t+120000));
  e.gap();assert!(e.snapshot("QQQ",t+120000,true).regular.history.is_empty());
 }
 #[test] fn big_trades_are_individual_verified_prints_not_candle_volume(){
  let mut e=Engine::default();let t=at("10:00:00");e.quote(99.,100.,t);
  e.trade("QQQ",100.,2000.,t,Some(1),&[0],t);
  e.trade("QQQ",100.,2000.,t,Some(1),&[0],t);
  e.trade("QQQ",99.,2000.,t+1,Some(2),&[0],t+1);
  e.trade("QQQ",99.5,2000.,t+2,Some(3),&[0],t+2);
  e.trade("QQQ",100.,2000.,t+3,Some(4),&[2],t+3);
  e.trade("QQQ",100.,10.,t+4,Some(5),&[0],t+4);
  let r=e.snapshot("QQQ",t+4,true);assert_eq!(r.big_trades.len(),3);
  assert_eq!(r.big_trades.iter().map(|p|p.side).collect::<Vec<_>>(),vec![1,-1,0]);
  assert_eq!(r.big_trades[0].notional,200000.);
  e.gap();assert_eq!(e.snapshot("QQQ",t+4,true).big_trades.len(),3);
  assert!(e.snapshot("IWM",t+4,true).big_trades.is_empty());
 }
 #[test] fn signed_shares_unknowns_duplicates_and_periods() {
  let mut e=Engine::default();let t=at("09:29:00");e.quote(99.,100.,t);e.trade("NVDA",100.,10.,t,Some(1),&[12],t);
  let t=at("09:30:00");e.quote(99.,100.,t);e.trade("NVDA",99.,7.,t,Some(2),&[0],t);e.trade("NVDA",99.,7.,t,Some(2),&[0],t);
  e.trade("NVDA",99.5,3.,t,Some(3),&[0],t);
  let r=e.snapshot("NVDA",t,true);assert_eq!(r.premarket.delta,10.);assert_eq!(r.regular.delta,-7.);assert_eq!(r.regular.unknown,3.);assert_eq!(r.regular.coverage_pct,Some(70.));assert_eq!(r.rejected,1);
 }
 #[test] fn stale_future_and_locked_quotes_never_assign_a_side() {
  for (b,a,q) in [(99.,100.,at("09:30:00")),(99.,100.,at("09:30:04")),(100.,100.,at("09:30:03"))] {
   let mut e=Engine::default();let t=at("09:30:03");e.quote(b,a,q);e.trade("NVDA",100.,5.,t,Some(1),&[0],t);assert_eq!(e.snapshot("NVDA",t,true).regular.unknown,5.);
  }
 }
 #[test] fn reconnect_is_a_new_segment_and_reading_expires() {
  let mut e=Engine::default();let t=at("10:00:00");e.quote(99.,100.,t);e.trade("NVDA",100.,8.,t,Some(1),&[0],t);e.gap();
  e.trade("NVDA",100.,2.,t+1,Some(2),&[0],t+1);let r=e.snapshot("NVDA",t+1,true);assert_eq!(r.regular.delta,0.);assert_eq!(r.regular.unknown,2.);assert_eq!(r.regular.gaps,1);
  assert!(!e.snapshot("NVDA",t+16000,true).available);assert!(!e.snapshot("AAPL",t+1,true).available);
 }
 #[test] fn rolling_five_uses_exact_bounds_and_resets_at_open_or_gap() {
  let mut e=Engine::default();let t=at("09:25:00");
  for (i,dt,price) in [(1,0,100.),(2,1,99.),(3,300000,100.)] {e.quote(99.,100.,t+dt);e.trade("NVDA",price,10.,t+dt,Some(i),&[12],t+dt);}
  // Opening resets the segment; premarket never supplies RTH coverage.
  assert!(!e.snapshot("NVDA",t+300000,true).recent_five.unwrap().complete);
  let t=at("10:00:00");let mut e=Engine::default();
  for (i,dt,price) in [(1,0,100.),(2,1,99.),(3,300000,100.)] {e.quote(99.,100.,t+dt);e.trade("NVDA",price,10.,t+dt,Some(i),&[0],t+dt);}
  let a=e.snapshot("NVDA",t+300000,true).recent_five.unwrap();assert!(a.complete);assert_eq!(a.delta,10.);
  let b=e.snapshot("NVDA",t+300001,true).recent_five.unwrap();assert_eq!(b.delta,0.);
  let earlier=e.recent_at(t+299999).unwrap();assert_eq!(earlier.delta,0.);assert!(!earlier.complete);
  e.gap();assert!(!e.snapshot("NVDA",t+300001,true).recent_five.unwrap().complete);
 }
 #[test] fn bounded_retention_cannot_claim_complete_after_capacity_drop() {
  let mut e=Engine::default();let t=at("10:00:00");e.quote(99.,100.,t);
  for i in 0..200001 {e.trade("NVDA",100.,1.,t,Some(i+1),&[0],t);}
  assert_eq!(e.recent.len(),200000);assert_eq!(e.dropped_at,Some(t));
  assert!(!e.recent_at(t+300000).unwrap().complete);
  assert!(e.recent_at(t+300001).unwrap().complete);
 }
}
#[cfg(test)]mod shared_policy_tests{
 use super::*;
 fn at(s:&str)->u64{crate::calendar::legacy_eastern_ms(s).unwrap()}
 #[test]fn admitting_volume_does_not_fabricate_direction(){let t=at("2026-10-12 10:00:00");let mut e=Engine::analysis();e.quote(99.99,100.,t);e.trade("QQQ",100.,100.25,t,Some(1),&[14,41],t);let r=e.snapshot("QQQ",t,true);assert_eq!(r.regular.buy,0.);assert_eq!(r.regular.unknown,100.25);assert_eq!(r.regular.delta,0.);assert_eq!(r.volume_policy["id"],crate::stock_volume_policy::ID);}
 #[test]fn volume_veto_and_missing_rule_are_excluded_without_changing_strategy_default(){let t=at("2026-10-12 10:00:00");let mut analysis=Engine::analysis();let mut strategy=Engine::default();for e in [&mut analysis,&mut strategy]{e.trade("QQQ",100.,20.,t,Some(1),&[15],t);e.trade("QQQ",100.,30.,t+1,Some(2),&[999],t+1);}let a=analysis.snapshot("QQQ",t+1,true);assert_eq!(a.regular.unknown,0.);assert!(!a.available);assert_eq!(a.volume_policy["excludedEvents"],2);assert_eq!(strategy.snapshot("QQQ",t+1,true).regular.unknown,50.);}
 #[test]fn reset_recovery_preserves_policy_and_does_not_show_old_coverage(){let t=at("2026-10-12 10:00:00");let mut e=Engine::analysis();e.trade("QQQ",100.,10.,t,Some(1),&[],t);e.gap();assert!(!e.snapshot("QQQ",t+1,true).available);e.trade("QQQ",100.,4.5,t+2,Some(2),&[],t+2);assert_eq!(e.snapshot("QQQ",t+2,true).regular.unknown,4.5);assert_eq!(e.snapshot("QQQ",t+2,true).volume_policy["id"],crate::stock_volume_policy::ID);}
 #[test]fn premarket_and_rth_are_separate_and_close_is_not_another_trade(){let p=at("2026-10-12 08:00:00");let r=at("2026-10-12 09:30:00");let c=at("2026-10-12 16:00:00");let mut e=Engine::analysis();e.trade("QQQ",100.,2.,p,Some(1),&[],p);e.trade("QQQ",100.,3.,r,Some(2),&[14,41],r);e.trade("QQQ",100.,999.,c,Some(3),&[],c);let s=e.snapshot("QQQ",c,true);assert_eq!(s.premarket.unknown,2.);assert_eq!(s.regular.unknown,3.);assert!(!s.available);}
 #[test]fn unknown_side_conserves_decimal_quantity_and_duplicate_cannot_add_it_twice(){let t=at("2026-10-12 10:00:00");let mut e=Engine::analysis();for q in [1,1,2]{e.trade("QQQ",100.,0.002,t,Some(q),&[37],t);}let s=e.snapshot("QQQ",t,true);assert_eq!(s.regular.unknown,0.004);assert_eq!(s.regular.delta,0.);}
}
