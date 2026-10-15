//! Descriptive quote-test estimates for large TRF prints, never investor identity.
//! Massive trade/quote timestamps and conditions are distinct from FINRA side reports.
use crate::{calendar,off_exchange::{Quote,MIN_NOTIONAL,QUOTE_MAX_AGE_MS},off_exchange_trade::Trade};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone,Default,Debug,Serialize)]
pub struct Period {
    pub count:usize,pub total:f64,pub buy:f64,pub sell:f64,pub unknown:f64,
    pub buy_pct:Option<f64>,pub sell_pct:Option<f64>,pub coverage_pct:Option<f64>,
    pub dominant:String,pub complete:bool,pub finalized:bool,pub through_ms:u64,
    pub top_levels:Vec<Zone>,
}
#[derive(Clone,Default,Debug,Serialize)]
pub struct Zone {pub price:f64,pub total:f64,pub buy:f64,pub sell:f64,pub count:usize}
#[derive(Clone,Default,Debug,Serialize)]
pub struct Report {
    pub symbol:String,pub date:String,pub minimum_notional:f64,
    pub premarket:Period,pub regular:Period,pub excluded_time:usize,
    #[serde(skip)] pub quote_requests:Vec<u64>,
}
pub fn execution(t:&Trade)->Option<u64>{
    t.participant_ms.filter(|ms|*ms<=t.sip_ms)
}
// First release deliberately excludes average-price, prior-reference and late modifiers.
pub fn conditions_eligible(t:&Trade)->bool{
    let regular=execution(t).and_then(calendar::date_at).is_some_and(|date|
        matches!(calendar::schedule(date),calendar::Day::Open(day) if execution(t).is_some_and(|at|at>=day.open_ms&&at<day.close_ms)));
    t.conditions.iter().all(|c|matches!(c,0|12|37))&&(regular||t.conditions.contains(&12))
}
pub fn quote_side(t:&Trade,q:Option<Quote>)->i8{
    let Some((at,q))=execution(t).zip(q) else{return 0};
    if !conditions_eligible(t)||t.correction.unwrap_or(0)!=0||!q.bid.is_finite()||!q.ask.is_finite()
        ||q.bid<=0.||q.ask<=q.bid||q.timestamp_ms>at||at-q.timestamp_ms>QUOTE_MAX_AGE_MS
        ||(q.ask-q.bid)/q.ask>0.01 {return 0;}
    let mid=q.bid+(q.ask-q.bid)/2.;let epsilon=((q.ask-q.bid)*0.05).min(0.001);
    if t.price>mid&&(t.price-q.ask).abs()<=epsilon {1}
    else if t.price<mid&&(t.price-q.bid).abs()<=epsilon {-1}else{0}
}
pub fn build<'a>(symbol:&str,now:u64,complete:bool,rows:impl Iterator<Item=(&'a Trade,Option<Quote>)>)->Report{
    let mut out=Report{symbol:symbol.into(),minimum_notional:MIN_NOTIONAL,..Default::default()};
    let Some(date)=calendar::date_at(now) else{return out};out.date=date.to_string();
    let calendar::Day::Open(day)=calendar::schedule(date) else{return out};
    let Some(start)=calendar::legacy_eastern_ms(&format!("{} 04:00:00",out.date)) else{return out};
    let mut zones:[BTreeMap<u64,Zone>;2]=Default::default();
    out.premarket.complete=complete;out.regular.complete=complete;
    out.premarket.finalized=now>=day.open_ms;out.regular.finalized=now>=day.close_ms;
    out.premarket.through_ms=now.min(day.open_ms);out.regular.through_ms=now.min(day.close_ms);
    for (t,q) in rows{
        let amount=t.price*t.shares;
        if t.symbol!=symbol||t.exchange!=4||!t.price.is_finite()||t.price<=0.||!t.shares.is_finite()||t.shares<=0.||!amount.is_finite()||amount<MIN_NOTIONAL||t.sip_ms>now||t.correction.unwrap_or(0)!=0 {continue;}
        let Some(at)=execution(t) else{out.excluded_time+=1;continue};
        let i=if at>=start&&at<day.open_ms{0}else if at>=day.open_ms&&at<day.close_ms{1}else{continue};
        let p=if i==0{&mut out.premarket}else{&mut out.regular};
        p.count+=1;p.total+=amount;
        let side=quote_side(t,q);
        match side{1=>p.buy+=amount,-1=>p.sell+=amount,_=>p.unknown+=amount}
        if q.is_none()&&conditions_eligible(t){out.quote_requests.push(at);}
        if let Some(level)=crate::off_exchange::bucket(t.price){
            let z=zones[i].entry(level.to_bits()).or_insert_with(||Zone{price:level,..Default::default()});
            z.total+=amount;z.count+=1;if side==1{z.buy+=amount}else if side==-1{z.sell+=amount}
        }
    }
    out.quote_requests.sort_unstable();out.quote_requests.dedup();
    for (p,z) in [&mut out.premarket,&mut out.regular].into_iter().zip(zones){
        p.complete &= out.excluded_time==0;
        let classified=p.buy+p.sell;
        if ![p.total,p.buy,p.sell,p.unknown,classified].iter().all(|n|n.is_finite()){
            p.complete=false;p.dominant="INSUFFICIENT".into();continue;
        }
        if p.total>0. {p.coverage_pct=Some(classified/p.total*100.);}
        if classified>0. {p.buy_pct=Some(p.buy/classified*100.);p.sell_pct=Some(p.sell/classified*100.);}
        // Unknown prints must not be able to overturn a claimed lead.
        p.dominant=if !p.complete||classified==0.||(p.buy-p.sell).abs()<=p.unknown{"INSUFFICIENT"}
            else if p.buy>p.sell{"BUY"}else{"SELL"}.into();
        p.top_levels=z.into_values().collect();p.top_levels.sort_by(|a,b|b.total.total_cmp(&a.total));p.top_levels.truncate(3);
    }
    out
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::off_exchange_trade::Source;
    fn ms(s:&str)->u64{calendar::rfc3339_ms(s).unwrap()}
    fn trade(price:f64,amount:f64)->Trade{
        let at=ms("2026-10-01T13:00:00Z");
        Trade{symbol:"QQQ".into(),price,shares:amount/price,sip_ms:at+10,participant_ms:Some(at),trf_ms:Some(at+5),exchange:4,trf_id:12,trade_id:Some("one".into()),sequence:Some(1),conditions:vec![12],tape:Some(3),correction:None,source:Source::History}
    }
    fn q(at:u64)->Quote{Quote{bid:99.98,ask:100.,timestamp_ms:at}}
    #[test]fn quote_test_never_uses_future_stale_locked_wide_midpoint_or_late_conditions(){
        let mut t=trade(100.,200_000.);let at=execution(&t).unwrap();
        assert_eq!(quote_side(&t,Some(q(at))),1);
        t.price=99.98;assert_eq!(quote_side(&t,Some(q(at))),-1);
        t.price=99.99;assert_eq!(quote_side(&t,Some(q(at))),0);
        t.price=100.;
        for quote in [None,Some(q(at+1)),Some(q(at-2001)),Some(Quote{bid:100.,..q(at)}),Some(Quote{bid:98.,..q(at)})]{assert_eq!(quote_side(&t,quote),0);}
        t.conditions=vec![13];assert_eq!(quote_side(&t,Some(q(at))),0);
        t.conditions=vec![12];t.participant_ms=None;assert_eq!(quote_side(&t,Some(q(at))),0);
    }
    #[test]fn denominator_and_unknown_importance_are_explicit_and_periods_do_not_mix(){
        let buy=trade(100.,200_000.);let sell=trade(99.98,100_000.);let unknown=trade(99.99,150_000.);
        let at=execution(&buy).unwrap();let now=ms("2026-10-01T14:00:00Z");
        let report=build("QQQ",now,true,[(&buy,Some(q(at))),(&sell,Some(q(at))),(&unknown,Some(q(at)))].into_iter());
        assert_eq!(report.premarket.count,3);assert_eq!(report.regular.count,0);
        assert!((report.premarket.buy_pct.unwrap()-66.6666667).abs()<0.001);
        assert!((report.premarket.coverage_pct.unwrap()-66.6666667).abs()<0.001);
        assert_eq!(report.premarket.dominant,"INSUFFICIENT");assert!(report.premarket.finalized);
        let r=build("QQQ",now,true,[(&buy,Some(q(at))),(&sell,Some(q(at)))].into_iter());assert_eq!(r.premarket.dominant,"BUY");
        let r=build("QQQ",now,false,[(&buy,Some(q(at)))].into_iter());assert_eq!(r.premarket.dominant,"INSUFFICIENT");
    }
    #[test]fn report_time_cannot_move_an_overnight_print_into_premarket_and_no_data_is_not_fifty_fifty(){
        let mut t=trade(100.,200_000.);t.participant_ms=Some(ms("2026-10-01T07:00:00Z"));
        let now=ms("2026-10-01T14:00:00Z");
        let r=build("QQQ",now,true,[(&t,None)].into_iter());assert_eq!(r.premarket.count,0);assert_eq!(r.premarket.buy_pct,None);
        t.participant_ms=None;let r=build("QQQ",now,true,[(&t,None)].into_iter());assert_eq!(r.excluded_time,1);assert_eq!(r.premarket.count,0);
        t=trade(100.,99_999.);let r=build("QQQ",now,true,[(&t,None)].into_iter());assert_eq!(r.premarket.count,0);
    }
    #[test]fn absent_trade_modifiers_are_usable_in_regular_hours_but_not_a_premarket_execution_proof(){
        let mut t=trade(100.,200_000.);t.conditions.clear();assert!(!conditions_eligible(&t));
        let at=ms("2026-10-01T14:00:00Z");t.participant_ms=Some(at);t.sip_ms=at+1;
        assert!(conditions_eligible(&t));assert_eq!(quote_side(&t,Some(q(at))),1);
        t.conditions=vec![0];assert_eq!(quote_side(&t,Some(q(at))),1);
        t.price=99.98;assert_eq!(quote_side(&t,Some(q(at))),-1);
        t.conditions=vec![10,37,41];assert_eq!(quote_side(&t,Some(q(at))),0);
    }
}
