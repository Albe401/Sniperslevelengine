//! Frozen, symbol-specific descriptive evidence available before entry evaluation.
use serde_json::{json, Value};
use std::sync::OnceLock;
pub fn reference() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../../../../data/price-route-history/bot-reference.json")).expect("validated asset history"))
}
pub fn asset(symbol: &str, now: u64) -> Option<&'static Value> {
    let r=reference();
    if now < r["availableAfterMs"].as_u64()? { return None; }
    r["assets"].get(symbol).filter(|a| a["summary"]["symbol"]==symbol)
}
pub fn pattern(symbol: &str, frame: u64, level: &str, side: f64, now: u64) -> Value {
    asset(symbol,now).and_then(|a|a["patterns"].as_array()).and_then(|rows| rows.iter().find(|p|p["symbol"]==symbol&&p["frame"]==frame&&p["levelName"]==level&&p["side"]==side)).cloned().unwrap_or(Value::Null)
}
pub fn attach(r: &mut massive_feed::intraday_strategy::Reading, now: u64) {
    let a=asset(&r.symbol,now);
    let scenario=&r.quick_context["priceRoute"]["fourHour"]["scenario"];
    let matched:Vec<Value>=a.and_then(|a|a["fourHour"]["summaries"].as_array()).into_iter().flatten().filter(|row| {
        if scenario["symbol"]!=r.symbol||!scenario["at"].as_u64().is_some_and(|t|t<=now){return false;}
        let suffix=format!(":{}:{}:{}:{}",scenario["setup"].as_str().unwrap_or(""),scenario["side"],scenario["blockMinutes"],scenario["targetSource"].as_str().unwrap_or(""));
        row["key"].as_str().is_some_and(|k|k.ends_with(&suffix))
    }).cloned().collect();
    r.quick_context["assetHistory"]=json!({"version":"bot-asset-history-v1","symbol":r.symbol,"at":now,"state":if a.is_some(){"available"}else if r.symbol=="SPX"{"provider_access_denied"}else{"history_unavailable_at_evaluation"},"summary":a.map(|a|&a["summary"]),"referenceSha256":reference()["sourceSha256"],"period":{"from":reference()["from"],"to":reference()["to"]},"fourHourMatchedCohorts":matched,"fourHourState":if matched.is_empty(){"pending_matching_scenario"}else{"descriptive"},"probabilityClaim":false,"changesEntryWeights":false,"basis":"own-symbol-study-no-cross-asset-substitution"});
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn all_assets_have_own_complete_pattern_keys(){let now=reference()["availableAfterMs"].as_u64().unwrap();assert_eq!(reference()["assets"].as_object().unwrap().len(),186);for(symbol,a)in reference()["assets"].as_object().unwrap(){assert_eq!(a["patterns"].as_array().unwrap().len(),30);assert!(pattern(symbol,3,"e200",-1.,now).is_object());assert_eq!(a["summary"]["symbol"],*symbol);}}
 #[test]fn future_and_unavailable_history_are_never_substituted(){let at=reference()["availableAfterMs"].as_u64().unwrap();assert!(asset("NVDA",at-1).is_none());assert!(asset("SPX",at).is_none());assert!(asset("UNKNOWN",at).is_none());assert_eq!(pattern("AAPL",1,"e9",1.,at)["symbol"],"AAPL");}
 #[test]fn attachment_preserves_admission_and_matches_own_scenario(){let mut r=massive_feed::intraday_strategy::Reading{symbol:"NVDA".into(),eligible:true,quick_context:json!({"priceRoute":{"fourHour":{"scenario":{"symbol":"NVDA","at":1,"setup":"breakdown","side":-1,"blockMinutes":240,"targetSource":"known-pivot"}}}}),..Default::default()};attach(&mut r,reference()["availableAfterMs"].as_u64().unwrap());assert!(r.eligible);assert!(!r.quick_context["assetHistory"]["fourHourMatchedCohorts"].as_array().unwrap().is_empty());r.quick_context["priceRoute"]["fourHour"]["scenario"]["symbol"]=json!("SPY");attach(&mut r,reference()["availableAfterMs"].as_u64().unwrap());assert!(r.quick_context["assetHistory"]["fourHourMatchedCohorts"].as_array().unwrap().is_empty());}
}
