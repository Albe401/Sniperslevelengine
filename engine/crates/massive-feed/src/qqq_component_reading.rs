//! One canonical, passive QQQ basket reading for UI and Paper. No broker access.
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use crate::{live,qqq_components::{self,Holdings}};
use snipers_core::calendar::{self,Day};
#[derive(Clone,Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Leader {pub symbol:String,pub contribution:f64}
#[derive(Clone,Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Report {
 pub version:String,pub symbol:String,pub at:u64,pub as_of:String,pub phase:String,pub available:bool,
 pub call:Option<f64>,pub put:Option<f64>,pub coverage:f64,pub option_coverage:f64,pub selected_weight:f64,pub price_coverage:f64,
 pub leaders:Vec<Leader>,pub daily_leaders:Vec<Leader>,pub opening_leaders:Vec<Leader>,pub missing:Vec<String>,
 pub relation:String,pub reason:String,pub qqq_price:Option<f64>,pub qqq_price_at:Option<u64>,pub quality:String,
}
fn n(v:&Value)->Option<f64>{v.as_f64().filter(|v|v.is_finite())}
fn fresh(v:&Value,now:u64,age:u64)->bool{v.as_u64().is_some_and(|t|t<=now&&now-t<=age&&calendar::date_at(t)==calendar::date_at(now))}
fn clamp(v:f64)->f64{v.clamp(-1.,1.)}
fn valid_bar(b:&Value)->bool{
 let Some((o,h,l,c,v))=n(&b["open"]).zip(n(&b["high"])).zip(n(&b["low"])).zip(n(&b["close"])).zip(n(&b["volume"])).map(|((((o,h),l),c),v)|(o,h,l,c,v))else{return false};
 l>0.&&v>=0.&&h>=o.max(c)&&l<=o.min(c)&&b["timestampMs"].as_u64().is_some()
}
fn price(f:&Value,symbol:&str,now:u64)->Option<f64>{let t=&f["ticks"][symbol];(t["symbol"]==symbol&&fresh(&t["tradeMs"],now,15000)).then(||n(&t["price"]).filter(|v|*v>0.)).flatten()}
fn five_return(f:&Value,now:u64)->Option<f64>{
 let end=now/60000*60000-60000;let start=end.checked_sub(300000)?;
 if calendar::date_at(start)!=calendar::date_at(now){return None;}
 let bars=f["chartHistory"]["minutes"].as_array()?;
 let mut closes=Vec::new();
 for i in 0..6 {let at=start+i*60000;let matching=bars.iter().filter(|b|b["timestampMs"]==at).collect::<Vec<_>>();if matching.len()!=1||!valid_bar(matching[0]){return None;}closes.push(n(&matching[0]["close"])?);}
 Some(closes[5]/closes[0]-1.)
}
fn cvd_signal(f:&Value,symbol:&str,phase:&str,now:u64)->Option<f64>{
 let c=&f["cvd"];let p=&c[if phase=="Premarket"{"premarket"}else{"regular"}];
 if c["symbol"]!=symbol||c["available"]!=true||!fresh(&c["at"],now,3000)||!fresh(&p["lastMs"],now,15000)||c["date"]!=calendar::date_at(now)?.to_string(){return None;}
 let (buy,sell,unknown,delta)=(n(&p["buy"])? ,n(&p["sell"])? ,n(&p["unknown"])? ,n(&p["delta"])?);
 if phase=="Premarket"&&(!p["sinceMs"].as_u64().is_some_and(|t|t<=now.saturating_sub(300000))||p["gaps"].as_u64()!=Some(0)){return None;}
 let total=buy+sell+unknown;
 if buy<0.||sell<0.||unknown<0.||total<=0.||(buy+sell)/total<0.7||(delta-buy+sell).abs()>1e-6_f64.max(delta.abs()*1e-9){return None;}
 Some(if delta.abs()>unknown{delta/total}else{0.})
}
fn flow_signal(f:&Value,minutes:u64,expiry:&str,now:u64)->Option<f64>{
 if f["options"]["phase"]!="streaming"||!fresh(&f["options"]["lastEventMs"],now,15000)||f["flowExpiry"]!=expiry{return None;}
 let w=f["flow"]["intraday_windows"].as_array()?.iter().find(|w|w["minutes"]==minutes)?;
 if w["premium_complete"]!=true||w["event_count"].as_u64()?==0||!fresh(&f["flow"]["asof_ms"],now,3000){return None;}
 let b=&w["buckets"];let cb=n(&b["CALL BUY"]["premium"])?;let ps=n(&b["PUT SELL"]["premium"])?;let pb=n(&b["PUT BUY"]["premium"])?;let cs=n(&b["CALL SELL"]["premium"])?;let u=n(&b["UNKNOWN"]["premium"])?;
 let total=cb+ps+pb+cs+u;if [cb,ps,pb,cs,u].iter().any(|v|*v<0.)||total<=0.||(total-u)/total<0.7{return None;}
 Some((cb+ps-pb-cs)/total)
}
fn price_signal(f:&Value,now:u64,open:u64)->Option<f64>{
 let minutes=f["chartHistory"]["minutes"].as_array()?;
 let mut bars=Vec::<(f64,f64,u64)>::new();let last=(now.saturating_sub(open)/120000)*120000+open;
 for i in (1..=6).rev(){let Some(at)=last.checked_sub(i*120000)else{continue};if at<open{continue;}
  let a=minutes.iter().filter(|b|b["timestampMs"]==at).collect::<Vec<_>>();let b=minutes.iter().filter(|b|b["timestampMs"]==at+60000).collect::<Vec<_>>();
  if a.len()!=1||b.len()!=1||!valid_bar(a[0])||!valid_bar(b[0]){bars.clear();continue;}
  bars.push((n(&a[0]["open"])?,n(&b[0]["close"])?,at));
 }
 let latest=bars.last()?;if now-latest.2>300000{return None;}
 let mut pieces=Vec::new();if latest.1!=latest.0{pieces.push(if latest.1>latest.0{0.35}else{-0.35});}
 if fresh(&f["metrics"]["asofMs"],now,180000){if let Some(v)=n(&f["metrics"]["vwap"]).filter(|v|*v>0.){let d=(latest.1-v)/v*100.;pieces.push(if d.abs()<=0.03{0.}else{clamp(d/0.35)});}}
 let closes=bars.iter().rev().take(4).map(|b|b.1).collect::<Vec<_>>();if closes.len()>=3{pieces.push(clamp((closes[0]-closes.last()?)/closes.last()?*100./0.35));}
 (!pieces.is_empty()).then(||pieces.iter().sum::<f64>()/pieces.len()as f64)
}
/// Same 40/40/20 direction formula, scoped to the actual sampled maturity.
fn score(f:&Value,symbol:&str,phase:&str,now:u64,open:u64)->Option<(f64,bool)>{
 let spot=price(f,symbol,now)?;
 if phase=="Premarket"{if now/60000*60000-360000<open.saturating_sub(19800000){return None;}let change=five_return(f,now)?;let cvd=cvd_signal(f,symbol,phase,now);return Some(((50.+15.*(0.6*clamp(change*100./0.35)+0.4*cvd.unwrap_or(0.))).round(),false));}
 let expiry=f["flowExpiry"].as_str()?;let today=calendar::date_at(now)?.to_string();if expiry<today.as_str(){return None;}
 let snap=&f["optionSnapshotStatus"];if snap["phase"]!="ready"||!fresh(&snap["observedMs"],now,900000){return None;}
 let horizons=snap["horizons"].as_array()?;
 let h=horizons.iter().find(|h|h["observedMs"]==snap["observedMs"]&&h["structure"]["complete"]==true&&h["expirations"].as_array().is_some_and(|e|e.len()==1&&e[0]==expiry))?;
 let st=&h["structure"];let cw=n(&st["callWall"]["strike"])?;let pw=n(&st["putWall"]["strike"])?;let cs=n(&st["callWall"]["score"])?;let ps=n(&st["putWall"]["score"])?;
 if pw>=cw||pw<=0.||!(0. ..=100.).contains(&cs)||!(0. ..=100.).contains(&ps){return None;}
 let flow=flow_signal(f,5,expiry,now)?;let p=price_signal(f,now,open)?;
 let mut structure=if spot<pw{-0.7}else if spot>cw{0.7}else{(ps-cs)/100.};
 if let Some(m)=n(&st["selected"]["M1"]){if (m-spot).abs()>spot*0.0003{structure+=(m-spot).signum()*0.2;}}
 if let Some(routes)=st["routes"].as_array(){for b in routes{if let Some((target,strength))=n(&b["barrier"]).zip(n(&b["barrierScore"])){if (target-spot).abs()/spot<=0.005&&(0. ..=100.).contains(&strength){structure-=(target-spot).signum()*strength/100.*0.2;}}}}
 let raw=0.4*p+0.4*flow+0.2*clamp(structure);let backing=if raw>=0.&&spot>=pw{Some(ps)}else if raw<0.&&spot<=cw{Some(cs)}else{None};
 let net=raw*backing.map_or(0.5,|v|0.5+v/200.)*if st["provisional"]==true{0.75}else{1.};
 Some(((50.+50.*net).round(),true))
}
pub fn read(h:&Holdings,frames:&[Value],connected:bool,now:u64)->Report{
 let mut out=Report{version:"qqq-components-v2".into(),symbol:"QQQ".into(),at:now,as_of:h.as_of.clone(),relation:"Pendiente".into(),quality:"Pendiente".into(),reason:"Esperando evidencia propia".into(),..Default::default()};
 let Some(date)=calendar::date_at(now)else{return out};let Day::Open(session)=calendar::schedule(date)else{return out};
 out.phase=if now<session.open_ms{"Premarket"}else{"Sesión RTH"}.into();
 if !connected||now<session.open_ms-19800000||now>=session.close_ms||h.rows.is_empty()||h.rows.len()>12||!(0..=7).any(|d|calendar::date_at(now.saturating_sub(d*86400000)).is_some_and(|day|day.to_string()==h.as_of)){return out;}
 let mut seen=std::collections::BTreeSet::new();if h.rows.iter().any(|r|!seen.insert(&r.symbol)||!r.weight.is_finite()||r.weight<=0.||r.weight>100.){return out;}
 out.selected_weight=h.rows.iter().map(|r|r.weight).sum();if out.selected_weight>100.{return Report::default();}
 let own=|symbol:&str|{let matches=frames.iter().filter(|f|f["symbol"]==symbol).collect::<Vec<_>>();if matches.len()!=1{return None;}let f=matches[0];(f["stocks"]["phase"]=="streaming"&&f["metrics"]["session"]==date.to_string()&&fresh(&f["publishedMs"],now,3000)).then_some(f)};
 let mut weighted=0.;
 for row in &h.rows{
  let Some(f)=own(&row.symbol)else{out.missing.push(row.symbol.clone());continue};
  if let Some((call,full))=score(f,&row.symbol,&out.phase,now,session.open_ms){out.coverage+=row.weight;weighted+=row.weight*call;if full{out.option_coverage+=row.weight;}}else{out.missing.push(row.symbol.clone());}
  if let Some(ret)=five_return(f,now){out.price_coverage+=row.weight;out.leaders.push(Leader{symbol:row.symbol.clone(),contribution:row.weight*ret});}
  if let Some(spot)=price(f,&row.symbol,now){let base=&f["priceBaselines"];
   if let Some(p)=n(&base["priorClose"]).filter(|v|*v>0.){if base["priorAt"].as_u64().is_some_and(|t|calendar::date_at(t).is_some_and(|d|d<date)&&now.saturating_sub(t)<7*86400000){out.daily_leaders.push(Leader{symbol:row.symbol.clone(),contribution:row.weight*(spot/p-1.)});}}
   if let Some(p)=n(&base["open"]).filter(|v|*v>0.){if base["openAt"]==session.open_ms&&now>=session.open_ms+60000{out.opening_leaders.push(Leader{symbol:row.symbol.clone(),contribution:row.weight*(spot/p-1.)});}}
  }
 }
 for rows in [&mut out.leaders,&mut out.daily_leaders,&mut out.opening_leaders]{rows.sort_by(|a,b|b.contribution.abs().total_cmp(&a.contribution.abs()));}
 out.available=out.coverage>=40.&&out.coverage>=out.selected_weight*0.8;
 if out.available{out.call=Some((weighted/out.coverage).round());out.put=out.call.map(|v|100.-v);out.quality=if out.phase=="Premarket"{"Premarket · precio; CVD si disponible"}else{"Confluencia parcial del ETF"}.into();}
 let side=|c:f64|if c>=60.{"CALL"}else if c<=40.{"PUT"}else{"MIXTO"};
 if let Some(f)=own("QQQ") {out.qqq_price=price(f,"QQQ",now);out.qqq_price_at=f["ticks"]["QQQ"]["tradeMs"].as_u64();if let Some(c)=out.call{out.relation=match score(f,"QQQ",&out.phase,now,session.open_ms){Some((q,_)) if side(c)=="MIXTO"||side(q)=="MIXTO"=>"Mixta",Some((q,_)) if side(c)==side(q)=>"Confirma QQQ",Some(_)=>"Divergencia",None=>"QQQ pendiente"}.into();}}
 out.reason=format!("Peso seguido {:0.1}% de QQQ; lectura {:0.1}%; opciones {:0.1}%; precio 5m {:0.1}%. Mínimo: 40% del ETF y 80% del peso seguido. Premarket usa precio de acciones 5m, normalizado a 0,35%, con peso 60%; CVD solo con cobertura ≥70%, peso 40%. Intensidad limitada a 59/41 sin CVD y 65/35 con CVD; sin opciones ni probabilidad validada. RTH usa prima 5m del vencimiento propio y niveles de ese vencimiento. GOOG/GOOGL separados. Aportes estimados; sin atribución causal.",out.selected_weight,out.coverage,out.option_coverage,out.price_coverage);
 out
}
pub fn apply_bot(report:&Value,r:&mut crate::intraday_strategy::Reading,now:u64){
 if r.symbol!="QQQ"{return;}
 r.quick_context["qqqComponents"]=report.clone();
 let Ok(q)=serde_json::from_value::<Report>(report.clone())else{return;};
 if q.symbol!="QQQ"||q.version!="qqq-components-v2"||q.at>now||now-q.at>3000||calendar::date_at(q.at)!=calendar::date_at(now)||!q.available||q.phase!="Sesión RTH"||q.option_coverage<40.||q.option_coverage<q.selected_weight*0.8{return;}
 if !q.selected_weight.is_finite()||q.selected_weight>100.||q.coverage<40.||q.coverage<q.selected_weight*0.8||q.option_coverage>q.coverage||q.coverage>q.selected_weight{return;}
 if let Some(c)=q.call.filter(|c|c.is_finite()&&(0. ..=100.).contains(c)&&q.put==Some(100.-c)) {if (r.entry_side=="CALL"&&c<=40.)||(r.entry_side=="PUT"&&c>=60.){r.eligible=false;r.blockers.push("Componentes QQQ: oposición con cobertura suficiente".into());}}
}
pub fn bot_demand(armed:bool,qqq_only:bool,top5:bool,targets:&[String],held:&[String])->bool{
 (armed&&(qqq_only||!top5||targets.iter().any(|r|r=="QQQ")))||held.iter().any(|r|r=="QQQ")
}
pub async fn run(state:live::SharedState,directory:std::path::PathBuf){
 let mut holdings:Option<Holdings>=None;let mut retry_at=0;let mut journal=Journal::default();
 loop {
  let now=live::now_ms();let demand={let s=state.lock().unwrap();s.qqq_ui_enabled||s.qqq_bot_demand};
  if !demand{let mut s=state.lock().unwrap();s.qqq_components.clear();s.qqq_report=Value::Null;}
  else {
   if now>=retry_at {match qqq_components::fetch().await{Ok(h)=>{holdings=Some(h);retry_at=now+60000;},Err(_)=>{retry_at=now+60000;}}}
   if let Some(h)=&holdings{
    let connected={let mut s=state.lock().unwrap();s.qqq_components=h.rows.iter().map(|r|r.symbol.clone()).chain(std::iter::once("QQQ".into())).collect();s.scanner_options_notify.notify_one();s.stocks.phase=="streaming"};
    let captured=live::now_ms();let mut frames=Vec::new();
    for symbol in h.rows.iter().map(|r|r.symbol.as_str()).chain(std::iter::once("QQQ")){if snipers_core::universe::stock_asset(symbol).is_some(){frames.push(live::bot_frame(&state,symbol,captured));}}
    let report=read(h,&frames,connected,captured);state.lock().unwrap().qqq_report=json!(report);
    let rows=journal.update(&report,captured);if !rows.is_empty(){let path=directory.join("qqq-components-v2.jsonl");let _=tokio::task::spawn_blocking(move||append(&path,&rows)).await;}
   }
  }
  tokio::time::sleep(std::time::Duration::from_secs(2)).await;
 }
}
#[derive(Default)]
struct Journal{last_minute:u64,pending:Vec<(Report,[bool;3])>}
impl Journal {
 fn update(&mut self,r:&Report,now:u64)->Vec<Value>{
  let mut rows=Vec::new();
  if let Some(spot)=r.qqq_price.filter(|v|*v>0.) {for (before,done) in &mut self.pending{for (i,m) in [1,5,15].iter().enumerate(){let at=before.at+*m*60000;if !done[i]&&now>=at{done[i]=true;let valid=now-at<=5000&&calendar::date_at(before.at)==calendar::date_at(now)&&r.qqq_price_at.is_some_and(|t|t>=at&&t<=now&&now-t<=3000);rows.push(json!({"kind":"outcome","at":before.at,"minutes":m,"call":before.call,"phase":before.phase,"returnBps":if valid{before.qqq_price.map(|p|10000.*(spot/p-1.))}else{None},"quality":if valid{"complete"}else{"missing"}}));}}}}
  self.pending.retain(|(r,done)|!done.iter().all(|d|*d)&&now-r.at<=960000);
  if r.available&&r.qqq_price.is_some()&&r.qqq_price_at.is_some_and(|t|t<=now&&now-t<=3000)&&now/60000!=self.last_minute{self.last_minute=now/60000;rows.push(json!({"kind":"reading","report":r}));self.pending.push((r.clone(),[false;3]));}
  rows
 }
}
fn append(path:&std::path::Path,rows:&[Value])->std::io::Result<()>{
 use std::io::Write;
 if std::fs::metadata(path).is_ok_and(|m|m.len()>20*1024*1024){let old=path.with_extension("previous.jsonl");if old.exists(){std::fs::remove_file(&old)?;}std::fs::rename(path,old)?;}
 let mut file=std::fs::OpenOptions::new().create(true).append(true).open(path)?;for row in rows{writeln!(file,"{row}")?;}Ok(())
}
/// Bounded trailing journal summary; an observation is not an options trade.
pub fn journal_summary(path:&std::path::Path)->Value{
 use std::io::{Read,Seek,SeekFrom};
 let Ok(mut file)=std::fs::File::open(path)else{return json!({"readings":0,"outcomes":[],"scope":"Sin historial todavía"})};
 let length=file.metadata().map(|m|m.len()).unwrap_or(0);let offset=length.saturating_sub(2*1024*1024);
 if file.seek(SeekFrom::Start(offset)).is_err(){return Value::Null;}let mut bytes=Vec::new();if file.take(2*1024*1024).read_to_end(&mut bytes).is_err(){return Value::Null;}
 let text=String::from_utf8_lossy(&bytes);let mut readings=0;let mut summaries=[(0u64,0u64,0f64,0u64);3];
 for line in text.lines().skip(if offset>0{1}else{0}){let Ok(v)=serde_json::from_str::<Value>(line)else{continue};if v["kind"]=="reading"{readings+=1;continue;}if v["kind"]!="outcome"{continue;}
  let Some(i)=[1,5,15].iter().position(|m|v["minutes"]==*m)else{continue};let s=&mut summaries[i];if let Some(ret)=n(&v["returnBps"]).filter(|_|v["quality"]=="complete"){s.0+=1;s.2+=ret;if let Some(call)=n(&v["call"]){if call>=60.||call<=40.{s.1+=1;if call>=60.&&ret>0.||call<=40.&&ret<0.{s.3+=1;}}}}}
 json!({"readings":readings,"scope":"Últimos 2 MiB del diario; ventanas solapadas, sin calibración estadística", "outcomes":summaries.iter().enumerate().map(|(i,s)|json!({"minutes":([1,5,15][i]),"complete":s.0,"directional":s.1,"aligned":s.3,"meanReturnBps":if s.0>0{Some(s.2/s.0 as f64)}else{None}})).collect::<Vec<_>>()})
}
#[cfg(test)]
mod tests {
 use super::*;
 fn now()->u64{calendar::rfc3339_ms("2026-10-05T11:00:00-04:00").unwrap()}
 fn holdings()->Holdings{Holdings{as_of:"2026-10-02".into(),source:qqq_components::SOURCE.into(),rows:vec![qqq_components::Holding{symbol:"NVDA".into(),weight:45.},qqq_components::Holding{symbol:"AAPL".into(),weight:10.}]}}
 fn frame(symbol:&str,now:u64,bull:bool,expiry:&str)->Value{
  let date=calendar::date_at(now).unwrap();let Day::Open(day)=calendar::schedule(date)else{panic!()};
  let minutes:Vec<_>=(0..90).map(|i|{let at=now-90*60000+i*60000;let o=100.+if bull{i as f64*0.01}else{-(i as f64)*0.01};json!({"timestampMs":at,"open":o,"close":o+if bull{0.01}else{-0.01},"high":o+0.1,"low":o-0.1,"volume":100.})}).collect();
  let b=json!({"CALL BUY":{"premium":if bull{900}else{100}},"PUT BUY":{"premium":if bull{100}else{900}},"CALL SELL":{"premium":0},"PUT SELL":{"premium":0},"UNKNOWN":{"premium":0}});
  let period=json!({"buy":if bull{900}else{100},"sell":if bull{100}else{900},"unknown":0,"delta":if bull{800}else{-800},"sinceMs":now-600000,"lastMs":now,"gaps":0});
  json!({"symbol":symbol,"publishedMs":now,"stocks":{"phase":"streaming"},"metrics":{"session":date.to_string(),"phase":"Sesión RTH","asofMs":now,"vwap":100.},"ticks":{symbol:{"symbol":symbol,"price":if bull{100.9}else{99.1},"tradeMs":now}},"options":{"phase":"streaming","lastEventMs":now},"flowExpiry":expiry,"flow":{"asof_ms":now,"intraday_windows":[{"minutes":5,"event_count":10,"premium_complete":true,"buckets":b}]},"chartHistory":{"minutes":minutes},"cvd":{"symbol":symbol,"date":date.to_string(),"at":now,"available":true,"premarket":period,"regular":period},"priceBaselines":{"priorClose":100.,"priorAt":calendar::rfc3339_ms("2026-10-02T15:59:00-04:00").unwrap(),"open":100.,"openAt":day.open_ms},"optionSnapshotStatus":{"phase":"ready","observedMs":now,"horizons":[{"mode":"NEXT_SESSION","observedMs":now,"expirations":[expiry],"structure":{"complete":true,"callWall":{"strike":102.,"score":60.},"putWall":{"strike":98.,"score":90.},"selected":{"M1":101.},"routes":[]}}]}})
 }
 fn good()->Report{read(&holdings(),&[frame("NVDA",now(),true,"2026-10-09"),frame("AAPL",now(),false,"2026-10-09"),frame("QQQ",now(),true,"2026-10-05")],true,now())}
 #[test]fn uses_real_next_maturity_and_daily_anchors(){let r=good();assert!(r.available);assert_eq!(r.option_coverage,55.);assert!(r.call.unwrap()>60.);assert_eq!(r.relation,"Confirma QQQ");assert_eq!(r.leaders.len(),2);assert_eq!(r.daily_leaders.len(),2);assert_eq!(r.opening_leaders.len(),2);}
 #[test]fn rejects_low_coverage_without_voting_missing_as_neutral(){let mut h=holdings();h.rows[0].weight=30.;h.rows[1].weight=25.;let r=read(&h,&[frame("NVDA",now(),true,"2026-10-09")],true,now());assert_eq!(r.coverage,30.);assert!(!r.available);assert!(r.call.is_none());assert_eq!(r.missing,vec!["AAPL"]);}
 #[test]fn premarket_uses_actions_and_cvd_with_options_closed(){let t=calendar::rfc3339_ms("2026-10-05T08:00:00-04:00").unwrap();let mut a=frame("NVDA",t,true,"2026-10-09");a["options"]["phase"]=json!("closed");let r=read(&holdings(),&[a],true,t);assert!(r.available);assert_eq!(r.phase,"Premarket");assert_eq!(r.option_coverage,0.);assert!(r.call.unwrap()<=65.);}
 #[test]fn incomplete_premarket_cvd_keeps_bounded_price_only_reading(){let t=calendar::rfc3339_ms("2026-10-05T08:00:00-04:00").unwrap();let mut a=frame("NVDA",t,true,"2026-10-09");a["cvd"]["premarket"]["sinceMs"]=json!(t-1000);let r=read(&holdings(),&[a],true,t);assert!(r.available);assert!(r.call.unwrap()<=59.);assert_eq!(r.option_coverage,0.);}
 #[test]fn price_only_premarket_requires_own_complete_recent_candles(){let t=calendar::rfc3339_ms("2026-10-05T08:00:00-04:00").unwrap();for kind in 0..4{let mut a=frame("NVDA",t,true,"2026-10-09");a["cvd"]=Value::Null;match kind{0=>a["ticks"]["NVDA"]["tradeMs"]=json!(t-15001),1=>{a["chartHistory"]["minutes"].as_array_mut().unwrap().pop();},2=>a["stocks"]["phase"]=json!("closed"),_=>a["symbol"]=json!("SPY")};assert!(!read(&holdings(),&[a],true,t).available);}}
 #[test]fn rejects_foreign_stale_future_duplicate_maturity_and_gaps(){for kind in 0..7{let mut a=frame("NVDA",now(),true,"2026-10-09");match kind{0=>a["symbol"]=json!("SPY"),1=>a["publishedMs"]=json!(now()-3001),2=>a["publishedMs"]=json!(now()+1),3=>a["flowExpiry"]=json!("2026-10-12"),4=>a["options"]["lastEventMs"]=json!(now()-15001),5=>{a["chartHistory"]["minutes"].as_array_mut().unwrap().truncate(80);},_=>{}}
  let frames=if kind==6{vec![a.clone(),a]}else{vec![a]};assert!(!read(&holdings(),&frames,true,now()).available,"case {kind}");}}
 #[test]fn bot_qqq_only_and_five_modes_keep_background_demand(){assert!(bot_demand(true,true,true,&[],&[]));assert!(bot_demand(true,false,true,&["NVDA".into(),"QQQ".into()],&[]));assert!(!bot_demand(true,false,true,&["NVDA".into()],&[]));assert!(bot_demand(false,false,true,&[],&["QQQ".into()]));assert!(!bot_demand(false,true,true,&[],&[]));}
 #[test]fn opposition_filters_qqq_entry_but_not_other_symbols_or_management(){let mut q=good();q.call=Some(25.);q.put=Some(75.);let v=json!(q);let mut r=crate::intraday_strategy::Reading{symbol:"QQQ".into(),entry_side:"CALL".into(),eligible:true,management_ready:true,..Default::default()};apply_bot(&v,&mut r,now());assert!(!r.eligible);assert!(r.management_ready);assert_eq!(r.quick_context["qqqComponents"]["call"],25.);r.symbol="NVDA".into();r.eligible=true;apply_bot(&v,&mut r,now());assert!(r.eligible);}
 #[test]fn pending_stale_and_premarket_cannot_change_bot_entry(){for kind in 0..3{let mut q=good();q.call=Some(25.);q.put=Some(75.);match kind{0=>q.available=false,1=>q.at-=3001,_=>q.phase="Premarket".into()}let mut r=crate::intraday_strategy::Reading{symbol:"QQQ".into(),entry_side:"CALL".into(),eligible:true,..Default::default()};apply_bot(&json!(q),&mut r,now());assert!(r.eligible);}}
 #[test]fn journal_records_forward_outcomes_and_marks_late_ticks_missing(){let mut j=Journal::default();let q=good();assert_eq!(j.update(&q,now()).len(),1);let mut after=q.clone();after.at=now()+60000;after.qqq_price_at=Some(after.at);after.qqq_price=Some(q.qqq_price.unwrap()*1.01);let rows=j.update(&after,after.at);assert!(rows.iter().any(|r|r["kind"]=="outcome"&&r["minutes"]==1&&r["quality"]=="complete"&&n(&r["returnBps"]).unwrap()>99.));after.at=now()+300000+10000;after.qqq_price_at=Some(after.at);assert!(j.update(&after,after.at).iter().any(|r|r["minutes"]==5&&r["quality"]=="missing"));}
 #[test]fn twelve_asset_backend_is_bounded_and_does_not_mutate_input(){let mut h=holdings();h.rows=(0..12).map(|i|qqq_components::Holding{symbol:format!("A{}",char::from(b'A'+i)),weight:4.}).collect();let frames=h.rows.iter().map(|r|frame(&r.symbol,now(),true,"2026-10-09")).collect::<Vec<_>>();let before=frames.clone();let start=std::time::Instant::now();for _ in 0..100{assert!(read(&h,&frames,true,now()).available);}let ms=start.elapsed().as_secs_f64()*10.;println!("QQQ canonical backend, 12 assets: {ms:.3} ms/update");assert!(ms<50.);assert_eq!(before,frames);}

 #[test]fn audit_direction_reverses_and_reports_divergence(){
  let t=now();let h=holdings();let bull=read(&h,&[frame("NVDA",t,true,"2026-10-09"),frame("AAPL",t,true,"2026-10-09"),frame("QQQ",t,true,"2026-10-05")],true,t);
  let bear=read(&h,&[frame("NVDA",t,false,"2026-10-09"),frame("AAPL",t,false,"2026-10-09"),frame("QQQ",t,true,"2026-10-05")],true,t);
  assert!(bull.call.unwrap()>=60.);assert!(bear.call.unwrap()<=40.);assert_eq!(bear.put,Some(100.-bear.call.unwrap()));assert_eq!(bear.relation,"Divergencia");assert!(bull.leaders.iter().all(|r|r.contribution>0.));assert!(bear.leaders.iter().all(|r|r.contribution<0.));
 }
 #[test]fn audit_weighting_is_not_equal_vote_and_order_does_not_matter(){
  let t=now();let a=frame("NVDA",t,true,"2026-10-09");let b=frame("AAPL",t,false,"2026-10-09");let h=holdings();
  let r=read(&h,&[a.clone(),b.clone()],true,t);let reversed=read(&h,&[b.clone(),a.clone()],true,t);assert_eq!(r.call,reversed.call);assert!(r.call.unwrap()>60.);
  let mut swapped=h.clone();swapped.rows[0].weight=10.;swapped.rows[1].weight=45.;let Day::Open(day)=calendar::schedule(calendar::date_at(t).unwrap())else{panic!()};let ac=score(&a,"NVDA","Sesión RTH",t,day.open_ms).unwrap().0;let bc=score(&b,"AAPL","Sesión RTH",t,day.open_ms).unwrap().0;let negative=read(&swapped,&[a,b],true,t);assert_eq!(r.call,Some(((45.*ac+10.*bc)/55.).round()));assert_eq!(negative.call,Some(((10.*ac+45.*bc)/55.).round()));assert!(negative.call.unwrap()<50.);assert!(negative.call.unwrap()<r.call.unwrap());
 }
 #[test]fn audit_closed_disconnected_and_expired_weights_have_no_percent(){
  let t=now();let frames=vec![frame("NVDA",t,true,"2026-10-09"),frame("AAPL",t,true,"2026-10-09")];
  assert!(!read(&holdings(),&frames,false,t).available);
  for text in ["2026-10-04T11:00:00-04:00","2026-10-05T16:00:00-04:00","2026-10-05T03:59:00-04:00"]{let report=read(&holdings(),&frames,true,calendar::rfc3339_ms(text).unwrap());assert!(!report.available);assert!(report.call.is_none());assert!(report.put.is_none());}
  let mut stale=holdings();stale.as_of="2026-09-01".into();assert!(!read(&stale,&frames,true,t).available);
 }
 #[test]fn audit_put_opposition_and_partial_options_never_change_management(){
  let mut q=good();q.call=Some(75.);q.put=Some(25.);let mut r=crate::intraday_strategy::Reading{symbol:"QQQ".into(),entry_side:"PUT".into(),eligible:true,management_ready:true,..Default::default()};apply_bot(&json!(q),&mut r,now());assert!(!r.eligible);assert!(r.management_ready);
  q.option_coverage=20.;r.eligible=true;apply_bot(&json!(q),&mut r,now());assert!(r.eligible);assert!(r.management_ready);
 }
}
