//! Diagnostic only: does not authorize orders or enlarge risk limits.
use serde_json::{json,Value};
fn positive(v:&Value)->Option<f64>{v.as_f64().filter(|n|n.is_finite()&&*n>0.)}
pub fn assess(v:&Value,side:&str,underlying:&str,anchor:Option<f64>,target:Option<f64>,now:u64)->Value{
 let r=&v["reading"];let q=&r["quick_context"];
 let sign=match side{"CALL"=>1.,"PUT"=>-1.,_=>return json!({"status":"pending"})};
 let fresh=|t:&Value|t.as_u64().is_some_and(|t|t<=now&&now-t<=5000);
 let current=r["symbol"]==underlying&&fresh(&v["reviewedMs"])&&(r["management_ready"]==true||r["ready"]==true);
 let context=current&&q["symbol"]==underlying&&fresh(&q["at"]);
 let spot=positive(&r["spot"]);
 let defended=anchor.filter(|a|a.is_finite()&&*a>0.).zip(spot).map(|(a,p)|(p-a)*sign>=0.);
 let room=target.filter(|t|t.is_finite()&&*t>0.).zip(spot).map(|(t,p)|(t-p)*sign);
 let cvd=if context{match q["cvd"]["side"].as_str(){Some(s) if s==side=>"supports",Some("CALL"|"PUT")=>"opposes",_=>"pending"}}else{"pending"};
 let flow:Vec<Option<f64>>=["flow_one","flow_five"].iter().map(|k|r[k].as_f64().filter(|n|n.is_finite()).map(|n|n*sign)).collect();
 let flow_support=current&&flow.iter().all(|n|n.is_some_and(|n|n>0.));
 let flow_opposes=current&&flow[0].is_some_and(|n|n<=-0.15)&&flow[1].is_some_and(|n|n<0.);
 let atr=if context{positive(&q["atr5m"])}else{None};
 let status=if !current||defended.is_none()||room.is_none(){"pending"}else if room.is_some_and(|n|n<=0.){"target_reached"}else if defended==Some(false){"level_lost"}else if flow_opposes||cvd=="opposes"{"weakened"}else if flow_support||cvd=="supports"{"supported"}else{"unconfirmed"};
 json!({"status":status,"levelHeld":if current{defended}else{None},"entryTarget":target,"remainingDollars":if current{room}else{None},"remainingAtr":room.zip(atr).filter(|_|current).map(|(d,a)|d/a),"cvd":cvd,"flowSupports":flow_support,"flowOpposes":flow_opposes,"structure":if context{q["alignment"].clone()}else{json!("pending")},"atrIsDirection":false,"absorption":if context{q["absorption"].clone()}else{json!("pending")}})
}
#[cfg(test)]mod tests{
 use super::*;
 fn sample(now:u64,side:&str)->Value{let sign=if side=="CALL"{1.}else{-1.};json!({"reviewedMs":now,"reading":{"symbol":"QQQ","management_ready":true,"spot":100.,"flow_one":sign*0.2,"flow_five":sign*0.3,"quick_context":{"symbol":"QQQ","at":now,"cvd":{"side":side},"atr5m":2.,"alignment":side}}})}
 #[test]fn symmetric_support_does_not_use_premium_loss_as_invalidation(){for side in ["CALL","PUT"]{let sign=if side=="CALL"{1.}else{-1.};let mut v=sample(10000,side);v["holding"]=json!({"entry_premium":5.,"bid":1.});let r=assess(&v,side,"QQQ",Some(100.-sign*2.),Some(100.+sign*6.),10000);assert_eq!(r["status"],"supported");assert_eq!(r["remainingAtr"],3.);}}
 #[test]fn missing_stale_foreign_context_never_confirms_hold(){let v=sample(10000,"CALL");assert_eq!(assess(&v,"CALL","QQQ",Some(98.),Some(106.),15001)["status"],"pending");assert_eq!(assess(&v,"CALL","SPY",Some(98.),Some(106.),10000)["status"],"pending");assert_eq!(assess(&v,"CALL","QQQ",None,Some(106.),10000)["status"],"pending");}
 #[test]fn opposed_cvd_weakens_despite_structural_call_alignment(){let mut v=sample(10000,"CALL");v["reading"]["quick_context"]["cvd"]["side"]=json!("PUT");assert_eq!(assess(&v,"CALL","QQQ",Some(98.),Some(106.),10000)["status"],"weakened");v["reading"]["spot"]=json!(97.);assert_eq!(assess(&v,"CALL","QQQ",Some(98.),Some(106.),10000)["status"],"level_lost");}
 #[test]fn benchmark(){let v=sample(10000,"CALL");let start=std::time::Instant::now();for _ in 0..10000{assess(&v,"CALL","QQQ",Some(98.),Some(106.),10000);}let ms=start.elapsed().as_secs_f64()*1000./10000.;println!("Position thesis: {ms:.4} ms/evaluation");assert!(ms<5.);}
}

