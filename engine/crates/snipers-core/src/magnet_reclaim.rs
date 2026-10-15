//! Prospective pattern observation. Never authorizes an order or uses future bars.
use serde::{Deserialize,Serialize};
#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct Bar {pub start_ms:u64,pub open:f64,pub high:f64,pub low:f64,pub close:f64}
#[derive(Clone,Debug,Default,Deserialize,Serialize)]
pub struct Read {
    pub detected:bool,pub entry_authorized:bool,pub reason:String,
    pub at:u64,pub magnet:Option<f64>,pub target:Option<f64>,pub reference:Option<f64>,
    pub invalidation:Option<f64>,pub reward_risk:Option<f64>,
}
pub fn evaluate(bars:&[Bar],magnet:Option<f64>,target:Option<f64>,now:u64)->Read {
    evaluate_side(bars,magnet,target,now,true)
}
/// Symmetric closed-candle evidence only. Execution authorization belongs to the Paper controller.
pub fn evaluate_side(bars:&[Bar],magnet:Option<f64>,target:Option<f64>,now:u64,call:bool)->Read {
    let direction=if call {1.} else {-1.};
    let mut r=Read{at:now,reason:"Esperando recuperación y dos cierres sobre el imán".into(),..Default::default()};
    if !call {r.reason="Esperando rechazo y dos cierres bajo el imán".into();}
    let Some((m,t))=magnet.zip(target).filter(|(m,t)|m.is_finite()&&t.is_finite()&&*m>0.0&&*t>0.0&&(t-m)*direction>0.) else{return r};
    r.magnet=Some(m);r.target=Some(t);
    if bars.len()<3{return r;}
    let b=&bars[bars.len()-3..];
    if b.iter().any(|v|v.start_ms>now||now-v.start_ms<120_000||[v.open,v.high,v.low,v.close].iter().any(|p|!p.is_finite()||*p<=0.0)
        ||v.low>v.open.min(v.close)||v.high<v.open.max(v.close))
        ||b.windows(2).any(|w|w[1].start_ms.checked_sub(w[0].start_ms)!=Some(120_000))
        ||now-b[2].start_ms>180_000 {r.reason="Velas incompletas, discontinuas o antiguas".into();return r;}
    let touched=b[..2].iter().any(|v|v.low<=m&&v.high>=m);
    if !touched||(b[1].close-m)*direction<=0.||(b[1].close-b[1].open)*direction<=0.||(b[2].close-m)*direction<=0.{return r;}
    let stop=if call {b.iter().map(|v|v.low).fold(f64::INFINITY,f64::min)}else{b.iter().map(|v|v.high).fold(f64::NEG_INFINITY,f64::max)};
    let reference=b[2].close;
    if (t-reference)*direction<=0.||(reference-stop)*direction<=0. {r.reason="Sin recorrido restante hacia el muro".into();return r;}
    let rr=(t-reference)/(reference-stop);
    r.reference=Some(reference);r.invalidation=Some(stop);r.reward_risk=Some(rr);
    r.detected=rr>=1.8;
    r.reason=if r.detected {if call {"CALL: rebote del imán confirmado en dos cierres"}else{"PUT: rechazo del imán confirmado en dos cierres"}}else{"Recorrido inferior a 1,8 veces la invalidación"}.into();
    r
}
#[cfg(test)]
mod tests {
    use super::*;
    fn bars()->Vec<Bar>{vec![
        Bar{start_ms:0,open:736.95,high:737.2218,low:736.26,close:736.37},
        Bar{start_ms:120000,open:736.37,high:737.15,low:736.275,close:737.1499},
        Bar{start_ms:240000,open:737.07,high:737.55,low:736.87,close:737.07}]}
    #[test] fn observed_reclaim_requires_closed_confirmation_and_keeps_orders_disabled(){
        let r=evaluate(&bars(),Some(737.),Some(740.),360000);
        assert!(r.detected);assert!(!r.entry_authorized);assert!((r.reward_risk.unwrap()-3.61728).abs()<0.001);
        assert!(!evaluate(&bars(),Some(737.),Some(740.),359999).detected);
        assert!(!evaluate(&bars(),Some(737.),Some(740.),420001).detected);
        let mut b=bars();b[2].close=736.99;assert!(!evaluate(&b,Some(737.),Some(740.),360000).detected);
        b=bars();b[2].start_ms+=1000;assert!(!evaluate(&b,Some(737.),Some(740.),361000).detected);
        assert!(!evaluate(&bars(),None,Some(740.),360000).detected);
        assert!(!evaluate(&bars(),Some(737.),Some(737.5),360000).detected);
    }
}
