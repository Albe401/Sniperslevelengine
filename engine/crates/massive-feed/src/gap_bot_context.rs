//! Price gaps are route context, never independent entry votes or risk overrides.
use serde_json::{json,Value};
use snipers_core::gap_engine::{self,Report};
pub fn read(s:&Value,symbol:&str,spot:f64,target:Option<f64>,side:&str,now:u64)->Value {
    let mut out=json!({"version":"gap-bot-v1","symbol":symbol,"at":now,"available":false,"zones":[],"mode":"context-only","probabilityClaim":false,"changesRiskLevels":false});
    let Ok(report)=serde_json::from_value::<Report>(s["gapReport"].clone()) else{return out;};
    let through=gap_engine::sessions(now,1).and_then(|v|v.last().map(|d|d.date.clone()));
    let today=snipers_core::calendar::date_at(now).map(|d|d.to_string()).unwrap_or_default();
    let tick=&s["ticks"][symbol];
    let live=tick["tradeMs"].as_u64().is_some_and(|t|t<=now && now-t<=15000) && tick["price"].as_f64().is_some_and(|p|p.is_finite()&&p>0.);
    if !live || s["symbol"]!=symbol || s["stocks"]["phase"]!="streaming" || !spot.is_finite() || spot<=0. || report.version!=gap_engine::VERSION || report.symbol!=symbol || !matches!(report.phase.as_str(),"ready"|"partial") || report.at>now || now-report.at>5000 || through.as_deref()!=Some(&report.through) || !report.actions_verified || report.gaps.len()>122 {return out;}
    let mut zones=vec![];
    let snap=&s["optionSnapshotStatus"];
    let range=&snap["dailyRange"];
    let band=(||{let at=snap["observedMs"].as_u64()?;let lo=range["snapshotLower"].as_f64()?;let hi=range["snapshotUpper"].as_f64()?;let anchor=range["spot"].as_f64()?;let movement=range["expectedMoveDollars"].as_f64()?;
        (snap["symbol"]==symbol && snap["phase"]=="ready" && range["ready"]==true && at<=now && now-at<=900000 && snipers_core::calendar::date_at(at)==snipers_core::calendar::date_at(now) && [lo,hi,anchor,movement].iter().all(|v|v.is_finite()&&*v>0.) && lo<hi && (lo-(anchor-movement)).abs()<0.01 && (hi-(anchor+movement)).abs()<0.01).then_some((lo,hi))})();
    for g in report.gaps {
        let width=g.upper-g.lower;
        if !g.verified || g.ex_dividend || !matches!(g.state.as_str(),"OPEN"|"PARTIAL") || !matches!(g.origin.as_str(),"UP"|"DOWN") || !matches!(g.kind.as_str(),"OPENING"|"SESSION_RANGE") || g.born>today || snipers_core::calendar::day_bounds(&g.born).is_none() || ![g.lower,g.upper,g.remaining_lower,g.remaining_upper,g.fill_pct].iter().all(|v|v.is_finite()) || g.lower<=0. || width<=0. || !(0. ..100.).contains(&g.fill_pct) || g.remaining_lower<g.lower || g.remaining_upper>g.upper || g.remaining_upper<=g.remaining_lower || (g.remaining_upper-g.remaining_lower-width*(1.-g.fill_pct/100.)).abs()>0.000001 || (g.state=="OPEN")!=(g.fill_pct==0.) {continue;}
        if (g.origin=="UP" && (g.remaining_lower-g.lower).abs()>0.000001) || (g.origin=="DOWN" && (g.remaining_upper-g.upper).abs()>0.000001){continue;}
        let direction=if g.remaining_lower>spot{"above"}else if g.remaining_upper<spot{"below"}else{"inside"};
        let distance=(g.remaining_lower-spot).max(spot-g.remaining_upper).max(0.);
        let in_path=target.filter(|t|t.is_finite()&&*t>0.).is_some_and(|t|if side=="CALL"&&t>spot{g.remaining_upper>spot&&g.remaining_lower<t}else if side=="PUT"&&t<spot{g.remaining_lower<spot&&g.remaining_upper>t}else{false});
        let expected_band=band.map(|(lo,hi)|if g.remaining_lower>=lo && g.remaining_upper<=hi{"inside"}else if g.remaining_upper<lo || g.remaining_lower>hi{"outside"}else{"partial"}).unwrap_or("pending");
        zones.push(json!({"id":g.id,"kind":g.kind,"state":g.state,"lower":g.remaining_lower,"upper":g.remaining_upper,"fillPct":g.fill_pct,"ageSessions":g.age_sessions,"direction":direction,"distance":distance,"inPath":in_path,"expectedBand":expected_band,"fillTarget":if direction=="above" || direction=="inside"&&g.origin=="DOWN"{g.remaining_upper}else{g.remaining_lower}}));
    }
    zones.sort_by(|a,b|a["distance"].as_f64().unwrap().total_cmp(&b["distance"].as_f64().unwrap()));
    let mut selected=vec![];
    for direction in ["above","inside","below"] {selected.extend(zones.iter().filter(|z|z["direction"]==direction).take(3).cloned());}
    out["available"]=json!(true);out["zones"]=json!(selected);out
}
pub fn apply(s:&Value,r:&mut crate::intraday_strategy::Reading,now:u64){
    r.quick_context["gaps"]=read(s,&r.symbol,r.spot,r.next,&r.entry_side,now);
}
#[cfg(test)]
mod tests {
 use super::*;
 fn fixture()->(Value,u64){let now=snipers_core::calendar::rfc3339_ms("2026-10-06T10:00:00-04:00").unwrap();(json!({"symbol":"QQQ","ticks":{"QQQ":{"price":100.,"tradeMs":now}},"stocks":{"phase":"streaming"},"gapReport":{"version":"price-gap-v1","symbol":"QQQ","phase":"ready","through":"2026-10-05","at":now,"coveragePct":100.,"actionsVerified":true,"detail":"","gaps":[{"id":"a","kind":"SESSION_RANGE","born":"2026-10-02","origin":"DOWN","lower":103.,"upper":104.,"remainingLower":103.,"remainingUpper":104.,"state":"OPEN","fillPct":0.,"ageSessions":2,"filledAt":null,"reclaimedAt":null,"verified":true,"exDividend":false}]}}),now)}
 #[test]fn path_is_directional_and_does_not_activate_entries(){let(s,now)=fixture();assert_eq!(read(&s,"QQQ",100.,Some(105.),"CALL",now)["zones"][0]["inPath"],true);assert_eq!(read(&s,"QQQ",110.,Some(100.),"PUT",now)["zones"][0]["inPath"],true);let mut r=crate::intraday_strategy::Reading{symbol:"QQQ".into(),spot:100.,next:Some(105.),entry_side:"CALL".into(),quick_context:json!({"fvg":{"pathBlocked":true}}),..Default::default()};apply(&s,&mut r,now);assert!(!r.eligible);assert_eq!(r.next,Some(105.));assert_eq!(r.quick_context["fvg"]["pathBlocked"],true);}
 #[test]fn invalid_history_never_confirms(){let(s,now)=fixture();for change in ["foreign","stale","future","dividend","geometry","filled"]{let mut bad=s.clone();match change{"foreign"=>bad["gapReport"]["symbol"]=json!("SPY"),"stale"=>bad["gapReport"]["at"]=json!(now-5001),"future"=>bad["gapReport"]["at"]=json!(now+1),"dividend"=>bad["gapReport"]["gaps"][0]["exDividend"]=json!(true),"geometry"=>bad["gapReport"]["gaps"][0]["remainingUpper"]=json!(105.),_=>bad["gapReport"]["gaps"][0]["state"]=json!("FILLED")};assert!(read(&bad,"QQQ",100.,Some(105.),"CALL",now)["zones"].as_array().unwrap().is_empty(),"{change}");}}
 #[test]fn speed(){let(s,now)=fixture();let start=std::time::Instant::now();for _ in 0..1000{read(&s,"QQQ",100.,Some(105.),"CALL",now);}let mean=start.elapsed().as_secs_f64();println!("GAP bot context: {mean:.3} ms/reading");assert!(mean<5.);}
 #[test]fn snapshot_band_is_not_reanchored_at_live_price(){let(mut s,now)=fixture();s["optionSnapshotStatus"]=json!({"symbol":"QQQ","phase":"ready","observedMs":now,"dailyRange":{"ready":true,"spot":100.,"expectedMoveDollars":5.,"snapshotLower":95.,"snapshotUpper":105.}});assert_eq!(read(&s,"QQQ",110.,Some(100.),"PUT",now)["zones"][0]["expectedBand"],"inside");s["optionSnapshotStatus"]["observedMs"]=json!(now-900001);assert_eq!(read(&s,"QQQ",110.,Some(100.),"PUT",now)["zones"][0]["expectedBand"],"pending");}
 #[test]fn background_frame_never_borrows_selected_asset_gaps(){let state=crate::live::new_state();let(_,now)=fixture();{let mut s=state.lock().unwrap();s.gap_report.symbol="SPY".into();s.gap_reports.insert("QQQ".into(),Report{symbol:"QQQ".into(),..Default::default()});}assert_eq!(crate::live::bot_frame(&state,"QQQ",now)["gapReport"]["symbol"],"QQQ");assert!(crate::live::bot_frame(&state,"NVDA",now)["gapReport"].is_null());}
}
