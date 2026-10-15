//! Portable pattern reader and prospective learning journal. No broker execution or weight updates.
use massive_feed::intraday_strategy::Reading;
use serde_json::{json,Value};
use snipers_core::calendar::{self,Day};
use std::{collections::BTreeMap};
fn reference()-> &'static Value {crate::bot_asset_history::reference()}
fn n(v:&Value)->Option<f64>{v.as_f64().filter(|n|n.is_finite())}
#[derive(Clone)]struct Bar{t:u64,o:f64,h:f64,l:f64,c:f64,v:f64,vw:f64}
#[derive(Clone)]struct Point{b:Bar,e:[Option<f64>;4],vwap:f64,atr:Option<f64>}
#[derive(Clone,Default)]struct Ema{sum:f64,count:usize,value:Option<f64>}
impl Ema{fn push(&mut self,c:f64,period:usize)->Option<f64>{if let Some(v)=self.value{self.value=Some(v+(c-v)*2./(period as f64+1.));}else{self.count+=1;self.sum+=c;if self.count==period{self.value=Some(self.sum/period as f64);}}self.value}}
fn bars(frame:&Value,now:u64)->Option<Vec<Bar>>{
 let mut rows=BTreeMap::new();for v in frame["chartHistory"]["minutes"].as_array()?{
  let t=v["timestampMs"].as_u64()?;if t.checked_add(60000)?>now{continue;}
  let Day::Open(day)=calendar::schedule(calendar::date_at(t)?)else{continue};if t<day.open_ms||t>=day.close_ms{continue;}
  let b=Bar{t,o:n(&v["open"])?,h:n(&v["high"])?,l:n(&v["low"])?,c:n(&v["close"])?,v:n(&v["volume"])?,vw:n(&v["vwap"]).or_else(||n(&v["vw"]))?};
  if (t-day.open_ms)%60000!=0||b.l<=0.||b.c<=0.||b.o<=0.||b.vw<=0.||b.v<0.||b.h<b.o.max(b.c)||b.l>b.o.min(b.c)||b.h<b.l||rows.insert(t,b).is_some(){return None;}
 }Some(rows.into_values().collect())
}
#[cfg(test)]mod tests{
 use super::*;
 fn fixture(symbol:&str)->(Value,Reading,u64){
  let mut minutes=Vec::new();let mut last=0;
  for(date,count)in[("2026-10-08",390),("2026-10-09",390),("2026-10-12",300)]{let open=calendar::legacy_eastern_ms(&format!("{date} 09:30:00")).unwrap();for i in 0..count{let(o,h,l,c)=if date=="2026-10-12"&&i>=297{if i==299{(100.,100.6,99.95,100.5)}else{(100.6,100.8,100.4,100.6)}}else{(100.,100.2,99.8,100.)};last=open+i*60000;minutes.push(json!({"timestampMs":last,"open":o,"high":h,"low":l,"close":c,"volume":100.,"vwap":(o+c)/2.}));}}
  let now=last+60000;let frame=json!({"symbol":symbol,"publishedMs":now,"stocks":{"phase":"streaming"},"ticks":{symbol:{"tradeMs":now}},"chartHistory":{"minutes":minutes}});
  let r=Reading{symbol:symbol.into(),entry_side:"CALL".into(),spot:100.5,ready:true,eligible:true,management_ready:true,coverage:90.,flow_one:0.2,flow_five:0.3,next:Some(104.),quick_context:json!({"cvd":{"side":"CALL"},"optionActivity":{"state":"pending"}}),..Default::default()};(frame,r,now)
 }
 #[test]fn native_reader_sees_patterns_and_history_without_changing_the_strategy(){let(f,mut r,now)=fixture("QQQ");let before=serde_json::to_value(&r).unwrap();let mut state=State::default();let record=state.update(&f,&mut r,now).unwrap();assert_eq!(record["frames"].as_array().unwrap().len(),3);assert!(!record["newPatterns"].as_array().unwrap().is_empty());assert!(record["newPatterns"][0]["historicalEvidence"].is_object());assert!(record["shadowPosition"].is_object());let mut after=serde_json::to_value(&r).unwrap();after["quick_context"].as_object_mut().unwrap().remove("pullbackLearning");assert_eq!(before,after);assert!(state.update(&f,&mut r,now+1000).is_none());assert!(r.quick_context["pullbackLearning"]["algorithmContext"]["pullbackLearning"].is_null());}
 #[test]fn other_assets_get_portable_readings_but_no_invented_historical_validation(){let(f,mut r,now)=fixture("AAPL");let record=State::default().update(&f,&mut r,now).unwrap();assert_eq!(record["historicallyStudied"],true);assert!(!record["newPatterns"].as_array().unwrap().is_empty());assert_eq!(record["newPatterns"][0]["historicalEvidence"]["symbol"],"AAPL");assert_eq!(record["historicalCandidate"].as_array().unwrap().len(),3);}
 #[test]fn missing_forming_stale_foreign_or_disconnected_data_never_propose_an_entry(){
  for kind in 0..5{let(mut f,mut r,now)=fixture("QQQ");match kind{0=>{f["chartHistory"]["minutes"].as_array_mut().unwrap().remove(800);},1=>{for b in f["chartHistory"]["minutes"].as_array_mut().unwrap().iter_mut().rev().take(3){b["timestampMs"]=json!(now+60000);}},2=>f["publishedMs"]=json!(now-3001),3=>f["symbol"]=json!("SPY"),_=>f["stocks"]["phase"]=json!("disconnected")};let _=State::default().update(&f,&mut r,now);assert!(r.quick_context["pullbackLearning"]["proposedEntries"].as_array().unwrap().is_empty(),"case {kind}");}
 }
 #[test]fn baseline_blockers_or_opposed_cvd_prevent_shadow_entry(){for kind in 0..2{let(f,mut r,now)=fixture("QQQ");if kind==0{r.eligible=false;r.blockers.push("baseline veto".into());}else{r.quick_context["cvd"]["side"]=json!("PUT");}let report=State::default().update(&f,&mut r,now).unwrap();assert!(report["shadowPosition"].is_null());assert!(report["proposedEntries"].as_array().unwrap().iter().all(|p|p["algorithmConfirmed"]==false));}}
 #[test]fn shadow_exits_are_recorded_and_never_submit_orders(){let(f,mut r,now)=fixture("QQQ");let mut state=State::default();state.update(&f,&mut r,now).unwrap();r.spot=98.;let mut next=f.clone();next["publishedMs"]=json!(now+1000);next["ticks"]["QQQ"]["tradeMs"]=json!(now+1000);let record=state.update(&next,&mut r,now+1000).unwrap();assert_eq!(record["shadowExit"]["action"],"shadow_exit");assert!(record["shadowPosition"].is_null());assert_eq!(record["changesEntries"],false);}
 #[test]fn outcome_horizons_use_only_current_prices_and_reset_after_connection_gaps(){let(f,mut r,now)=fixture("QQQ");let mut state=State::default();state.update(&f,&mut r,now).unwrap();assert!(!state.pending.is_empty());for step in 1..=300{let t=now+step*1000;let mut next=f.clone();next["publishedMs"]=json!(t);next["ticks"]["QQQ"]["tradeMs"]=json!(t);let open_bar=json!({"timestampMs":now+((step-1)/60)*60000,"open":100.5,"high":100.8,"low":100.3,"close":100.6,"volume":100.,"vwap":100.55});if step%60==0{next["chartHistory"]["minutes"].as_array_mut().unwrap().extend((0..step/60).map(|i|{let mut b=open_bar.clone();b["timestampMs"]=json!(now+i*60000);b}));}else{next["chartHistory"]["minutes"].as_array_mut().unwrap().extend((0..step/60).map(|i|{let mut b=open_bar.clone();b["timestampMs"]=json!(now+i*60000);b}));}r.spot=100.6;let result=state.update(&next,&mut r,t);if step==300{let record=result.unwrap();assert!(record["outcomes"].as_array().unwrap().iter().any(|o|o["horizonMinutes"]==5&&o["status"]=="observed"));}}
  let mut disconnected=f;disconnected["stocks"]["phase"]=json!("disconnected");state.update(&disconnected,&mut r,now+301000);assert!(state.pending.is_empty());assert!(state.position.is_none());
 }
}
fn points(minutes:&[Bar],frame:u64,now:u64)->Vec<Point>{
 let mut emas:[Ema;4]=Default::default();let mut days:BTreeMap<String,Vec<Bar>>=BTreeMap::new();
 for b in minutes{if let Some(d)=calendar::date_at(b.t){days.entry(d.to_string()).or_default().push(b.clone());}}
 let today=calendar::date_at(now).map(|d|d.to_string()).unwrap_or_default();let mut out=Vec::new();
 for (date,bs)in days{let Some(d)=calendar::date_at(bs[0].t)else{continue};let Day::Open(day)=calendar::schedule(d)else{continue};
  if bs[0].t!=day.open_ms||bs.windows(2).any(|w|w[1].t!=w[0].t+60000){emas=Default::default();continue;}
  let mut pv=0.;let mut volume=0.;let mut prior:Option<Bar>=None;let mut trs=Vec::new();let mut current=Vec::new();
  if bs[0].t!=day.open_ms{emas=Default::default();}
  for group in bs.chunks(frame as usize){if group.len()!=frame as usize{break;}let b=&group[0];
   if (b.t-day.open_ms)%(frame*60000)!=0||group.iter().enumerate().any(|(i,v)|v.t!=b.t+i as u64*60000)||prior.as_ref().is_some_and(|p|b.t!=p.t+frame*60000){emas=Default::default();trs.clear();prior=None;current.clear();continue;}
   let v=group.iter().map(|b|b.v).sum::<f64>();let bar=Bar{t:b.t,o:b.o,h:group.iter().map(|b|b.h).fold(f64::NEG_INFINITY,f64::max),l:group.iter().map(|b|b.l).fold(f64::INFINITY,f64::min),c:group.last().unwrap().c,v,vw:if v>0.{group.iter().map(|b|b.v*b.vw).sum::<f64>()/v}else{group.last().unwrap().vw}};
   pv+=bar.v*bar.vw;volume+=bar.v;
   if let Some(p)=&prior{trs.push((bar.h-bar.l).max((bar.h-p.c).abs()).max((bar.l-p.c).abs()));}
   let atr=(trs.len()>=14).then(||trs.iter().rev().take(14).sum::<f64>()/14.);
   let e=std::array::from_fn(|i|emas[i].push(bar.c,[9,20,50,200][i]));
   current.push(Point{b:bar.clone(),e,vwap:if volume>0.{pv/volume}else{f64::NAN},atr});prior=Some(bar);
  }
  if date==today{out=current;}
 }out
}
fn level(p:&Point,i:usize)->Option<f64>{if i<4{p.e[i]}else{p.vwap.is_finite().then_some(p.vwap)}}
fn patterns(minutes:&[Bar],symbol:&str,now:u64)->Value{
 let mut frames=Vec::new();for frame in [1,2,3]{let ps=points(minutes,frame,now);let mut detected=Vec::new();let mut current=Value::Null;
  if ps.len()>=3{let i=ps.len()-1;let(b,p,pp)=(&ps[i],&ps[i-1],&ps[i-2]);let end=b.b.t+frame*60000;
   if b.e.iter().all(Option::is_some)&&now>=end&&now-end<frame*60000&&p.atr.is_some_and(|a|a>0.){
    current=json!({"closedMs":end,"ema9":b.e[0],"ema20":b.e[1],"ema50":b.e[2],"ema200":b.e[3],"vwap":b.vwap,"atr":b.atr});
    let atr=p.atr.unwrap();for(li,name)in ["e9","e20","e50","e200","vwap"].iter().enumerate(){let(Some(l),Some(pl))=(level(p,li),level(pp,li))else{continue};for side in [1.,-1.]{
     if ((if side>0.{p.b.l}else{p.b.h})-l)*side<=atr*0.25||((if side>0.{pp.b.l}else{pp.b.h})-pl)*side<=atr*0.25||b.b.l>l+atr*0.05||b.b.h<l-atr*0.05{continue;}
     let rebound=(b.b.c-l)*side>atr*0.1&&(b.b.c-b.b.o)*side>0.;
     let history=crate::bot_asset_history::pattern(symbol,frame,name,side,now);
     detected.push(json!({"key":format!("{symbol}:{frame}:{name}:{side}:{end}"),"symbol":symbol,"frame":frame,"levelName":name,"side":if side>0.{"CALL"}else{"PUT"},"closedMs":end,"level":l,"atr":atr,"rebound":rebound,"touchLow":b.b.l,"touchHigh":b.b.h,"historicalEvidence":history}));
    }}
   }
  }frames.push(json!({"frame":frame,"state":if current.is_null(){"pending_warmup_or_data"}else{"observing"},"indicators":current,"patterns":detected}));
 }json!(frames)
}
#[derive(Clone)]struct Pending{key:String,since:u64,price:f64,side:f64,done:[bool;3]}
#[derive(Clone)]struct ShadowTrade{key:String,since:u64,entry:f64,side:f64,stop:f64,initial_stop:f64,target:f64,frame:u64,last_trail:u64}
#[derive(Clone,Default)]pub struct State{last_bar:u64,last_observed:u64,session:String,frames:Value,pending:Vec<Pending>,seen:std::collections::BTreeSet<String>,position:Option<ShadowTrade>}
impl State{
 pub fn update(&mut self,frame:&Value,r:&mut Reading,now:u64)->Option<Value>{
  let mut report=json!({"version":"bot-pullback-learning-v1","symbol":r.symbol,"at":now,"mode":"shadow","state":"pending","historicallyStudied":crate::bot_asset_history::asset(&r.symbol,now).is_some(),"validatedForExecution":false,"changesEntries":false,"referenceSha256":reference()["sourceSha256"],"referencePeriod":{"from":reference()["from"],"to":reference()["to"]},"frames":[],"proposedEntries":[],"outcomes":[]});
  let connected=frame["symbol"]==r.symbol&&frame["stocks"]["phase"]=="streaming"&&frame["publishedMs"].as_u64().is_some_and(|t|t<=now&&now-t<=3000);
  let date=calendar::date_at(now).map(|d|d.to_string()).unwrap_or_default();
  if !connected||self.session!=date||self.last_observed>0&&now.saturating_sub(self.last_observed)>15000{
   report["discardedPendingMeasurements"]=json!(self.pending.len());report["discardedShadowPosition"]=json!(self.position.is_some());report["resetReason"]=json!(if !connected{"connection_pending"}else if self.session!=date{"session_changed"}else{"observation_gap"});
   *self=Self{session:date.clone(),..Default::default()};
  }
  if !connected{r.quick_context["pullbackLearning"]=report;return None;}
  self.last_observed=now;
  let price_ready=r.spot.is_finite()&&r.spot>0.&&frame["ticks"][&r.symbol]["tradeMs"].as_u64().is_some_and(|t|t<=now&&now-t<=15000);
  let Some(minutes)=bars(frame,now)else{r.quick_context["pullbackLearning"]=report;return None;};let Some(last)=minutes.last()else{r.quick_context["pullbackLearning"]=report;return None;};
  if calendar::date_at(last.t)!=calendar::date_at(now)||now.saturating_sub(last.t+60000)>60000{r.quick_context["pullbackLearning"]=report;return None;}
  let new_bar=last.t!=self.last_bar;if new_bar{self.frames=patterns(&minutes,&r.symbol,now);self.last_bar=last.t;}
  let mut proposed=Vec::new();let mut new_patterns=Vec::new();
  for f in self.frames.as_array().into_iter().flatten(){for p in f["patterns"].as_array().into_iter().flatten(){let Some(key)=p["key"].as_str()else{continue};if !new_bar||self.seen.contains(key){continue;}self.seen.insert(key.into());new_patterns.push(p.clone());
   if p["rebound"]!=true{continue;}let side=if p["side"]=="CALL"{1.}else{-1.};let atr=n(&p["atr"]).unwrap_or(0.);let raw_stop=n(&p[if side>0.{"touchLow"}else{"touchHigh"}]).map(|s|s-side*atr*0.1);let risk=raw_stop.map(|s|((r.spot-s)*side).max(atr*0.5));let stop=risk.map(|risk|r.spot-side*risk);let rr=r.next.zip(risk).filter(|(_,risk)|*risk>0.).map(|(t,risk)|(t-r.spot)*side/risk);
   let timely=p["closedMs"].as_u64().is_some_and(|t|t<=now&&now-t<=15000);
   let confirms=timely&&price_ready&&r.eligible&&r.ready&&r.entry_side==p["side"].as_str().unwrap_or("")&&r.coverage>=65.&&r.flow_one*side>=0.15&&r.flow_five*side>0.&&r.quick_context["cvd"]["side"]==p["side"]&&rr.is_some_and(|r|r>=1.8)&&risk.is_some_and(|risk|risk<=atr*3.);
   proposed.push(json!({"historicalEvidence":p["historicalEvidence"],"assetHistory":r.quick_context["assetHistory"],"patternKey":key,"side":p["side"],"frame":p["frame"],"entryReference":r.spot,"invalidation":stop,"structuralTarget":r.next,"rewardRisk":rr,"timelySignal":timely,"algorithmConfirmed":confirms,"state":if confirms{"shadow_entry_proposed"}else{"waiting_algorithm_confirmation"},"baselineBlockers":r.blockers,"exitHypotheses":["structural-target","level-invalidation","closed-EMA9-trail"],"ordersEnabled":false}));
   if confirms&&self.position.is_none(){self.position=Some(ShadowTrade{key:key.into(),since:now,entry:r.spot,side,stop:stop.unwrap(),initial_stop:stop.unwrap(),target:r.next.unwrap(),frame:p["frame"].as_u64().unwrap_or(1),last_trail:p["closedMs"].as_u64().unwrap_or(now)});}
   if price_ready&&self.pending.len()<200{self.pending.push(Pending{key:key.into(),since:now,price:r.spot,side,done:[false;3]});}
  }}
  let mut outcomes=Vec::new();if price_ready{for p in &mut self.pending{for(i,h)in[5,15,30].iter().enumerate(){let elapsed=now.saturating_sub(p.since);if !p.done[i]&&elapsed>=h*60000{p.done[i]=true;outcomes.push(json!({"patternKey":p.key,"observedMs":p.since,"measuredMs":now,"horizonMinutes":h,"status":if elapsed-h*60000<=15000{"observed"}else{"pending_missed_horizon"},"directionAdjustedPriceMove":if elapsed-h*60000<=15000{Some((r.spot-p.price)*p.side)}else{None},"basis":"live-observed-underlying-price-not-option-profit"}));}}}}
  self.pending.retain(|p|!p.done.iter().all(|d|*d));
  report["state"]=json!("observing");report["frames"]=self.frames.clone();report["proposedEntries"]=json!(proposed);report["outcomes"]=json!(outcomes);report["pendingMeasurements"]=json!(self.pending.len());report["newPatterns"]=json!(new_patterns);
  let mut shadow_event=Value::Null;
  if price_ready{if let Some(p)=&mut self.position{
   let reason=if now<=p.since{None}else if(r.spot-p.stop)*p.side<=0.{Some("invalidation_or_trailing_stop")}else if(r.spot-p.target)*p.side>=0.{Some("structural_target")}else if now-p.since>=1800000{Some("time_limit_30m")}else{None};
   if let Some(reason)=reason{shadow_event=json!({"patternKey":p.key,"action":"shadow_exit","reason":reason,"entryMs":p.since,"exitMs":now,"entryReference":p.entry,"exitReference":r.spot,"directionAdjustedMove":(r.spot-p.entry)*p.side,"initialInvalidation":p.initial_stop,"basis":"observed-ticks-no-broker-fill-no-option-pnl"});}
   else if let Some(f)=self.frames.as_array().and_then(|fs|fs.iter().find(|f|f["frame"]==p.frame)){let i=&f["indicators"];if let Some(t)=i["closedMs"].as_u64().filter(|t|*t>p.last_trail&&*t>p.since){if let(Some(e),Some(a))=(n(&i["ema9"]),n(&i["atr"])){let next=e-p.side*a*0.1;p.stop=if p.side>0.{p.stop.max(next)}else{p.stop.min(next)};p.last_trail=t;}}}
  }}
  if !shadow_event.is_null(){self.position=None;}
  report["shadowExit"]=shadow_event.clone();report["shadowPosition"]=self.position.as_ref().map(|p|json!({"patternKey":p.key,"entryMs":p.since,"entryReference":p.entry,"invalidation":p.stop,"structuralTarget":p.target,"frame":p.frame,"ordersEnabled":false})).unwrap_or(Value::Null);
  let mut algorithm=r.quick_context.clone();if let Some(m)=algorithm.as_object_mut(){m.remove("pullbackLearning");}
  report["algorithmContext"]=algorithm; // Never recursively retain earlier learning reports.
  report["algorithmReading"]=json!({"ready":r.ready,"eligible":r.eligible,"managementReady":r.management_ready,"entrySide":r.entry_side,"entryBasis":r.entry_basis,"coverage":r.coverage,"flowOne":r.flow_one,"flowFive":r.flow_five,"levels":r.levels,"next":r.next,"greekSupport":r.greek_support,"neuroContext":r.neuro_context,"blockers":r.blockers});
  report["historicalCandidate"]=crate::bot_asset_history::asset(&r.symbol,now).map(|a|a["chosen"].clone()).unwrap_or(json!([]));
  r.quick_context["pullbackLearning"]=report.clone();
  if new_bar||!outcomes.is_empty()||!shadow_event.is_null(){Some(report)}else{None}
 }
}
