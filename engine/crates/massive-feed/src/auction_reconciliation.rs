//! Bounded comparison of identical AMT populations and SIP windows.
//! Matching REST does not certify the entire market or resolve corrections.
use serde::Serialize;
use std::collections::BTreeMap;
use snipers_core::auction_map::Trade;

pub const POPULATION: &str = snipers_core::stock_volume_policy::ID;
pub const MAX_ROWS: usize = 50_000;
#[derive(Clone)]
pub struct Request {
    pub symbol: String,
    pub epoch: u64,
    pub start_ms: u64,
    pub end_ms: u64,
    pub live: Vec<Trade>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct Report {
    pub population_id: &'static str,
    pub unit: &'static str,
    pub scope: &'static str,
    pub start_ms: u64,
    pub end_ms: u64,
    pub checked_at_ms: u64,
    pub live_events: usize,
    pub rest_events: usize,
    pub live_shares: f64,
    pub rest_shares: f64,
    pub matched_events: usize,
    pub missing_live_events: usize,
    pub missing_live_shares: f64,
    pub extra_live_events: usize,
    pub extra_live_shares: f64,
    pub conflicting_events: usize,
    pub rest_excluded_rows: usize,
    pub unresolved_corrections: usize,
    pub matched_reference_shares_percent: Option<f64>,
    pub status: &'static str,
    pub whole_session_certified: bool,
}
fn index<'a>(rows:&'a [Trade],request:&Request)->Result<BTreeMap<u64,&'a Trade>, &'static str> {
    if rows.len()>MAX_ROWS {return Err("Ventana AMT excede capacidad de cotejo");}
    let mut out=BTreeMap::new();
    for t in rows {
        if t.symbol!=request.symbol||t.at<request.start_ms||t.at>=request.end_ms||!t.price.is_finite()||t.price<=0.||!t.volume.is_finite()||t.volume<=0. {return Err("Operación fuera de población de cotejo AMT");}
        let key=t.id.parse::<u64>().map_err(|_|"Secuencia de cotejo AMT inválida")?;
        if out.insert(key,t).is_some(){return Err("Identidad repetida en cotejo AMT");}
    }
    Ok(out)
}
pub fn compare(request:&Request,rest:&[Trade],excluded:usize,corrections:usize,now:u64)->Result<Report,&'static str> {
    if request.start_ms>=request.end_ms||request.end_ms>now {return Err("Ventana no causal de cotejo AMT");}
    let live=index(&request.live,request)?;let reference=index(rest,request)?;
    let mut r=Report{population_id:POPULATION,unit:"shares",scope:"sampled_sip_window_accepted_AMT_vs_REST",start_ms:request.start_ms,end_ms:request.end_ms,checked_at_ms:now,live_events:live.len(),rest_events:reference.len(),live_shares:live.values().map(|t|t.volume).sum(),rest_shares:reference.values().map(|t|t.volume).sum(),matched_events:0,missing_live_events:0,missing_live_shares:0.,extra_live_events:0,extra_live_shares:0.,conflicting_events:0,rest_excluded_rows:excluded,unresolved_corrections:corrections,matched_reference_shares_percent:None,status:"pending",whole_session_certified:false};
    let mut matched_shares=0.;
    for (q,t) in &reference {
        match live.get(q) {
            None=>{r.missing_live_events+=1;r.missing_live_shares+=t.volume;},
            Some(l) if l.at==t.at&&(l.price-t.price).abs()<=1e-8&&(l.volume-t.volume).abs()<=1e-8=>{r.matched_events+=1;matched_shares+=t.volume;},
            Some(_)=>r.conflicting_events+=1,
        }
    }
    for (q,t) in &live {if !reference.contains_key(q){r.extra_live_events+=1;r.extra_live_shares+=t.volume;}}
    if !r.live_shares.is_finite()||!r.rest_shares.is_finite(){return Err("Suma de cotejo AMT fuera de rango");}
    r.matched_reference_shares_percent=if r.rest_shares>0.{Some(100.*matched_shares/r.rest_shares)}else{None};
    r.status=if corrections>0{"corrections_pending"}else if r.missing_live_events+r.extra_live_events+r.conflicting_events>0{"difference_detected"}else if reference.is_empty(){"empty_reference"}else{"matched_scoped_window"};
    Ok(r)
}
#[cfg(test)] mod tests {
    use super::*;
    fn row(q:u64,v:f64)->Trade{Trade{symbol:"QQQ".into(),at:100,id:q.to_string(),price:100.,volume:v}}
    fn request(live:Vec<Trade>)->Request{Request{symbol:"QQQ".into(),epoch:1,start_ms:100,end_ms:101,live}}
    #[test] fn identity_comparison_detects_equal_volume_with_different_trades(){let r=compare(&request(vec![row(1,10.)]),&[row(2,10.)],0,0,101).unwrap();assert_eq!(r.status,"difference_detected");assert_eq!(r.missing_live_shares,10.);assert_eq!(r.extra_live_shares,10.);assert_eq!(r.matched_reference_shares_percent,Some(0.));}
    #[test] fn sequence_gaps_are_not_missing_trades(){let rows=vec![row(1,0.25),row(99,2.)];let r=compare(&request(rows.clone()),&rows,0,0,101).unwrap();assert_eq!(r.status,"matched_scoped_window");assert_eq!(r.rest_shares,2.25);assert!(!r.whole_session_certified);}
    #[test] fn corrections_conflicts_duplicates_and_empty_are_explicit(){let r=compare(&request(vec![row(1,1.)]),&[row(1,2.)],1,1,101).unwrap();assert_eq!(r.status,"corrections_pending");assert_eq!(r.conflicting_events,1);assert!(compare(&request(vec![]),&[row(1,1.),row(1,1.)],0,0,101).is_err());assert_eq!(compare(&request(vec![]),&[],0,0,101).unwrap().matched_reference_shares_percent,None);}
}
