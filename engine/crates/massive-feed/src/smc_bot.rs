//! SMC is an additional admission confirmation; it never creates an entry or moves risk levels.
use serde_json::{json,Value};
pub const VERSION:&str="smc-confirmation-v1";
pub fn apply(context:Value,r:&mut crate::intraday_strategy::Reading,now:u64){
 let baseline=r.eligible;
 let review=crate::smc::advisory(&context,&serde_json::to_value(&*r).unwrap_or(Value::Null),None,None,now);
 if baseline&&matches!(review["entryAssessment"].as_str(),Some("opposes"|"mixed")){
  r.eligible=false;r.blockers.push("SMC: retesteo confirmado contrario o conflicto entre marcos; esperar nueva confirmación".into());
 }
 let mut context=context;
 // The existing FVG context remains authoritative; avoid duplicating the complete zone list over IPC.
 context["fvg"]=json!({"available":context["fvg"]["available"],"selected":context["fvg"]["selected"]});
 context["strategyConnected"]=json!(true);
 r.quick_context["smc"]=json!({"version":VERSION,"symbol":r.symbol,"at":now,"mode":"additional-confirmation","context":context,"review":review,"baselineEligible":baseline,"finalEligible":r.eligible,"strategyConnected":true,"changesRiskLevels":false});
}
#[cfg(test)]mod tests{
 use super::*;
 fn context(now:u64,side:i8)->Value{json!({"version":crate::smc::VERSION,"symbol":"QQQ","at":now,"state":"observing","layers":[{"timeframe":2,"setups":[{"side":side,"state":"CONFIRMED","retestAt":now}]}]})}
 #[test]fn support_never_creates_entry_and_opposition_blocks_only_admission(){for(side,baseline,expected)in [(1,false,false),(1,true,true),(-1,true,false)]{let now=100000;let mut r=crate::intraday_strategy::Reading{symbol:"QQQ".into(),at:now,entry_side:"CALL".into(),eligible:baseline,management_ready:true,next:Some(105.),quick_context:json!({"fvg":{"pathBlocked":true}}),..Default::default()};apply(context(now,side),&mut r,now);assert_eq!(r.eligible,expected);assert!(r.management_ready);assert_eq!(r.next,Some(105.));assert_eq!(r.quick_context["fvg"]["pathBlocked"],true);}}
 #[test]fn stale_and_foreign_evidence_do_not_override_baseline(){for foreign in [false,true]{let now=100000;let mut c=context(now, -1);if foreign{c["symbol"]=json!("SPY");}else{c["at"]=json!(now-3001);}let mut r=crate::intraday_strategy::Reading{symbol:"QQQ".into(),at:now,entry_side:"CALL".into(),eligible:true,..Default::default()};apply(c,&mut r,now);assert!(r.eligible);}}
}
