//! Shared one-day magnitude context; never an entry vote or a hard price ceiling.
use serde::Serialize;
use serde_json::Value;
#[derive(Clone,Default,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Room {
 pub symbol:String,pub at:u64,pub available:bool,pub anchored:bool,
 pub expected_move_dollars:Option<f64>,pub expected_move_pct:Option<f64>,
 pub reference:Option<f64>,pub lower:Option<f64>,pub upper:Option<f64>,
 pub call_dollars:Option<f64>,pub put_dollars:Option<f64>,pub call_pct:Option<f64>,pub put_pct:Option<f64>,
 pub extension_call:Option<f64>,pub extension_put:Option<f64>,pub reason:String,
}
pub fn calculate(symbol:&str,model:&Value,observed:Option<u64>,spot:f64,price_at:Option<u64>,minutes:&Value,now:u64)->Room{
 let mut r=Room{symbol:symbol.into(),at:now,reason:"Rango o precio vigente pendiente".into(),..Default::default()};
 let number=|key:&str|model[key].as_f64().filter(|v|v.is_finite()&&*v>0.);
 let Some(date)=snipers_core::calendar::date_at(now)else{return r};
 let fresh=|t:u64,age:u64|t<=now&&now-t<=age&&snipers_core::calendar::date_at(t)==Some(date);
 if model["ready"]!=true||!observed.is_some_and(|t|fresh(t,900000))||!price_at.is_some_and(|t|fresh(t,120000))||!spot.is_finite()||spot<=0.{return r;}
 let (Some(reference),Some(movement),Some(pct))=(number("spot"),number("expectedMoveDollars"),number("expectedMovePct"))else{return r};
 if (pct-100.*movement/reference).abs()>0.01{return r;}
 r.available=true;r.expected_move_dollars=Some(movement);r.expected_move_pct=Some(pct);
 let session=match snipers_core::calendar::schedule(date){snipers_core::calendar::Day::Open(s)=>Some(s),_=>None};
 let opening=session.filter(|s|now>=s.open_ms+60000&&now<s.close_ms).and_then(|s|{
  let rows:Vec<_>=minutes.as_array()?.iter().filter(|b|b["timestampMs"].as_u64()==Some(s.open_ms)).collect();
  if rows.len()!=1{return None;}let b=rows[0];let (o,h,l,c)=(b["open"].as_f64()?,b["high"].as_f64()?,b["low"].as_f64()?,b["close"].as_f64()?);
  ([o,h,l,c].iter().all(|v|v.is_finite()&&*v>0.)&&h>=o.max(c)&&l<=o.min(c)&&h>=l).then_some(o)
 });
 r.reference=Some(opening.unwrap_or(reference));r.lower=r.reference.map(|v|v-movement);r.upper=r.reference.map(|v|v+movement);
 if r.lower.is_some_and(|v|!v.is_finite()||v<=0.)||r.upper.is_some_and(|v|!v.is_finite()){return Room{symbol:symbol.into(),at:now,reason:"Banda inválida".into(),..Default::default()};}
 r.anchored=opening.is_some();
 if r.anchored{
  let call=r.upper.unwrap()-spot;let put=spot-r.lower.unwrap();
  r.call_dollars=Some(call.max(0.));r.put_dollars=Some(put.max(0.));r.call_pct=Some(call.max(0.)/spot*100.);r.put_pct=Some(put.max(0.)/spot*100.);
  r.extension_call=Some((-call).max(0.));r.extension_put=Some((-put).max(0.));r.reason="Recorrido restante frente a apertura RTH; estimación, no límite ni entrada".into();
 }else{r.reason="Rango diario del snapshot; recorrido restante pendiente de apertura RTH completa".into();}
 r
}
#[cfg(test)]mod tests{
 use super::*;use serde_json::json;
 #[test]fn asymmetric_room_and_extension_do_not_manufacture_reversals(){
  let open=snipers_core::calendar::rfc3339_ms("2026-10-02T09:30:00-04:00").unwrap();let now=open+60000;
  let model=json!({"ready":true,"spot":100.,"expectedMoveDollars":10.,"expectedMovePct":10.});let bars=json!([{"timestampMs":open,"open":100.,"high":104.,"low":99.,"close":103.}]);
  let r=calculate("AAPL",&model,Some(now),103.,Some(now),&bars,now);assert!(r.anchored);assert_eq!(r.call_dollars,Some(7.));assert_eq!(r.put_dollars,Some(13.));
  let e=calculate("AAPL",&model,Some(now),112.,Some(now),&bars,now);assert_eq!(e.call_dollars,Some(0.));assert_eq!(e.extension_call,Some(2.));assert_eq!(e.put_dollars,Some(22.));
  for (t,p) in [(now+1,now),(now-900001,now),(now,now-120001)]{assert!(!calculate("AAPL",&model,Some(t),103.,Some(p),&bars,now).available);}
  let missing=calculate("AAPL",&model,Some(now),103.,Some(now),&json!([]),now);assert!(missing.available);assert!(!missing.anchored);assert_eq!(missing.call_dollars,None);
  let duplicate=json!([bars[0],bars[0]]);assert!(!calculate("AAPL",&model,Some(now),103.,Some(now),&duplicate,now).anchored);
 }
}
