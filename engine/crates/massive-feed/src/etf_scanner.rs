//! Standalone ETF option profile using Snipers' native exposure model.
use crate::{credentials::Credentials,live::now_ms,option_snapshot};
use serde::Serialize;
use snipers_core::{exposure::{Contract,Model},options::Observation};
use std::collections::{BTreeMap,BTreeSet};

#[derive(Clone,Default,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Side {pub oi:Option<f64>,pub volume:Option<f64>,pub delta:Option<f64>,pub gamma:Option<f64>}
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Row {pub strike:f64,pub call:Side,pub put:Side}
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Report {pub heatmap:Option<option_snapshot::HeatmapData>,pub daily_range:Option<snipers_core::daily_range::Range>,pub amplitude:Option<crate::movement_room::Room>,pub enrichment:Option<Enrichment>,pub symbol:String,pub days:u32,pub at:u64,pub spot:f64,pub price_at:Option<u64>,pub expirations:Vec<String>,pub contracts:usize,pub rows:Vec<Row>,pub gamma_complete:bool,pub delta_complete:bool}

#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Detail {
    pub ticker:String,pub expiration:String,pub strike:f64,pub kind:String,
    pub oi:Option<f64>,pub oi_change:Option<f64>,pub volume:Option<f64>,pub volume_oi:Option<f64>,
    pub delta:Option<f64>,pub gamma:Option<f64>,pub dex:Option<f64>,pub gex:Option<f64>,pub iv:Option<f64>,pub theta:Option<f64>,pub vega:Option<f64>,
    pub charm:Option<f64>,pub vanna:Option<f64>,pub bid:Option<f64>,pub ask:Option<f64>,pub spread_pct:Option<f64>,
    pub quote_at:Option<u64>,pub realtime:bool,pub fresh:bool,
}
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Enrichment {
    pub oi_reference_date:Option<String>,pub previous_observed_date:Option<String>,pub oi_comparable:bool,
    pub price_fresh:bool,pub oi_known:usize,pub gamma_known:usize,pub delta_known:usize,pub quotes_fresh:usize,
    pub details:Vec<Detail>,pub storage_error:Option<String>,pub model:String,
}
fn nonnegative(v:Option<f64>)->Option<f64>{v.filter(|v|v.is_finite()&&*v>=0.)}
fn recent(at:Option<u64>,now:u64)->bool{at.is_some_and(|t|t<=now&&now-t<=120_000&&snipers_core::calendar::date_at(t)==snipers_core::calendar::date_at(now))}
fn previous_session(now:u64)->Option<String>{
    let mut day=snipers_core::calendar::date_at(now)?;
    for _ in 0..10 {day=day.pred_opt()?;match snipers_core::calendar::schedule(day){
        snipers_core::calendar::Day::Open(_)=>return Some(day.to_string()),
        snipers_core::calendar::Day::Unknown=>return None,
        _=>{}
    }}None
}
pub fn enrich(o:&Observation,previous:Option<&Observation>,r:&Report,storage_error:Option<String>)->Enrichment{
    let previous_day=previous_session(o.observed_ms);
    let baseline=previous.filter(|p|p.symbol==o.symbol&&Some(&p.observed_date)==previous_day.as_ref()&&p.observed_ms<o.observed_ms);
    let price_fresh=recent(r.price_at,o.observed_ms);
    let details:Vec<_>=o.contracts.values().filter(|c|r.expirations.contains(&c.expiration)).map(|c|{
        let oi=nonnegative(c.open_interest);let volume=nonnegative(c.volume);
        let old=baseline.and_then(|p|p.contracts.get(&c.ticker)).filter(|old|old.expiration==c.expiration&&old.strike==c.strike&&old.kind==c.kind).and_then(|old|nonnegative(old.open_interest));
        let realtime=c.quote_timeframe.as_deref()==Some("REAL-TIME");
        let quote_ok=recent(c.quote_ms,o.observed_ms)&&realtime;
        let bid=nonnegative(c.bid);let ask=nonnegative(c.ask);
        let spread=bid.zip(ask).filter(|(b,a)|a>=b&&a+b>0.).map(|(b,a)|100.*(a-b)/((a+b)/2.));
        let years=snipers_core::horizons::model_years(&c.expiration,o.observed_ms);
        let model=if price_fresh&&quote_ok&&spread.is_some_and(|s|s<=20.) {c.iv.zip(years).and_then(|(iv,t)|snipers_core::greek_support::model(r.spot,c.strike,iv,t,0.04,0.005,c.kind=="call"))}else{None};
        let size=oi.zip(c.premium_multiplier.filter(|m|m.is_finite()&&*m>0.)).map(|(oi,m)|oi*m);
        Detail{ticker:c.ticker.clone(),expiration:c.expiration.clone(),strike:c.strike,kind:c.kind.clone(),oi,oi_change:oi.zip(old).map(|(a,b)|a-b),volume,volume_oi:volume.zip(oi).filter(|(_,oi)|*oi>0.).map(|(v,oi)|v/oi),
            dex:size.zip(c.delta.filter(|d|d.is_finite()&&d.abs()<=1.)).map(|(s,d)|s*d.abs()),gex:size.zip(nonnegative(c.gamma)).map(|(s,g)|s*g*r.spot*r.spot*0.01),
            delta:c.delta.filter(|v|v.is_finite()&&v.abs()<=1.),gamma:nonnegative(c.gamma),iv:nonnegative(c.iv),theta:c.theta.filter(|v|v.is_finite()),vega:nonnegative(c.vega),
            charm:model.map(|m|m.0),vanna:model.map(|m|m.1),bid,ask,spread_pct:spread,quote_at:c.quote_ms,realtime,fresh:quote_ok&&spread.is_some()}
    }).collect();
    Enrichment{oi_reference_date:o.oi_reference_date.clone(),previous_observed_date:baseline.map(|p|p.observed_date.clone()),oi_comparable:baseline.is_some(),price_fresh,
        oi_known:details.iter().filter(|d|d.oi.is_some()).count(),gamma_known:details.iter().filter(|d|d.gamma.is_some()).count(),delta_known:details.iter().filter(|d|d.delta.is_some()).count(),quotes_fresh:details.iter().filter(|d|d.fresh).count(),details,storage_error,
        model:"Charm/vanna por opción larga; BSM aproximado, tasa 4%, dividendo 0,5%, cierre convencional 16:00 ET. No es inventario dealer; últimos 15 minutos sin estimación.".into()}
}

pub fn profile(o:&Observation,spot:f64,price_at:Option<u64>,days:u32)->Result<Report,&'static str>{
    profile_impl(o,spot,price_at,days,false)
}
pub fn structural_profile(o:&Observation,spot:f64,price_at:Option<u64>)->Result<Report,&'static str>{
    profile_impl(o,spot,price_at,28,true)
}
fn profile_impl(o:&Observation,spot:f64,price_at:Option<u64>,days:u32,all_dates:bool)->Result<Report,&'static str>{
    if snipers_core::universe::stock_asset(&o.symbol).is_none() || !matches!(days,0|7|28|30|183){return Err("Universo de opciones inválido");}
    let today=snipers_core::calendar::date_at(o.observed_ms).ok_or("Fecha inválida")?;
    let end=option_snapshot::window_end(o.observed_ms,days).ok_or("Fecha fuera de rango")?;
    let contracts=o.contracts.values().filter(|c|c.expiration>=today.to_string()&&(all_dates||c.expiration<=end)).collect::<Vec<_>>();
    let inputs=contracts.iter().map(|c|Contract{kind:c.kind.clone(),strike:c.strike,oi:c.open_interest,gamma:c.gamma,delta:c.delta,iv:c.iv,multiplier:c.premium_multiplier,years:snipers_core::horizons::model_years(&c.expiration,o.observed_ms)}).collect::<Vec<_>>();
    if !spot.is_finite()||spot<=0.{return Err("Precio inválido");}
    let exposure=if days==183{None}else{Some(Model::default().calculate(&inputs,spot)?)};
    let mut rows:BTreeMap<i64,Row>=BTreeMap::new();
    for c in &contracts {
        if !c.strike.is_finite()||c.strike<=0.{return Err("Strike inválido");}
        let row=rows.entry((c.strike*1e6).round() as i64).or_insert_with(||Row{strike:c.strike,call:Side{oi:Some(0.),volume:Some(0.),delta:Some(0.),gamma:Some(0.)},put:Side{oi:Some(0.),volume:Some(0.),delta:Some(0.),gamma:Some(0.)}});
        let side=match c.kind.as_str(){"call"=>&mut row.call,"put"=>&mut row.put,_=>return Err("Lado inválido")};
        let good=|v:Option<f64>|v.filter(|n|n.is_finite()&&*n>=0.);
        side.oi=side.oi.zip(good(c.open_interest)).map(|(a,b)|a+b);
        side.volume=side.volume.zip(good(c.volume)).map(|(a,b)|a+b);
        if good(c.open_interest).is_none()||c.premium_multiplier.is_none_or(|m|!m.is_finite()||m<=0.){side.gamma=None;side.delta=None;}
        if good(c.gamma).is_none(){side.gamma=None;}
        if c.delta.is_none_or(|d|!d.is_finite()||d.abs()>1.){side.delta=None;}
        if days==183 {
          let size=good(c.open_interest).zip(c.premium_multiplier.filter(|m|m.is_finite()&&*m>0.)).map(|(oi,m)|oi*m);
          side.gamma=side.gamma.zip(size).zip(good(c.gamma)).map(|((total,size),g)|total+size*g*spot*spot*0.01).filter(|v|v.is_finite());
          side.delta=side.delta.zip(size).zip(c.delta.filter(|d|d.is_finite()&&d.abs()<=1.)).map(|((total,size),d)|total+size*d.abs()).filter(|v|v.is_finite());
        }
    }
    for row in rows.values_mut(){
        if let Some(e)=exposure.as_ref().and_then(|e|e.strikes.iter().find(|e|e.strike==row.strike)){
            if row.call.gamma.is_some(){row.call.gamma=Some(e.call_gex.abs());}
            if row.put.gamma.is_some(){row.put.gamma=Some(e.put_gex.abs());}
            if row.call.delta.is_some(){row.call.delta=Some(e.call_dex.abs());}
            if row.put.delta.is_some(){row.put.delta=Some(e.put_dex.abs());}
        }
    }
    let included:BTreeSet<_>=contracts.iter().map(|c|c.ticker.as_str()).collect();
    let mut scoped=o.clone();scoped.contracts.retain(|_,c|included.contains(c.ticker.as_str()));
    let daily_range=snipers_core::daily_range::from_observation(&scoped,spot).ok();
    Ok(Report{heatmap:None,daily_range,amplitude:None,enrichment:None,symbol:o.symbol.clone(),days,at:o.observed_ms,spot,price_at,expirations:contracts.iter().map(|c|c.expiration.clone()).collect::<BTreeSet<_>>().into_iter().collect(),contracts:contracts.len(),rows:rows.values().cloned().collect(),gamma_complete:!rows.is_empty()&&rows.values().all(|r|r.call.gamma.is_some()&&r.put.gamma.is_some()),delta_complete:!rows.is_empty()&&rows.values().all(|r|r.call.delta.is_some()&&r.put.delta.is_some())})
}
pub async fn fetch(symbol:&str,days:u32,directory:Option<std::path::PathBuf>)->Result<Report,&'static str>{
    if !matches!(symbol,"SPY"|"QQQ")||!matches!(days,0|7|28|30|183){return Err("Universo ETF inválido");}
    fetch_stock_impl(symbol,days,if days>28{days}else{28},directory).await
}
pub async fn fetch_stock(symbol:&str,days:u32,directory:Option<std::path::PathBuf>)->Result<Report,&'static str>{
    fetch_stock_impl(symbol,days,days,directory).await
}
async fn fetch_stock_impl(symbol:&str,days:u32,window:u32,directory:Option<std::path::PathBuf>)->Result<Report,&'static str>{
    if snipers_core::universe::stock_asset(symbol).is_none()||!matches!(days,0|7|28|30|183){return Err("Universo de opciones inválido");}
    let key=Credentials::load()?;
    let client=reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_|"Cliente no disponible")?;
    let response=client.get(format!("https://api.massive.com/v2/last/trade/{symbol}")).query(&[("apiKey",key.0.as_str())]).send().await.map_err(|_|"Precio de referencia no disponible")?.error_for_status().map_err(|_|"Consulta de precio rechazada")?;
    let data:serde_json::Value=response.json().await.map_err(|_|"Precio inválido")?;
    if data["results"]["T"]!=symbol{return Err("Precio de otro activo");}
    let spot=data["results"]["p"].as_f64().filter(|p|p.is_finite()&&*p>0.).ok_or("Precio ausente")?;
    let price_at=data["results"]["t"].as_u64().map(|t|t/1_000_000).filter(|t|*t<=now_ms().saturating_add(1000));
    let (observation,heatmap)=if window==183 {option_snapshot::fetch_stock_window_heatmap(&key,symbol,spot).await?}else{(option_snapshot::fetch_stock_window(&key,symbol,spot,window).await?,None)};
    let disk=observation.clone();
    let (previous,storage_error)=tokio::task::spawn_blocking(move || {
        let Some(directory)=directory else{return (None,Some("Historial local no configurado".to_owned()))};
        let previous=crate::oi_store::previous(&directory,&disk.symbol,&disk.observed_date);
        static SAVED:std::sync::Mutex<BTreeMap<(std::path::PathBuf,String),u64>>=std::sync::Mutex::new(BTreeMap::new());
        let mut saved=SAVED.lock().unwrap_or_else(|p|p.into_inner());
        let bucket=disk.observed_ms/900_000;
        let cache_key=(directory.clone(),disk.symbol.clone());
        let trading_day=snipers_core::calendar::date_at(disk.observed_ms).is_some_and(|d|matches!(snipers_core::calendar::schedule(d),snipers_core::calendar::Day::Open(_)));
        let write=if !trading_day || saved.get(&cache_key)==Some(&bucket){Ok(())}else{
            crate::oi_store::save(&directory,&disk).map(|_|{saved.insert(cache_key,bucket);})
        };
        match (previous,write){(Ok(p),Ok(()))=>(p,None),(Ok(p),Err(e))=>(p,Some(e.to_owned())),(Err(e),_)=>(None,Some(e.to_owned()))}
    }).await.unwrap_or((None,Some("Historial OI no disponible".into())));
    let mut report=profile(&observation,spot,price_at,days)?;
    report.heatmap=heatmap;
    report.enrichment=Some(enrich(&observation,previous.as_ref(),&report,storage_error));
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn observation()->Observation{
        let contract=|expiration:&str,strike:f64,kind:&str|snipers_core::options::ContractSnapshot{
            ticker:format!("{expiration}-{strike}-{kind}"),expiration:expiration.into(),strike,kind:kind.into(),
            open_interest:Some(10.),volume:Some(3.),premium_multiplier:Some(100.),shares_per_contract:Some(100.),
            delta:Some(if kind=="call"{0.5}else{-0.4}),gamma:Some(0.01),iv:Some(0.2),theta:None,vega:None,bid:None,ask:None,quote_ms:None,trade_ms:None,underlying_price:None,underlying_ms:None,quote_timeframe:None,underlying_timeframe:None
        };
        let contracts=[contract("2026-10-02",500.,"call"),contract("2026-10-02",500.,"put"),contract("2026-10-09",800.,"call"),contract("2026-10-30",200.,"put"),contract("2026-11-02",900.,"call")].into_iter().map(|c|(c.ticker.clone(),c)).collect();
        Observation{schema:1,symbol:"SPY".into(),observed_ms:snipers_core::calendar::legacy_eastern_ms("2026-10-02 10:00:00").unwrap(),observed_date:"2026-10-02".into(),oi_reference_date:None,contracts}
    }
    #[test]
    fn calendar_windows_include_all_strikes_and_use_native_exposure(){
        let o=observation();
        let day=profile(&o,500.,None,0).unwrap();assert_eq!(day.contracts,2);assert_eq!(day.rows.len(),1);
        assert_eq!(day.rows[0].call.gamma,Some(25000.));assert_eq!(day.rows[0].put.gamma,Some(25000.));
        assert_eq!(day.rows[0].call.delta,Some(500.));assert_eq!(day.rows[0].put.delta,Some(400.));
        let week=profile(&o,500.,None,7).unwrap();assert_eq!(week.contracts,3);assert!(week.rows.iter().any(|r|r.strike==800.));
        let four=profile(&o,500.,None,28).unwrap();assert_eq!(four.contracts,4);assert!(four.rows.iter().any(|r|r.strike==200.));assert!(!four.rows.iter().any(|r|r.strike==900.));
    }
    #[test]
    fn missing_fields_stay_unknown_and_no_0dte_does_not_fall_back(){
        let mut o=observation();o.contracts.retain(|_,c|c.expiration!="2026-10-02");
        assert!(profile(&o,500.,None,0).unwrap().rows.is_empty());
        let c=o.contracts.values_mut().find(|c|c.strike==800.).unwrap();c.open_interest=None;c.volume=None;
        let week=profile(&o,500.,None,7).unwrap();let r=&week.rows[0];
        assert!(r.call.oi.is_none()&&r.call.volume.is_none()&&r.call.delta.is_none()&&r.call.gamma.is_none());
    }
    #[test]
    fn structural_profile_supports_selected_stocks_and_preserves_actual_expiry_universe(){
        let mut o=observation();o.symbol="NVDA".into();
        let r=structural_profile(&o,500.,Some(o.observed_ms)).unwrap();
        assert_eq!(r.contracts,5);assert!(r.expirations.contains(&"2026-11-02".into()));
        let e=enrich(&o,None,&r,None);assert_eq!(e.details.len(),5);
        assert_eq!(profile(&o,500.,None,28).unwrap().contracts,4);
        o.symbol="UNKNOWN".into();assert!(structural_profile(&o,500.,None).is_err());
    }
    #[test]
    fn oi_changes_require_consecutive_sessions_same_contract_and_known_values(){
        let o=observation();let r=profile(&o,500.,Some(o.observed_ms),28).unwrap();
        let mut previous=o.clone();previous.observed_ms-=86_400_000;previous.observed_date="2026-10-01".into();
        previous.contracts.values_mut().for_each(|c|c.open_interest=Some(8.));
        let e=enrich(&o,Some(&previous),&r,None);assert!(e.oi_comparable);
        assert!(e.details.iter().all(|c|c.oi_change==Some(2.)&&c.volume_oi==Some(0.3)));
        previous.symbol="QQQ".into();assert!(!enrich(&o,Some(&previous),&r,None).oi_comparable);
        previous.symbol="SPY".into();previous.observed_date="2026-09-30".into();assert!(!enrich(&o,Some(&previous),&r,None).oi_comparable);
        previous.observed_date="2026-10-01".into();previous.contracts.clear();
        assert!(enrich(&o,Some(&previous),&r,None).details.iter().all(|d|d.oi_change.is_none()));
    }
    #[test]
    fn sensitivities_require_recent_price_real_time_quote_and_valid_spread(){
        let mut o=observation();let now=o.observed_ms;
        for c in o.contracts.values_mut(){c.quote_ms=Some(now);c.quote_timeframe=Some("REAL-TIME".into());c.bid=Some(1.);c.ask=Some(1.1);}
        let r=profile(&o,500.,Some(now),0).unwrap();let e=enrich(&o,None,&r,None);
        assert_eq!(e.details.len(),2);assert!(e.details.iter().all(|d|d.charm.is_some()&&d.vanna.is_some()&&d.fresh));
        let stale=profile(&o,500.,Some(now-120_001),0).unwrap();assert!(enrich(&o,None,&stale,None).details.iter().all(|d|d.charm.is_none()));
        o.contracts.values_mut().for_each(|c|c.quote_timeframe=Some("DELAYED".into()));assert_eq!(enrich(&o,None,&r,None).quotes_fresh,0);
        assert_eq!(previous_session(snipers_core::calendar::legacy_eastern_ms("2026-10-05 10:00:00").unwrap()).as_deref(),Some("2026-10-02"));
    }
    #[test]
    fn six_month_inventory_preserves_distant_strikes_without_greeks_and_excludes_later_expiry(){
      let mut o=observation();let mut c=o.contracts.values().next().unwrap().clone();c.ticker="far-call".into();c.expiration="2027-04-02".into();c.strike=5000.;c.gamma=None;c.delta=None;o.contracts.insert(c.ticker.clone(),c.clone());
      c.ticker="too-late".into();c.expiration="2027-04-03".into();o.contracts.insert(c.ticker.clone(),c);
      let r=profile(&o,500.,None,183).unwrap();let row=r.rows.iter().find(|r|r.strike==5000.).unwrap();
      assert_eq!(row.call.oi,Some(10.));assert!(row.call.gamma.is_none());assert!(r.expirations.contains(&"2027-04-02".into()));assert!(!r.expirations.contains(&"2027-04-03".into()));
    }
    #[test]
    fn inventory_above_twenty_thousand_series_does_not_hit_the_strategy_model_limit(){
      let mut o=observation();let source=o.contracts.values().next().unwrap().clone();o.contracts.clear();
      for i in 0..20001 {let mut c=source.clone();c.ticker=format!("series-{i}");c.strike=100.+i as f64*0.01;c.kind=if i%2==0{"call"}else{"put"}.into();o.contracts.insert(c.ticker.clone(),c);}
      let r=profile(&o,500.,None,183).unwrap();assert_eq!(r.contracts,20001);assert_eq!(r.rows.len(),20001);
    }

}
