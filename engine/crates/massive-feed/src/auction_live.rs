//! Fixed-bin streaming compression: preserves observed bin volume and bracket presence.
//! Never certifies a complete session; no historical candle volume is used as ticks.
use snipers_core::{auction_map::{self, Input, Reading, Trade}, calendar::{self, Day}, Candle};
use std::collections::{BTreeMap,VecDeque};
use std::sync::{Arc,Mutex};
#[derive(Clone, Default, serde::Serialize)]
#[serde(rename_all="camelCase")]
struct Exclusion { events:u64, known_shares:f64 }
#[derive(Clone, Default, serde::Serialize)]
#[serde(rename_all="camelCase")]
struct Quality {
    excluded:BTreeMap<String,Exclusion>,
    excluded_conditions:BTreeMap<u32,u64>,
    capacity_degraded:bool,
}
impl Quality {
    fn exclude(&mut self,reason:&str,size:f64){
        let row=self.excluded.entry(reason.into()).or_default();row.events+=1;
        if size.is_finite()&&size>0. {row.known_shares+=size;}
    }
}
#[derive(Clone)]
struct ClassifiedTrade{trade:Trade,separation:Option<&'static str>}
#[derive(Clone, Default)]
pub struct Engine {
    quality:Quality,
    partitions:crate::auction_profile_partitions::Partitions,
    reconciliation:Option<crate::auction_reconciliation::Report>,
    reconciliation_error:Option<String>,
    reconciliation_attempt_ms:u64,
    epoch:u64,
    backfill:Option<(u64,Arc<Mutex<(Vec<ClassifiedTrade>,bool)>>)>,
    history_status:String,
    history_through:Option<u64>,
    history_pages:usize,
    history_rows:usize,
    symbol: String,
    open: u64,
    close: u64,
    sequence: Option<u64>,
    pending: BTreeMap<(u64,u64),ClassifiedTrade>,
    deferred: u64,
    released: u64,
    maximum_lead_ms: u64,
    pending_drops: u64,
    bins: BTreeMap<(i64,u64),f64>,
    count: usize,
    weighted: f64,
    observed_since: Option<u64>,
    last_trade: u64,
    raw_ticks:VecDeque<Trade>,
    discarded_through:u64,
    rejection:crate::auction_rejection_live::Tracker,
}
impl Engine {
    /// Compare a settled 30-second sample at most once per minute. Never imply continuity.
    pub fn begin_reconciliation(&mut self,now:u64)->Option<crate::auction_reconciliation::Request> {
        let through=self.history_through?;
        if self.backfill.is_some()||now>=self.close||now.saturating_sub(self.reconciliation_attempt_ms)<60_000{return None;}
        let end=now.checked_sub(15_000)?;let start=end.checked_sub(30_000)?;
        if start<=through||start<self.open||self.discarded_through>=start{return None;}
        let request=self.reconciliation_window(start,end)?;
        self.reconciliation_attempt_ms=now;self.reconciliation_error=None;Some(request)
    }
    pub fn reconciliation_window(&self,start:u64,end:u64)->Option<crate::auction_reconciliation::Request> {
        if self.symbol.is_empty()||start>=end||start<self.open||end>self.close||self.discarded_through>=start||self.observed_since.is_none_or(|t|t>start){return None;}
        let live:Vec<_>=self.raw_ticks.iter().filter(|t|t.at>=start&&t.at<end).cloned().collect();
        if live.len()>crate::auction_reconciliation::MAX_ROWS{return None;}
        Some(crate::auction_reconciliation::Request{symbol:self.symbol.clone(),epoch:self.epoch,start_ms:start,end_ms:end,live})
    }
    pub fn reconciliation_current(&self,request:&crate::auction_reconciliation::Request)->bool {
        self.epoch==request.epoch&&self.symbol==request.symbol&&self.backfill.is_none()
    }
    pub fn finish_reconciliation(&mut self,request:&crate::auction_reconciliation::Request,result:Result<crate::auction_reconciliation::Report,&'static str>)->bool {
        if !self.reconciliation_current(request){return false;}
        match result {Ok(r)=>{self.reconciliation=Some(r);self.reconciliation_error=None;},Err(e)=>{self.reconciliation_error=Some(e.into());}}
        true
    }
    pub(crate) fn record_rest_exclusion(&mut self,reason:&str,size:f64,conditions:&[u32]) {
        self.quality.exclude(reason,size);
        for c in conditions.iter().copied().collect::<std::collections::BTreeSet<_>>() {if snipers_core::stock_volume_policy::condition(c)!=snipers_core::stock_volume_policy::Decision::Eligible{*self.quality.excluded_conditions.entry(c).or_default()+=1;}}
    }
    pub fn gap(&mut self) {let epoch=self.epoch.saturating_add(1);*self=Self::default();self.epoch=epoch;}
    pub fn begin_history(&mut self,now:u64)->Option<(u64,u64,u64)>{
        if self.symbol.is_empty()||self.open==0||now<self.open||now>=self.close||self.backfill.is_some()||self.history_through.is_some(){return None;}
        self.backfill=Some((now,Arc::new(Mutex::new((vec![],false)))));self.history_status="loading".into();
        Some((self.open,now,self.epoch))
    }
    pub fn history_progress(&mut self,epoch:u64,pages:usize,rows:usize)->bool{
        if self.epoch!=epoch||self.backfill.is_none(){return false;}self.history_pages=pages;self.history_rows=rows;true
    }
    pub fn history_failed(&mut self,epoch:u64,message:&str){if self.epoch==epoch{self.backfill=None;self.history_status=message.into();}}
    pub fn history_trade(&mut self,symbol:&str,p:f64,v:f64,t:u64,q:u64,through:u64)->Result<(),&'static str>{
        if t>through||!p.is_finite()||p<=0.||!v.is_finite()||v<=0.{return Err("Operación REST inválida");}
        if self.sequence.is_some_and(|old|q<=old){return Err("Secuencia REST duplicada o fuera de orden");}
        self.history_trade_with_conditions(symbol,p,v,t,q,through,&[])
    }
    pub fn history_trade_with_conditions(&mut self,symbol:&str,p:f64,v:f64,t:u64,q:u64,through:u64,conditions:&[u32])->Result<(),&'static str>{
        if t>through||self.sequence.is_some_and(|old|q<=old){return Err("Invalid REST window or sequence");}
        let before=self.count;self.trade(symbol,p,v,t,Some(q),conditions,t);
        if self.count!=before+1{return Err("Historial fuera de sesión o capacidad de perfil excedida");}Ok(())
    }
    pub fn history_count(&self)->usize{self.count}
    pub fn apply_history(&mut self,epoch:u64,mut historical:Self,pages:usize,rows:usize,now:u64)->Result<(),&'static str>{
        if self.epoch!=epoch{return Err("Perfil sustituido durante la carga");}
        let Some((through,buffer))=self.backfill.take()else{return Err("Carga cancelada");};
        let (tail,lost)={let mut b=buffer.lock().unwrap();(std::mem::take(&mut b.0),b.1)};
        if lost||historical.symbol!=self.symbol||historical.open!=self.open{return Err("Cobertura de unión incompleta");}
        let mut tail=tail;tail.sort_by_key(|t|t.trade.id.parse::<u64>().unwrap_or(u64::MAX));
        for row in tail{let t=row.trade;if t.at>through{historical.accept(&t.symbol,t.price,t.volume,t.at,t.id.parse().map_err(|_|"Identidad inválida")?,now,row.separation);}}
        historical.pending=std::mem::take(&mut self.pending);historical.epoch=epoch;historical.history_through=Some(through);historical.history_status="ready".into();historical.history_pages=pages;historical.history_rows=rows;
        historical.observed_since=Some(self.open);*self=historical;Ok(())
    }
    pub fn trade(&mut self,symbol:&str,price:f64,size:f64,at:u64,sequence:Option<u64>,conditions:&[u32],now:u64) {
        if !price.is_finite()||price<=0.||!size.is_finite()||size<=0. {self.quality.exclude("invalid_price_or_quantity",size);return;}
        if now.saturating_sub(at)>15000 {self.quality.exclude("late_over_15s",size);return;}
        if !snipers_core::stock_volume_policy::permits_volume_at(conditions,at) {
            self.quality.exclude("condition_policy",size);
            for c in conditions.iter().copied().collect::<std::collections::BTreeSet<_>>() {if snipers_core::stock_volume_policy::volume_at(&[c],at)!=snipers_core::stock_volume_policy::Decision::Eligible{*self.quality.excluded_conditions.entry(c).or_default()+=1;}}
            return;
        }
        let Some(date)=calendar::date_at(at) else{self.quality.exclude("invalid_date",size);return};
        if calendar::date_at(now)!=Some(date){self.quality.exclude("foreign_session",size);return;}
        let Day::Open(session)=calendar::schedule(date) else{self.quality.exclude("closed_session",size);return};
        if at<session.open_ms||at>=session.close_ms{self.quality.exclude("outside_rth",size);return;}
        let Some(q)=sequence else{self.quality.exclude("missing_sequence",size);return};
        if self.symbol!=symbol||self.open!=session.open_ms {
            let initial_quality=if self.symbol.is_empty(){Some(std::mem::take(&mut self.quality))}else{None};
            self.gap();if let Some(q)=initial_quality{self.quality=q;}self.symbol=symbol.into();self.open=session.open_ms;self.close=session.close_ms;
        }
        self.release_pending(now);
        if at>now {
            let lead=at-now;
            if lead>1000{self.quality.exclude("future_over_1s",size);return;}
            if self.pending.len()>=4096 {self.pending_drops+=1;self.quality.exclude("pending_capacity",size);self.quality.capacity_degraded=true;return;}
            if !self.pending.contains_key(&(at,q)){self.pending.insert((at,q),ClassifiedTrade{trade:Trade{symbol:symbol.into(),at,id:q.to_string(),price,volume:size},separation:crate::auction_profile_partitions::separation(conditions)});self.deferred+=1;}else{self.quality.exclude("pending_duplicate",size);}
            self.maximum_lead_ms=self.maximum_lead_ms.max(lead);return;
        }
        self.accept(symbol,price,size,at,q,now,crate::auction_profile_partitions::separation(conditions));
    }
    fn release_pending(&mut self,now:u64){
        let due:Vec<_>=self.pending.range(..=(now,u64::MAX)).map(|(key,_)|*key).collect();
        for key in due {if let Some(row)=self.pending.remove(&key){let t=row.trade;if now.saturating_sub(t.at)>15000{self.pending_drops+=1;self.quality.exclude("pending_expired",t.volume);}else{self.accept(&t.symbol,t.price,t.volume,t.at,key.1,now,row.separation);self.released+=1;}}}
    }
    fn accept(&mut self,symbol:&str,price:f64,size:f64,at:u64,q:u64,now:u64,separation:Option<&'static str>){
        if self.sequence.is_some_and(|old|q<=old){self.quality.exclude("duplicate_or_out_of_order",size);return;}
        let key=((price/0.01+1e-9).floor() as i64,(at-self.open)/1800000);
        if !self.bins.contains_key(&key)&&self.bins.len()>=200000 {self.quality.exclude("bin_capacity",size);self.quality.capacity_degraded=true;return;}
        let volume=self.bins.get(&key).copied().unwrap_or(0.)+size;
        let weighted=self.weighted+price*size;
        if !volume.is_finite()||!weighted.is_finite(){self.quality.exclude("numeric_overflow",size);self.quality.capacity_degraded=true;return;}
        self.bins.insert(key,volume);self.weighted=weighted;self.count+=1;
        self.partitions.add(price,size,at,self.open,separation);
        self.sequence=Some(q);self.observed_since.get_or_insert(at);self.last_trade=at;
        if let Some((through,buffer))=&self.backfill{if at>*through{let mut b=buffer.lock().unwrap();if b.0.len()<1_000_000{b.0.push(ClassifiedTrade{trade:Trade{symbol:symbol.into(),at,id:q.to_string(),price,volume:size},separation});}else{b.1=true;}}}
        self.raw_ticks.push_back(Trade{symbol:symbol.into(),at,id:q.to_string(),price,volume:size});
        while self.raw_ticks.front().is_some_and(|t|t.at.saturating_add(900000)<now)||self.raw_ticks.len()>200000{if let Some(t)=self.raw_ticks.pop_front(){self.discarded_through=self.discarded_through.max(t.at);}}
    }
    pub fn snapshot(&mut self,symbol:&str,now:u64,connected:bool,bars:Vec<Candle>)->Option<serde_json::Value> {
        if connected&&self.symbol==symbol{self.release_pending(now);}
        if !connected||self.symbol!=symbol||now<self.open||now>=self.close||self.count==0||self.last_trade>now||now-self.last_trade>15000{return None;}
        let mut bars=bars;bars.retain(|b|b.timestamp_ms>=now.saturating_sub(1200000)&&b.timestamp_ms+60000<=now);
        // Each row represents an exact fixed-bin/bracket aggregate, not an invented print.
        let trades=self.bins.iter().map(|(&(bin,bracket),&volume)|Trade {
            symbol:symbol.into(),at:self.open+bracket*1800000,id:format!("bin:{bin}:{bracket}"),
            price:(bin as f64+0.5)*0.01,volume,
        }).collect();
        let mut r:Reading=auction_map::read(&Input {symbol:symbol.into(),start_ms:self.open,end_ms:self.close,
            as_of_ms:now,bin_size:0.01,bracket_ms:1800000,value_fraction:0.7,complete_window:false,trades,bars:bars.clone()}).ok()?;
        r.accepted_trades=self.count;r.vwap=self.weighted/r.total_volume;
        let (upper,lower)=self.rejection.snapshot(&r,&bars,&self.raw_ticks,self.observed_since,self.discarded_through,now);
        let mut value=serde_json::to_value(r).ok()?;
        value["rejectionUpper"]=serde_json::to_value(upper).ok()?;value["rejectionLower"]=serde_json::to_value(lower).ok()?;
        value["clockQueue"]=serde_json::json!({"pending":self.pending.len(),"deferred":self.deferred,"released":self.released,"maximumLeadMs":self.maximum_lead_ms,"dropped":self.pending_drops});
        value["observedSinceMs"]=serde_json::json!(self.observed_since);
        value["reconciliation"]=serde_json::json!({"latest":self.reconciliation,"error":self.reconciliation_error,"lastAttemptMs":self.reconciliation_attempt_ms,"wholeSessionCertified":false});
        value["dataQuality"]=serde_json::json!({"populationId":snipers_core::stock_volume_policy::ID,"volumePolicy":snipers_core::stock_volume_policy::metadata(),"unit":"shares","sourceScope":"current_engine_epoch_admission_calls","acceptedEvents":self.count,"acceptedShares":value["totalVolume"],"exclusions":self.quality,"pendingShares":self.pending.values().map(|t|t.trade.volume).sum::<f64>(),"reconciled":false,"marketCoveragePercent":null,"conditionCountsMayOverlap":true,"historyReplacementResetsCounters":true});
        value["profilePopulations"]=self.partitions.snapshot(symbol,self.open,self.close,now,value["pocLow"].as_f64()?,value["totalVolume"].as_f64()?);
        value["scope"]=serde_json::json!("observed-rth-ticks-fixed-bin-bracket-aggregates");
        value["sessionHistory"]=serde_json::json!({"status":self.history_status,"throughMs":self.history_through,"pages":self.history_pages,"rows":self.history_rows,"fromOpen":self.history_through.is_some(),"liveProvisional":true});
        Some(value)
    }
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn streaming_profile_conserves_volume_and_rejects_replays_future_foreign_sessions_and_gaps(){
        let t=calendar::legacy_eastern_ms("2026-10-09 10:00:00").unwrap();let mut e=Engine::default();
        e.trade("QQQ",100.01,20.,t,Some(1),&[0],t);
        e.trade("QQQ",100.02,30.,t+1,Some(2),&[0],t+1);
        e.trade("QQQ",100.02,30.,t+1,Some(2),&[0],t+1);
        e.trade("QQQ",101.,999.,t+20,Some(3),&[0],t+2);
        e.trade("QQQ",101.,999.,t+2,Some(4),&[15],t+2);
        let r=e.snapshot("QQQ",t+2,true,vec![]).unwrap();
        assert_eq!(r["totalVolume"],50.);assert_eq!(r["acceptedTrades"],2);assert_eq!(r["provisional"],true);
        assert!((r["vwap"].as_f64().unwrap()-100.016).abs()<1e-9);
        assert!(e.snapshot("SPY",t+2,true,vec![]).is_none());assert!(e.snapshot("QQQ",t+2,false,vec![]).is_none());
        e.gap();assert!(e.snapshot("QQQ",t+2,true,vec![]).is_none());
    }
    #[test]fn compressed_bins_match_raw_tick_profile(){
        let t=calendar::legacy_eastern_ms("2026-10-09 10:00:00").unwrap();let mut e=Engine::default();let mut raw=vec![];
        for i in 0..100 {let at=t+i*60000;let price=100.+(i%5) as f64*0.05;let volume=(i+1) as f64;
            e.trade("QQQ",price,volume,at,Some(i+1),&[0],at);
            raw.push(Trade{symbol:"QQQ".into(),at,id:i.to_string(),price,volume});}
        let now=t+99*60000;let input=Input{symbol:"QQQ".into(),start_ms:e.open,end_ms:e.close,as_of_ms:now,bin_size:0.01,bracket_ms:1800000,value_fraction:0.7,complete_window:false,trades:raw,bars:vec![]};
        let a=serde_json::to_value(auction_map::read(&input).unwrap()).unwrap();let b=e.snapshot("QQQ",now,true,vec![]).unwrap();
        for k in ["bins","totalVolume","pocLow","pocHigh","val","vah","acceptedTrades"]{assert_eq!(a[k],b[k],"{k}");}
    }
}

#[cfg(test)] mod runtime_audit_tests {
 use super::*;
 #[test] fn freshness_boundary_recovers_without_erasing_accumulated_bins() {
  let t=calendar::legacy_eastern_ms("2026-10-12 11:00:00").unwrap();let mut e=Engine::default();
  e.trade("QQQ",746.71,100.,t,Some(1),&[0],t);
  assert!(e.snapshot("QQQ",t+15000,true,vec![]).is_some());
  assert!(e.snapshot("QQQ",t+15001,true,vec![]).is_none());
  e.trade("QQQ",746.76,50.,t+16000,Some(2),&[0],t+16000);
  let r=e.snapshot("QQQ",t+16000,true,vec![]).unwrap();
  assert_eq!(r["totalVolume"],150.);assert_eq!(r["acceptedTrades"],2);assert_eq!(r["observedSinceMs"],t);
 }
 #[test] fn explicit_gap_restarts_scope_and_cannot_claim_prior_session_volume() {
  let t=calendar::legacy_eastern_ms("2026-10-12 11:00:00").unwrap();let mut e=Engine::default();
  e.trade("IWM",278.86,100.,t,Some(5),&[0],t);e.gap();
  e.trade("IWM",278.56,20.,t+60000,Some(1),&[0],t+60000);
  let r=e.snapshot("IWM",t+60000,true,vec![]).unwrap();assert_eq!(r["totalVolume"],20.);assert_eq!(r["observedSinceMs"],t+60000);assert_eq!(r["provisional"],true);
 }
}

/// Massive ds preserves fractional shares. Invalid reported decimals are not silently replaced.
pub use crate::trade_units::stock_trade_size as trade_size;
#[cfg(test)] mod quality_tests {
 use super::*;
 #[test] fn exclusions_preserve_profile_and_do_not_claim_market_coverage(){
 let t=calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();let mut e=Engine::default();
 e.trade("QQQ",100.,10.5,t,Some(1),&[0],t);
 e.trade("QQQ",100.,20.,t,Some(2),&[15,15,999],t);
 e.trade("QQQ",100.,30.,t-15001,Some(3),&[0],t);
 e.trade("QQQ",100.,10.5,t,Some(1),&[0],t);
 let r=e.snapshot("QQQ",t,true,vec![]).unwrap();let q=&r["dataQuality"];
 assert_eq!(r["totalVolume"],10.5);
 assert_eq!(q["exclusions"]["excluded"]["condition_policy"]["knownShares"],20.);
 assert_eq!(q["exclusions"]["excludedConditions"]["15"],1);
 assert_eq!(q["exclusions"]["excludedConditions"]["999"],1);
 assert_eq!(q["exclusions"]["excluded"]["late_over_15s"]["knownShares"],30.);
 assert_eq!(q["exclusions"]["excluded"]["duplicate_or_out_of_order"]["events"],1);
 assert!(q["marketCoveragePercent"].is_null());assert_eq!(q["reconciled"],false);
 }
 #[test] fn capacity_loss_is_explicit(){
 let t=calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();let mut e=Engine::default();e.trade("QQQ",100.,1.,t,Some(1),&[0],t);
 for i in 0..200000 {e.bins.insert((i,0),1.);}
 e.trade("QQQ",3000.,7.,t,Some(2),&[0],t);
 assert!(e.quality.capacity_degraded);assert_eq!(e.quality.excluded["bin_capacity"].known_shares,7.);assert_eq!(e.count,1);
 }
 #[test] fn rest_exclusions_survive_first_valid_trade_and_gap_resets_scope(){
 let t=calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();let mut e=Engine::default();
 e.record_rest_exclusion("rest_correction_pending",12.25,&[0]);
 e.record_rest_exclusion("condition_policy",3.,&[15]);
 e.history_trade("QQQ",100.,1.5,t,1,t).unwrap();
 assert_eq!(e.quality.excluded["rest_correction_pending"].known_shares,12.25);
 assert_eq!(e.quality.excluded_conditions[&15],1);
 e.gap();assert!(e.quality.excluded.is_empty());
 }
 #[test] fn scheduled_comparison_waits_for_history_lag_and_rejects_prior_epoch(){
 let t=calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();let mut e=Engine::default();
 e.trade("QQQ",100.,1.,t,Some(1),&[0],t);assert!(e.begin_reconciliation(t+60000).is_none());
 e.history_through=Some(t);e.trade("QQQ",100.,2.,t+20000,Some(2),&[0],t+20000);
 let request=e.begin_reconciliation(t+60000).unwrap();assert_eq!(request.start_ms,t+15000);assert_eq!(request.end_ms,t+45000);assert_eq!(request.live.len(),1);
 assert!(e.begin_reconciliation(t+60001).is_none());
 let r=crate::auction_reconciliation::compare(&request,&request.live,0,0,t+60000).unwrap();assert!(e.finish_reconciliation(&request,Ok(r.clone())));
 e.gap();assert!(!e.finish_reconciliation(&request,Ok(r)));assert!(e.reconciliation.is_none());
 }
 #[test] fn truncated_tape_cannot_start_reconciliation(){
 let t=calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();let mut e=Engine::default();e.trade("QQQ",100.,1.,t,Some(1),&[0],t);
 e.history_through=Some(t);e.discarded_through=t+20000;
 assert!(e.begin_reconciliation(t+60000).is_none());assert!(e.reconciliation_window(t,t+1000).is_none());
 }
}
#[cfg(test)]mod clock_tests{
 use super::*;
 #[test]fn deferred_trades_preserve_original_time_and_wait_until_causal(){let t=calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();let mut e=Engine::default();e.trade("QQQ",747.4,100.,t+300,Some(1),&[0],t);assert!(e.snapshot("QQQ",t+299,true,vec![]).is_none());let r=e.snapshot("QQQ",t+300,true,vec![]).unwrap();assert_eq!(r["totalVolume"],100.);assert_eq!(r["observedSinceMs"],t+300);assert_eq!(r["clockQueue"]["released"],1);}
 #[test]fn large_future_leads_remain_rejected(){let t=calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();let mut e=Engine::default();e.trade("QQQ",747.4,100.,t+1001,Some(1),&[0],t);assert!(e.snapshot("QQQ",t+2000,true,vec![]).is_none());}
 #[test]fn decimal_sizes_are_not_truncated_or_invented(){assert_eq!(trade_size(0.,Some("0.002000")),Some(0.002));assert_eq!(trade_size(100.,Some("100.5")),Some(100.5));assert_eq!(trade_size(100.,None),Some(100.));for v in ["NaN","-1","0","bad"]{assert_eq!(trade_size(100.,Some(v)),None);}}
 #[test]fn queued_duplicates_do_not_inflate_volume_and_gap_drops_pending(){let t=calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();let mut e=Engine::default();for _ in 0..2{e.trade("QQQ",747.4,10.,t+300,Some(1),&[0],t);}assert_eq!(e.snapshot("QQQ",t+300,true,vec![]).unwrap()["totalVolume"],10.);e.trade("QQQ",747.5,20.,t+600,Some(2),&[0],t+300);e.gap();assert!(e.snapshot("QQQ",t+600,true,vec![]).is_none());}
}

#[cfg(test)]mod causal_order_test{use super::*;#[test]fn release_due_trades_before_accepting_a_later_sequence(){let t=calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();let mut e=Engine::default();e.trade("QQQ",747.4,100.,t+300,Some(1),&[0],t);e.trade("QQQ",747.5,50.,t+350,Some(2),&[0],t+400);assert_eq!(e.snapshot("QQQ",t+400,true,vec![]).unwrap()["totalVolume"],150.);}}

#[cfg(test)]mod session_history_tests{
 use super::*;
 #[test]fn joins_session_history_and_concurrent_live_tail_without_counting_overlap_twice(){
  let open=calendar::legacy_eastern_ms("2026-10-12 09:30:00").unwrap();let now=open+3*3600000;
  let mut live=Engine::default();live.trade("NVDA",225.,10.,now,Some(20),&[0],now);
  let (_,through,epoch)=live.begin_history(now).unwrap();
  live.trade("NVDA",226.,30.,now+1000,Some(21),&[0],now+1000);
  let mut history=Engine::default();history.history_trade("NVDA",220.,100.,open,1,through).unwrap();history.history_trade("NVDA",225.,10.,now,20,through).unwrap();
  live.apply_history(epoch,history,2,2,now+1000).unwrap();
  let r=live.snapshot("NVDA",now+1000,true,vec![]).unwrap();assert_eq!(r["acceptedTrades"],3);assert_eq!(r["totalVolume"],140.);assert_eq!(r["observedSinceMs"],open);assert_eq!(r["sessionHistory"]["fromOpen"],true);assert_eq!(r["sessionHistory"]["throughMs"],through);
  live.trade("NVDA",227.,20.,now+2000,Some(22),&[0],now+2000);assert_eq!(live.snapshot("NVDA",now+2000,true,vec![]).unwrap()["totalVolume"],160.);
 }
 #[test]fn gap_rejects_old_download_and_pending_future_ticks_remain_causal(){
  let t=calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();let mut live=Engine::default();live.trade("QQQ",747.,10.,t,Some(20),&[0],t);let (_,through,epoch)=live.begin_history(t).unwrap();
  let mut history=Engine::default();history.history_trade("QQQ",747.,10.,t,20,through).unwrap();live.trade("QQQ",748.,20.,t+300,Some(21),&[0],t);
  live.apply_history(epoch,history.clone(),1,1,t).unwrap();assert_eq!(live.snapshot("QQQ",t+299,true,vec![]).unwrap()["totalVolume"],10.);assert_eq!(live.snapshot("QQQ",t+300,true,vec![]).unwrap()["totalVolume"],30.);
  live.gap();assert!(live.apply_history(epoch,history,1,1,t).is_err());
 }
 #[test]fn lost_live_tail_never_publishes_session_coverage(){
  let t=calendar::legacy_eastern_ms("2026-10-12 12:00:00").unwrap();let mut live=Engine::default();live.trade("QQQ",747.,10.,t,Some(20),&[0],t);let (_,through,epoch)=live.begin_history(t).unwrap();
  live.backfill.as_ref().unwrap().1.lock().unwrap().1=true;
  let mut history=Engine::default();history.history_trade("QQQ",747.,10.,t,20,through).unwrap();assert!(live.apply_history(epoch,history,1,1,t).is_err());assert_eq!(live.snapshot("QQQ",t,true,vec![]).unwrap()["sessionHistory"]["fromOpen"],false);
 }
}

#[cfg(test)]mod partition_transport_tests{
 use super::*;
 #[test]fn contingent_classification_survives_future_queue_and_history_join(){
  let t=calendar::legacy_eastern_ms("2026-10-13 12:00:00").unwrap();let mut live=Engine::default();
  live.trade("QQQ",100.,100.,t,Some(1),&[],t);let (_,through,epoch)=live.begin_history(t).unwrap();
  live.trade("QQQ",101.,650000.,t+300,Some(2),&[41,53],t);
  let mut history=Engine::default();history.history_trade_with_conditions("QQQ",100.,100.,t,1,through,&[]).unwrap();
  live.apply_history(epoch,history,1,1,t).unwrap();
  let before=live.snapshot("QQQ",t+299,true,vec![]).unwrap();assert_eq!(before["profilePopulations"]["separated"]["shares"],0.);
  let r=live.snapshot("QQQ",t+300,true,vec![]).unwrap();assert_eq!(r["pocLow"],101.);assert_eq!(r["profilePopulations"]["continuous"]["pocLow"],100.);assert_eq!(r["profilePopulations"]["separated"]["shares"],650000.);
  live.gap();assert!(live.snapshot("QQQ",t+301,true,vec![]).is_none());
 }
 #[test]fn history_seed_and_live_tail_keep_separated_prints_and_no_overlap(){
  let t=calendar::legacy_eastern_ms("2026-10-13 12:00:00").unwrap();let mut live=Engine::default();live.trade("QQQ",100.,10.,t,Some(10),&[],t);
  let (_,through,epoch)=live.begin_history(t).unwrap();live.trade("QQQ",101.,20.,t+100,Some(11),&[2],t+100);
  let mut history=Engine::default();history.history_trade_with_conditions("QQQ",100.,10.,t,10,through,&[]).unwrap();live.apply_history(epoch,history,1,1,t+100).unwrap();
  let r=live.snapshot("QQQ",t+100,true,vec![]).unwrap();let p=&r["profilePopulations"];assert_eq!(r["totalVolume"],30.);assert_eq!(p["continuousShares"],10.);assert_eq!(p["separated"]["shares"],20.);assert_eq!(p["volumeBalanceDifference"],0.);
  live.trade("QQQ",101.,20.,t+100,Some(11),&[2],t+100);assert_eq!(live.snapshot("QQQ",t+100,true,vec![]).unwrap()["totalVolume"],30.);
 }
}
