//! Versioned consolidated-volume rules, separate from OHLC and aggressor inference.
//! Provider snapshot plus explicitly sampled empty-condition convention; never order logic.
use std::{collections::BTreeMap,sync::OnceLock};
pub const ID:&str="stocks-consolidated-volume-20261013-v1";
pub const OBSERVED_ON:&str="2026-10-13";
#[derive(Clone,Copy,Debug,PartialEq)]pub enum Decision{Eligible,Excluded,Pending}
fn rules()->&'static BTreeMap<u32,Option<bool>>{
 static RULES:OnceLock<BTreeMap<u32,Option<bool>>>=OnceLock::new();
 RULES.get_or_init(||{
  let value:serde_json::Value=serde_json::from_str(include_str!("../../../fixtures/massive-stock-volume-rules-20261013.json")).expect("Versioned tested volume reference JSON");
  value["results"].as_array().unwrap().iter().filter_map(|r|Some((u32::try_from(r["id"].as_u64()?).ok()?,r["update_rules"]["consolidated"]["updates_volume"].as_bool()))).collect()
 })
}
pub fn condition(code:u32)->Decision{
 // Zero regular sale is separately documented by provider glossary; not emitted in API snapshot.
 if code==0{return Decision::Eligible;}
 match rules().get(&code){Some(Some(true))=>Decision::Eligible,Some(Some(false))=>Decision::Excluded,_=>Decision::Pending}
}
pub fn volume(conditions:&[u32])->Decision{
 // Empty conditions preserve original encoding; sampled full-minute REST comparisons support eligibility.
 if conditions.iter().any(|&c|condition(c)==Decision::Excluded){return Decision::Excluded;}
 if conditions.iter().any(|&c|condition(c)==Decision::Pending){return Decision::Pending;}
 Decision::Eligible
}
pub fn permits_volume(conditions:&[u32])->bool{volume(conditions)==Decision::Eligible}
pub fn volume_at(conditions:&[u32],at:u64)->Decision{
 if !conditions.iter().any(|c|matches!(c,12|13)){return volume(conditions);}
 let in_regular=crate::calendar::date_at(at).is_some_and(|d|matches!(crate::calendar::schedule(d),crate::calendar::Day::Open(s) if at>=s.open_ms&&at<s.close_ms));
 // T/U identify outside-hours execution even when reported during RTH. Do not reclassify its volume as RTH.
 if in_regular&&conditions.iter().any(|c|matches!(c,12|13)){return Decision::Excluded;}
 volume(conditions)
}
pub fn permits_volume_at(conditions:&[u32],at:u64)->bool{volume_at(conditions,at)==Decision::Eligible}
pub fn metadata()->serde_json::Value{serde_json::json!({"id":ID,"unit":"shares","rulesObservedOn":OBSERVED_ON,"scope":"consolidated_volume_session_specific","outsideHoursModifiersInRth":"12_13_excluded_from_RTH","emptyConditions":"eligible_empirical_full_minute_reference","unknownRules":"pending_excluded_from_calculation","directionClassification":"separate_inference","historicalCorrections":"REST_review_separate_from_observed_stream"})}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn volume_is_not_price_or_direction(){for c in [0,2,10,12,14,37,41,52,53]{assert_eq!(condition(c),Decision::Eligible);}assert_eq!(condition(15),Decision::Excluded);assert_eq!(condition(38),Decision::Excluded);}
 #[test]fn explicit_veto_wins_and_unknown_never_defaults_to_volume(){assert_eq!(volume(&[14,37,41]),Decision::Eligible);assert_eq!(volume(&[37,15,41]),Decision::Excluded);assert_eq!(volume(&[999]),Decision::Pending);assert_eq!(volume(&[57]),Decision::Pending);assert_eq!(volume(&[]),Decision::Eligible);assert_eq!(volume(&[999,15]),Decision::Excluded);}
 #[test]fn extended_execution_reported_at_open_is_not_regular_volume(){let p=crate::calendar::legacy_eastern_ms("2026-10-12 08:00:00").unwrap();let r=crate::calendar::legacy_eastern_ms("2026-10-12 09:30:00").unwrap();assert!(permits_volume_at(&[12,37],p));assert!(!permits_volume_at(&[12,37],r));assert!(!permits_volume_at(&[13],r));assert!(permits_volume_at(&[14,41],r));}
}
