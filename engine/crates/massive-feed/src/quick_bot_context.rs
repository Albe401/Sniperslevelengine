//! Independent quick-reading confirmations. ATR is magnitude, OI is context, never extra votes.
use serde_json::{json,Value};
use crate::intraday_strategy::Reading;
fn n(v:&Value)->Option<f64>{v.as_f64().filter(|n|n.is_finite())}
fn fresh(v:&Value,now:u64,age:u64)->bool{v.as_u64().is_some_and(|t|t<=now&&now-t<=age&&snipers_core::calendar::date_at(t)==snipers_core::calendar::date_at(now))}
pub fn apply(s:&Value,r:&mut Reading,now:u64){
 let market_date=snipers_core::calendar::date_at(now).map(|d|d.to_string()).unwrap_or_default();
 let connected=s["stocks"]["phase"]=="streaming"&&fresh(&s["publishedMs"],now,3000);
 let c=&s["cvd"];let p=&c["regular"];let mut cvd="pending";let mut cvd_side="";
 if connected&&c["symbol"]==r.symbol&&c["date"]==market_date&&c["available"]==true&&fresh(&c["at"],now,3000)&&fresh(&p["lastMs"],now,15000){
  if let (Some(b),Some(a),Some(u),Some(d),Some(coverage))=(n(&p["buy"]),n(&p["sell"]),n(&p["unknown"]),n(&p["delta"]),n(&p["coveragePct"])){
   let total=b+a+u;
   if b>=0.&&a>=0.&&u>=0.&&total>0.&&(d-b+a).abs()<=1e-6_f64.max(d.abs()*1e-9)&&((b+a)/total*100.-coverage).abs()<0.01&&coverage>=70.{
    cvd_side=if d>u{"CALL"}else if -d>u{"PUT"}else{""};
    if !cvd_side.is_empty()&&matches!(r.entry_side.as_str(),"CALL"|"PUT"){cvd=if cvd_side==r.entry_side{"confirms"}else{"contradicts"};}
   }
  }
 }
 if cvd=="contradicts"&&r.eligible {r.eligible=false;r.blockers.push("CVD de acciones vigente contradice el lado de entrada".into());}
 let atr=intraday_atr(s,now,connected);
 let target_atr=atr.zip(r.next).map(|(a,t)|(t-r.spot).abs()/a);
 let structure=&s["optionSnapshotStatus"];
 let horizons:Vec<Value>=["DAY","WEEK","NEXT4"].into_iter().map(|mode|{
  let matches:Vec<_>=structure["horizons"].as_array().into_iter().flatten().filter(|h|h["mode"]==mode||(mode=="DAY"&&h["mode"]=="NEXT_SESSION")).collect();
  let Some(h)=matches.first().filter(|_|matches.len()==1)else{return json!({"mode":mode,"side":"pending"})};
  let e=&h["exposure"];let gross=n(&e["callDex"]).zip(n(&e["putDex"])).map(|(a,b)|a.abs()+b.abs()).unwrap_or(0.);
  let dates:Vec<_>=h["expirations"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
  let all:std::collections::BTreeSet<_>=structure["expirations"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|d|*d>=market_date.as_str()&&snipers_core::calendar::day_bounds(d).is_some()).collect();
  let expected:Vec<_>=if mode=="NEXT4"{all.iter().copied().take(4).collect()}else if mode=="DAY"&&h["mode"]=="DAY"{vec![market_date.as_str()]}else if mode=="DAY"{all.iter().copied().filter(|d|*d>market_date.as_str()).take(1).collect()}else{
   snipers_core::calendar::day_bounds(&market_date).and_then(|(start,_)|{let weekday=(start/86400000+4)%7;snipers_core::calendar::date_at(start+43200000+(5+7-weekday)%7*86400000)}).map(|friday|all.iter().copied().filter(|v|*v<=friday.to_string().as_str()).collect()).unwrap_or_default()
  };
  let ok=connected&&structure["symbol"].as_str().is_none_or(|symbol|symbol==r.symbol)&&structure["phase"]=="ready"&&fresh(&structure["observedMs"],now,900000)&&h["observedMs"]==structure["observedMs"]&&!dates.is_empty()&&dates==expected&&(mode!="NEXT4"||dates.len()==4)&&e["dexComplete"]==true&&e["eligible"].as_u64().is_some_and(|v|v>0&&Some(v)==e["deltaUsed"].as_u64())&&e["unknownOi"]==0&&gross.is_finite()&&gross>0.;
  let values=n(&e["callDexPct"]).zip(n(&e["putDexPct"])).zip(n(&e["callDex"]).zip(n(&e["putDex"]))).zip(n(&e["netDex"]));
  let side=if let Some((((a,b),(call,put)),net))=values.filter(|&(((a,b),(call,put)),net)|ok&&a>=0.&&b>=0.&&a<=100.&&b<=100.&&(a+b-100.).abs()<0.01&&call>=0.&&put<=0.&&(net-call-put).abs()<1e-6_f64.max(gross*1e-9)&&(a-100.*call/gross).abs()<0.01){let _=(call,put,net);if a>=52.5{"CALL"}else if b>=52.5{"PUT"}else{"neutral"}}else{"pending"};
  json!({"mode":mode,"side":side,"expirations":dates,"observedMs":h["observedMs"]})
 }).collect();
 let alignment=if horizons.iter().all(|h|h["side"]=="CALL"){"CALL"}else if horizons.iter().all(|h|h["side"]=="PUT"){"PUT"}else if horizons.iter().any(|h|h["side"]=="pending"){"pending"}else{"mixed"};
 // Structural OI and ATR remain context; neither manufactures an entry or an exhaustion exit.
 let range_minutes=if s["rthOpen"].is_object(){json!([s["rthOpen"]])}else{s["chartHistory"]["minutes"].clone()};
 let amplitude=crate::movement_room::calculate(&r.symbol,&structure["dailyRange"],
  if connected&&s["symbol"]==r.symbol&&structure["phase"]=="ready"&&structure["symbol"].as_str().is_none_or(|v|v==r.symbol){structure["observedMs"].as_u64()}else{None},
  r.spot,s["ticks"][&r.symbol]["tradeMs"].as_u64(),&range_minutes,now);
 r.quick_context=json!({"version":"quick-reading-link-v1","symbol":r.symbol,"at":now,"amplitude":amplitude,"cvd":{"status":cvd,"side":cvd_side},"atr5m":atr,"targetAtr":target_atr,"structure":horizons,"alignment":alignment,"levels":r.levels,"entryBasis":r.entry_basis,"absorption":"pending","greekSupportAvailable":r.greek_support.available});
}
fn intraday_atr(s:&Value,now:u64,connected:bool)->Option<f64>{
 if !connected{return None;}let date=snipers_core::calendar::date_at(now)?;let snipers_core::calendar::Day::Open(day)=snipers_core::calendar::schedule(date)else{return None};
 if now<day.open_ms||now>=day.close_ms{return None;}
 let end=day.open_ms+(now-day.open_ms)/300000*300000;let start=end.checked_sub(4500000)?;if start<day.open_ms{return None;}
 let mut rows=std::collections::BTreeMap::new();
 for c in s["chartHistory"]["minutes"].as_array()?{let t=c["timestampMs"].as_u64()?;if t<start||t>=end{continue;}let (h,l,o,close,v)=(n(&c["high"])?,n(&c["low"])?,n(&c["open"])?,n(&c["close"])?,n(&c["volume"])?);if t%60000!=0||l<=0.||h<o.max(close)||l>o.min(close)||v<0.||rows.insert(t,(h,l,close)).is_some(){return None;}}
 let mut bars=Vec::new();for i in 0..15 {let mut high=f64::NEG_INFINITY;let mut low=f64::INFINITY;let mut close=0.;for j in 0..5{let &(h,l,c)=rows.get(&(start+i*300000+j*60000))?;high=high.max(h);low=low.min(l);close=c;}bars.push((high,low,close));}snipers_core::atr::sma14(&bars)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn real_pipeline_uses_market_date_not_tracker_session_id(){
  let mut s:Value=serde_json::from_str(include_str!("../../../fixtures/intraday-strategy.json")).unwrap();let now=s["publishedMs"].as_u64().unwrap();let day=snipers_core::calendar::date_at(now).unwrap().to_string();
  let baseline=crate::intraday_strategy::read_paper(&s,now);assert!(baseline.ready);assert!(baseline.session.contains("QQQ:"));
  let opposite=baseline.entry_side=="CALL";s["cvd"]=json!({"symbol":"QQQ","date":day,"at":now,"available":true,"regular":{"buy":if opposite{10.}else{90.},"sell":if opposite{90.}else{10.},"unknown":0.,"delta":if opposite{-80.}else{80.},"coveragePct":100.,"lastMs":now}});
  let r=crate::intraday_strategy::read_paper(&s,now);assert_eq!(r.session,baseline.session);assert_eq!(r.quick_context["cvd"]["status"],"contradicts");assert!(!r.eligible);assert!(r.management_ready);
  s["cvd"]["date"]=json!("2026-01-01");assert_eq!(crate::intraday_strategy::read_paper(&s,now).quick_context["cvd"]["status"],"pending");
 }
 #[test]fn real_pipeline_aligns_exact_daily_weekly_and_four_expiration_universes(){
  let mut s:Value=serde_json::from_str(include_str!("../../../fixtures/intraday-strategy.json")).unwrap();let now=s["publishedMs"].as_u64().unwrap();let dates=["2026-09-25","2026-09-28","2026-09-29","2026-09-30"];
  let st=s["optionSnapshotStatus"]["horizons"][0]["structure"].clone();s["optionSnapshotStatus"]["symbol"]=json!("QQQ");s["optionSnapshotStatus"]["expirations"]=json!(dates);s["optionSnapshotStatus"]["horizons"]=json!(["DAY","WEEK","NEXT4"].into_iter().map(|mode|json!({"mode":mode,"observedMs":now,"expirations":if mode=="NEXT4"{dates.to_vec()}else{vec![dates[0]]},"structure":st,"exposure":{"dexComplete":true,"eligible":4,"deltaUsed":4,"unknownOi":0,"callDex":80.,"putDex":-20.,"netDex":60.,"callDexPct":80.,"putDexPct":20.}})).collect::<Vec<_>>());
  let r=crate::intraday_strategy::read_paper(&s,now);assert!(r.session.starts_with("QQQ:2026-09-25:"));assert_eq!(r.quick_context["alignment"],"CALL");
  s["optionSnapshotStatus"]["symbol"]=json!("SPY");assert_eq!(crate::intraday_strategy::read_paper(&s,now).quick_context["alignment"],"pending");
 }
 #[test]fn opposing_cvd_blocks_only_new_entry_and_rejects_stale_foreign_or_corrupt(){
  let now=snipers_core::calendar::rfc3339_ms("2026-10-02T11:00:00-04:00").unwrap();
  let base=json!({"publishedMs":now,"stocks":{"phase":"streaming"},"cvd":{"symbol":"QQQ","date":"2026-10-02","at":now,"available":true,"regular":{"buy":10.,"sell":90.,"unknown":0.,"delta":-80.,"coveragePct":100.,"lastMs":now}}});
  for i in 0..4{let mut s=base.clone();if i==1{s["cvd"]["symbol"]=json!("NVDA");}if i==2{s["cvd"]["at"]=json!(now-3001);}if i==3{s["cvd"]["regular"]["delta"]=json!(-1.);}
   let mut r=Reading{symbol:"QQQ".into(),session:"2026-10-02".into(),entry_side:"CALL".into(),eligible:true,management_ready:true,..Default::default()};apply(&s,&mut r,now);assert_eq!(r.eligible,i!=0);assert!(r.management_ready);assert_eq!(r.quick_context["alignment"],"pending");}
 }
 #[test]fn atr_requires_exact_closed_minutes_and_is_magnitude_only(){
  let open=snipers_core::calendar::rfc3339_ms("2026-10-02T09:30:00-04:00").unwrap();let now=open+75*60000;
  let minutes:Vec<_>=(0..75).map(|i|json!({"timestampMs":open+i*60000,"open":100.,"high":101.,"low":99.,"close":100.,"volume":10.})).collect();let mut s=json!({"chartHistory":{"minutes":minutes}});
  assert_eq!(intraday_atr(&s,now,true),Some(2.));assert_eq!(intraday_atr(&s,now-1,true),None);s["chartHistory"]["minutes"].as_array_mut().unwrap().remove(5);assert_eq!(intraday_atr(&s,now,true),None);
 }
 #[test]fn structure_requires_exact_four_expirations_and_conserved_delta(){
  let now=snipers_core::calendar::rfc3339_ms("2026-10-02T11:00:00-04:00").unwrap();
  let dates=["2026-10-02","2026-10-05","2026-10-06","2026-10-07"];
  let horizons:Vec<_>=["DAY","WEEK","NEXT4"].into_iter().map(|mode|json!({"mode":mode,"observedMs":now,"expirations":if mode=="NEXT4"{dates.to_vec()}else{vec![dates[0]]},"exposure":{"dexComplete":true,"eligible":4,"deltaUsed":4,"unknownOi":0,"callDex":80.,"putDex":-20.,"netDex":60.,"callDexPct":80.,"putDexPct":20.}})).collect();
  let base=json!({"publishedMs":now,"stocks":{"phase":"streaming"},"optionSnapshotStatus":{"phase":"ready","observedMs":now,"expirations":dates,"horizons":horizons}});
  for i in 0..4{let mut s=base.clone();if i==1{s["optionSnapshotStatus"]["horizons"][2]["expirations"]=json!([dates[0],dates[1],dates[1],dates[2]]);}if i==2{s["optionSnapshotStatus"]["horizons"][1]["exposure"]["netDex"]=json!(0.);}if i==3{s["optionSnapshotStatus"]["horizons"][0]["observedMs"]=json!(now-1);}
   let mut r=Reading{symbol:"QQQ".into(),session:"2026-10-02".into(),eligible:true,..Default::default()};apply(&s,&mut r,now);assert_eq!(r.quick_context["alignment"],if i==0{"CALL"}else{"pending"});assert!(r.eligible,"OI context must not manufacture a directional entry filter");}
 }
 #[test]fn benchmark_five_asset_contexts_is_bounded(){
  let s:Value=serde_json::from_str(include_str!("../../../fixtures/intraday-strategy.json")).unwrap();let now=s["publishedMs"].as_u64().unwrap();let start=std::time::Instant::now();
  for _ in 0..500{for symbol in ["QQQ","SPY","NVDA","AAPL","TSLA"]{let mut r=Reading{symbol:symbol.into(),session:"2026-09-24".into(),..Default::default()};apply(&s,&mut r,now);}}
  let ms=start.elapsed().as_secs_f64()*1000./500.;println!("quick bot context, five assets: {ms:.3} ms/cycle");assert!(ms<50.);
 }
 #[test]fn amplitude_is_native_context_and_does_not_modify_admission(){
  let open=snipers_core::calendar::rfc3339_ms("2026-10-02T09:30:00-04:00").unwrap();let now=open+4*3600000;
  let mut s=json!({"symbol":"AAPL","publishedMs":now,"stocks":{"phase":"streaming"},"ticks":{"AAPL":{"tradeMs":now}},"rthOpen":{"timestampMs":open,"open":100.,"high":101.,"low":99.,"close":100.},"chartHistory":{"minutes":[]},"optionSnapshotStatus":{"symbol":"AAPL","phase":"ready","observedMs":now,"dailyRange":{"ready":true,"spot":100.,"expectedMoveDollars":10.,"expectedMovePct":10.}}});
  for foreign in [false,true]{if foreign{s["optionSnapshotStatus"]["symbol"]=json!("QQQ");}let mut r=Reading{symbol:"AAPL".into(),session:"2026-10-02".into(),spot:103.,eligible:true,..Default::default()};apply(&s,&mut r,now);assert!(r.eligible);assert_eq!(r.quick_context["amplitude"]["available"],!foreign);if !foreign{assert_eq!(r.quick_context["amplitude"]["callDollars"],7.);assert_eq!(r.quick_context["amplitude"]["putDollars"],13.);}}
 }
}
