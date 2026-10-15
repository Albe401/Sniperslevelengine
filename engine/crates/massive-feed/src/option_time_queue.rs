//! Hold small provider clock leads without accepting future trading evidence.
use crate::Event;
use serde::Serialize;
use std::collections::BTreeMap;
const CAPACITY:usize=16384;
#[derive(Clone,Default,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Stats {pub deferred:u64,pub released:u64,pub large_leads:u64,pub dropped:u64,pub pending:usize,pub maximum_lead_ms:u64,pub by_symbol:BTreeMap<String,u64>}
#[derive(Default)]
pub struct Queue {rows:BTreeMap<(u64,u64),Event>,sequence:u64,pub stats:Stats}
impl Queue {
 pub fn ingest(&mut self,event:Event,now:u64)->Option<Event>{
  let (t,symbol)=match &event{Event::Trade{t,sym,..}|Event::Quote{t,sym,..}=>(*t,sym),_=>return Some(event)};
  let lead=t.saturating_sub(now);self.stats.maximum_lead_ms=self.stats.maximum_lead_ms.max(lead);
  if lead==0{return Some(event)}
  if lead>1000{self.stats.large_leads+=1;return Some(event)}
  if self.rows.len()>=CAPACITY {self.stats.dropped+=self.rows.len() as u64+1;self.rows.clear();self.stats.pending=0;return None}
  let root=crate::live::option_root_for_diagnostic(symbol);
  let root=if self.stats.by_symbol.contains_key(&root)||self.stats.by_symbol.len()<32{root}else{"OTHER".into()};
  *self.stats.by_symbol.entry(root).or_default()+=1;
  self.sequence=self.sequence.wrapping_add(1);self.rows.insert((t,self.sequence),event);self.stats.deferred+=1;self.stats.pending=self.rows.len();None
 }
 pub fn ready(&mut self,now:u64)->Vec<Event>{
  let mut out=Vec::new();
  for _ in 0..2048 {let Some((&key,_))=self.rows.first_key_value()else{break};if key.0>now{break}let event=self.rows.remove(&key).unwrap();if now-key.0>15000{self.stats.dropped+=1}else{self.stats.released+=1;out.push(event)}}
  self.stats.pending=self.rows.len();out
 }
 pub fn retain(&mut self,allowed:impl Fn(&Event)->bool){let before=self.rows.len();self.rows.retain(|_,e|allowed(e));self.stats.dropped+=(before-self.rows.len()) as u64;self.stats.pending=self.rows.len();}
}
#[cfg(test)]mod tests{
 use super::*;
 fn quote(t:u64)->Event{Event::Quote{sym:"O:QQQ261009C00750000".into(),bp:1.,ap:1.1,bs:1.,ask_size:1.,t}}
 #[test]fn holds_until_original_time_and_preserves_order(){let mut q=Queue::default();assert!(q.ingest(quote(1650),1000).is_none());assert!(q.ingest(quote(1400),1000).is_none());assert!(q.ready(1399).is_empty());assert!(matches!(q.ready(1400)[0],Event::Quote{t:1400,..}));assert!(q.ready(1649).is_empty());assert_eq!(q.ready(1650).len(),1);assert!(q.ready(2000).is_empty());}
 #[test]fn large_skew_is_not_hidden_and_capacity_is_bounded(){let mut q=Queue::default();assert!(q.ingest(quote(2001),1000).is_some());for _ in 0..=CAPACITY{q.ingest(quote(1650),1000);}assert_eq!(q.stats.pending,0);assert_eq!(q.stats.dropped,CAPACITY as u64+1);assert_eq!(q.stats.large_leads,1);}
 #[test]fn rotation_and_expiry_discard_old_evidence(){let mut q=Queue::default();q.ingest(quote(1650),1000);q.retain(|_|false);assert!(q.ready(2000).is_empty());q.ingest(quote(2650),2000);assert!(q.ready(20000).is_empty());assert_eq!(q.stats.dropped,2);}
 #[test]fn event_bursts_are_bounded_and_fast(){let start=std::time::Instant::now();let mut q=Queue::default();for batch in 0..25 {let now=1000+batch*1000;for i in 0..2000{q.ingest(quote(now+1+i%900),now);}while !q.rows.is_empty(){q.ready(now+1000);}}let ms=start.elapsed().as_secs_f64()*1000.;println!("50000 buffered quotes: {ms:.2} ms");assert_eq!(q.stats.released,50000);assert_eq!(q.stats.dropped,0);assert!(ms<2000.);}
}
