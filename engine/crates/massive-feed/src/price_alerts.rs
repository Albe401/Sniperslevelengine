use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Alert {pub id:String,pub symbol:String,pub price:f64,pub direction:String,pub sound:bool,#[serde(default)]pub armed_at:u64,pub fired_at:Option<u64>,pub fired_price:Option<f64>}
#[derive(Clone,Default)]
pub struct Engine {rows:BTreeMap<String,Vec<Alert>>,previous:BTreeMap<String,(f64,u64)>}
impl Engine {
 pub fn configure(&mut self,alerts:Vec<Alert>)->Result<(),String>{
  if alerts.len()>5550{return Err("Demasiadas alarmas".into())}let mut next:BTreeMap<String,Vec<Alert>>=BTreeMap::new();let mut ids=std::collections::BTreeSet::new();
  for mut a in alerts {if crate::price_alerts::supported(&a.symbol)&&a.price.is_finite()&&a.price>0.&&a.id.len()<=80&&!a.id.is_empty()&&ids.insert((a.symbol.clone(),a.id.clone()))&&["up","down","either"].contains(&a.direction.as_str()) {
   let rows=next.entry(a.symbol.clone()).or_default();if rows.len()>=30{return Err("Maximo 30 alarmas por activo".into())}
   if let Some(old)=self.rows.get(&a.symbol).and_then(|r|r.iter().find(|r|r.id==a.id&&r.armed_at==a.armed_at&&r.price==a.price&&r.direction==a.direction)){a.fired_at=old.fired_at;a.fired_price=old.fired_price;}
   rows.push(a);
  }else{return Err("Alarma invalida".into())}}
  for (symbol,rows) in &next {if self.rows.get(symbol).map(|r|r.iter().map(|a|(&a.id,a.armed_at)).collect::<Vec<_>>())!=Some(rows.iter().map(|a|(&a.id,a.armed_at)).collect()){self.previous.remove(symbol);}}
  self.previous.retain(|s,_|next.contains_key(s));self.rows=next;Ok(())
 }
 pub fn reset(&mut self){self.previous.clear();}
 pub fn snapshot(&self)->Vec<Alert>{self.rows.values().flatten().cloned().collect()}
 pub fn trade(&mut self,symbol:&str,price:f64,at:u64,now:u64){
  let Some(rows)=self.rows.get_mut(symbol) else{return};
  if !price.is_finite()||price<=0.||at>now.saturating_add(1000)||now.saturating_sub(at)>15000{return}
  if let Some((old,t))=self.previous.get(symbol).copied(){if at<=t{return}if at-t<=15000 {for a in rows {if a.fired_at.is_none()&&((a.direction!="down"&&old<a.price&&price>=a.price)||(a.direction!="up"&&old>a.price&&price<=a.price)){a.fired_at=Some(at);a.fired_price=Some(price);}}}}
  self.previous.insert(symbol.into(),(price,at));
 }
}
fn supported(symbol:&str)->bool {snipers_core::universe::stock_asset(symbol).is_some()}
#[cfg(test)]mod tests {use super::*;
 fn alert(symbol:&str)->Alert{Alert{id:symbol.into(),symbol:symbol.into(),price:100.,direction:"up".into(),sound:false,armed_at:1,fired_at:None,fired_price:None}}
 #[test]fn background_cross_survives_return_before_ui_snapshot(){let mut e=Engine::default();e.configure(vec![alert("AAPL"),alert("QQQ")]).unwrap();e.trade("AAPL",99.,10000,10000);e.trade("QQQ",98.,10001,10001);e.trade("AAPL",101.,10002,10002);e.trade("AAPL",99.,10003,10003);let a=e.snapshot();assert_eq!(a[0].fired_at,Some(10002));assert_eq!(a[1].fired_at,None);}
 #[test]fn stale_reordered_gaps_and_reconnect_dont_fire(){let mut e=Engine::default();e.configure(vec![alert("AAPL")]).unwrap();e.trade("AAPL",99.,10000,10000);e.trade("AAPL",101.,9999,10000);e.trade("AAPL",101.,10001,50000);e.trade("AAPL",101.,50000,50000);assert!(e.snapshot()[0].fired_at.is_none());e.reset();e.trade("AAPL",99.,50001,50001);assert!(e.snapshot()[0].fired_at.is_none());}
 #[test]fn config_preserves_fired_until_explicit_rearm(){let mut e=Engine::default();e.configure(vec![alert("AAPL")]).unwrap();e.trade("AAPL",99.,10000,10000);e.trade("AAPL",101.,10001,10001);e.configure(vec![alert("AAPL")]).unwrap();assert!(e.snapshot()[0].fired_at.is_some());let mut a=alert("AAPL");a.armed_at=2;e.configure(vec![a]).unwrap();assert!(e.snapshot()[0].fired_at.is_none());}
 #[test]fn validates_all_rows_before_replacing(){let mut e=Engine::default();e.configure(vec![alert("QQQ")]).unwrap();let mut bad=alert("AAPL");bad.price=f64::NAN;assert!(e.configure(vec![bad]).is_err());assert_eq!(e.snapshot()[0].symbol,"QQQ");}
}
