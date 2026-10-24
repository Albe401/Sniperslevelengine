use snipers_core::{levels,liquidity,options};
use serde_json::json;
fn raw()->serde_json::Value {json!({"details":{"ticker":"O:QQQ261026C00710000","expiration_date":"2026-10-26","strike_price":710,"contract_type":"call"},"underlying_asset":{"ticker":"QQQ"},"open_interest":100,"day":{"volume":20},"greeks":{"gamma":0.02}})}
#[test]
fn option_series_identity_matches_expiry_side_and_strike_for_stocks_too(){
 for (field,value) in [("strike_price",json!(711)),("contract_type",json!("put")),("expiration_date",json!("2026-10-27"))]{let mut r=raw();r["details"][field]=value;assert!(options::normalize(&r,"QQQ").is_err(),"accepted mismatched {field}");}
 assert!(options::normalize(&raw(),"QQQ").is_ok());
}
#[test]
fn aggregate_contract_counts_remain_exact_safe_integers(){
 let c=levels::Contract{strike:710.,kind:"call".into(),oi:Some(4_503_599_627_370_496.),volume:Some(1.),gamma:Some(0.02),gex:Some(1.)};
 assert!(levels::calculate(&[c.clone(),c],700.,&[],false,true).is_err());
}
fn row()->levels::Row {levels::Row{strike:710.,call_oi:100.,put_oi:100.,total_oi:200.,call_volume:20.,put_volume:20.,total_volume:40.,call_gamma_weight:2.,put_gamma_weight:2.,total_gamma_weight:4.,score:80.,..Default::default()}}
#[test]
fn liquidity_rejects_duplicated_strikes_and_invalid_count_domains(){
 assert!(liquidity::calculate(&[row(),row()],700.,None,true).is_err(),"duplicates counted twice");
 for n in [-1.,0.5]{let mut r=row();r.call_oi=n;assert!(liquidity::calculate(&[r],700.,None,true).is_err(),"invalid OI {n}");}
 let mut r=row();r.dominance=3.;assert!(liquidity::calculate(&[r],700.,None,true).is_err());
}
#[test]
fn zero_activity_does_not_publish_a_directional_ratio(){
 let r=snipers_core::reading::calculate(&[levels::Row{strike:710.,..Default::default()}],true,0.,0.);
 assert!(!r.available);assert!(r.call_score.is_none());
}

#[test]
fn missing_or_duplicate_oi_changes_cannot_enable_history_weights(){
 let c=levels::Contract{strike:710.,kind:"call".into(),oi:Some(100.),volume:Some(20.),gamma:Some(0.02),gex:Some(1.)};
 for changes in [vec![],vec![(710.,Some(1.),None)],vec![(710.,Some(1.),Some(0.)),(710.,Some(1.),Some(0.))]] {
  let r=levels::calculate(&[c.clone()],700.,&changes,true,true).unwrap();assert!(!r.history_available);assert_eq!(r.rows[0].score_weights.oi_change,0.);
 }
}
#[test]
fn oi_baselines_require_the_same_identity_and_a_causal_clock(){
 use std::collections::BTreeMap;
 let mut c=options::normalize(&raw(),"QQQ").unwrap();c.premium_multiplier=Some(100.);
 let o=options::Observation{schema:1,symbol:"QQQ".into(),observed_ms:snipers_core::calendar::legacy_eastern_ms("2026-10-24 10:00:00").unwrap(),observed_date:"2026-10-24".into(),oi_reference_date:None,contracts:BTreeMap::from([(c.ticker.clone(),c)])};
 let dates=vec!["2026-10-26".into()];let mut p=o.clone();p.observed_date="2026-10-23".into();p.observed_ms-=86400000;
 assert!(snipers_core::structure::calculate(&o,Some(&p),&dates,700.).unwrap().history_available);
 let valid=p.clone();p.observed_ms=o.observed_ms+1;assert!(!snipers_core::structure::calculate(&o,Some(&p),&dates,700.).unwrap().history_available);
 p=valid;p.contracts.values_mut().next().unwrap().strike=711.;assert!(!snipers_core::structure::calculate(&o,Some(&p),&dates,700.).unwrap().history_available);
 let mut broken=o;let c=broken.contracts.values().next().unwrap().clone();broken.contracts.insert("duplicate-key".into(),c);assert!(snipers_core::structure::calculate(&broken,None,&dates,700.).is_err());
}
#[test]
fn coverage_percentages_must_match_their_covered_level_counts(){
 use snipers_core::{wall_flow::{Coverage,Window},wall_pull};
 let good=Coverage{levels_requested:1,levels_covered:1,coverage_pct:100.,contracts:Some(1.),..Default::default()};
 let mut w=Window{minutes:5,call_zone:good.clone(),put_zone:good.clone(),call_pull:good.clone(),put_pull:good};
 assert!(wall_pull::summarize(&w,"CALL").ready);
 w.call_zone.levels_covered=0;assert!(!wall_pull::summarize(&w,"CALL").ready);
 w.call_zone.levels_covered=1;w.call_pull.coverage_pct=0.;assert!(!wall_pull::summarize(&w,"CALL").ready);
}
#[test]
fn magnet_strength_has_explicit_point_units_and_no_missing_value_default(){
 use snipers_core::magnet_confirmation::{calculate,OptionsMagnet,OffMagnet};
 let off=OffMagnet{price:100.,notional:200.,total_notional:1000.,prints:20,total_prints:1000,concentration_pct:20.};
 for score in [0.,0.5,1.,50.,100.]{let r=calculate(Some(OptionsMagnet{price:100.,score:Some(score)}),Some(off),Some(100.)).unwrap();assert_eq!(r.options_score,Some(score));assert!(!r.entry_confirmed);}
 let r=calculate(Some(OptionsMagnet{price:100.,score:None}),Some(off),Some(100.)).unwrap();assert_eq!(r.status,"NO_OPTIONS_SCORE");assert!(r.options_score.is_none());
 for score in [-1.,101.]{assert!(calculate(Some(OptionsMagnet{price:100.,score:Some(score)}),Some(off),Some(100.)).is_err());}
 let bad=OffMagnet{concentration_pct:10.,..off};assert!(calculate(Some(OptionsMagnet{price:100.,score:Some(80.)}),Some(bad),Some(100.)).is_err());
}
#[test]
fn a_tied_main_magnet_is_deterministic_under_page_order_changes(){
 let make=|strike,kind:&str|levels::Contract{strike,kind:kind.into(),oi:Some(100.),volume:Some(20.),gamma:Some(0.02),gex:Some(1.)};
 let a=levels::calculate(&[make(99.,"call"),make(99.,"put"),make(101.,"call"),make(101.,"put")],100.,&[],false,true).unwrap();
 let b=levels::calculate(&[make(101.,"put"),make(101.,"call"),make(99.,"put"),make(99.,"call")],100.,&[],false,true).unwrap();
 assert_eq!(a.selected,b.selected);
}
