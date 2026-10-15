//! Bounded, on-demand option trade scanner. A print is a reported execution, not a new position.
use crate::{credentials::Credentials, history::checked_url, live::now_ms};
use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snipers_core::calendar;
use std::{collections::BTreeSet, time::Duration};

const MAX_CHAIN_PAGES: usize = 100;
const CONTRACTS_PER_SIDE: usize = 8;
const MAX_RESULTS: usize = 300;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimestampSource {
    #[default]
    Participant,
    Sip,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Print {
    pub ticker: String,
    pub side: String,
    pub expiration: String,
    pub strike: f64,
    pub executed_ms: Option<u64>,
    pub sip_ms: u64,
    /// Legacy saved prints required participant_timestamp; preserve their provenance.
    #[serde(default)]
    pub timestamp_source: TimestampSource,
    pub size: u64,
    pub price: f64,
    pub premium: f64,
    pub exchange: u64,
    pub sequence: Option<u64>,
}

impl Print {
    pub fn observed_ms(&self) -> u64 {
        self.executed_ms.unwrap_or(self.sip_ms)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub symbol: String,
    pub date: String,
    pub scanned_ms: u64,
    pub min_contracts: u64,
    pub chain_examined: usize,
    pub selected_contracts: usize,
    pub chain_complete: bool,
    pub trades_complete: bool,
    pub matching_prints: usize,
    pub rows: Vec<Print>,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
struct Candidate { ticker:String, side:String, expiration:String, strike:f64, volume:u64 }

pub fn valid_symbol(symbol:&str)->bool {
    (1..=10).contains(&symbol.len())&&symbol.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit()||b==b'.'||b==b'-')
}

fn candidate(v:&Value,symbol:&str,today:&str)->Option<Candidate>{
    let d=&v["details"];
    let ticker=d["ticker"].as_str()?;
    let side=d["contract_type"].as_str()?;
    let expiration=d["expiration_date"].as_str()?;
    let strike=d["strike_price"].as_f64()?;
    let volume=v["day"]["volume"].as_u64().unwrap_or(0);
    if !matches!(side,"call"|"put")||d["shares_per_contract"].as_u64()!=Some(100)
        ||d["underlying_ticker"].as_str().is_some_and(|s|s!=symbol)
        ||snipers_core::options::underlying(ticker)!=Some(symbol)||expiration<today
        ||!strike.is_finite()||strike<=0.||volume==0 {return None;}
    Some(Candidate{ticker:ticker.into(),side:side.to_ascii_uppercase(),expiration:expiration.into(),strike,volume})
}

fn parse_print(v:&Value,c:&Candidate,from:u64,to:u64,min_contracts:u64)->Option<Print>{
    let sip_ms=v["sip_timestamp"].as_u64().filter(|t|*t>=1_000_000)? / 1_000_000;
    // Only absence permits SIP fallback. Malformed supplied execution clocks are rejected.
    let executed_ms=if v["participant_timestamp"].is_null(){None}else{
        Some(v["participant_timestamp"].as_u64().filter(|t|*t>=1_000_000)? / 1_000_000)
    };
    let timestamp_source=if executed_ms.is_some(){TimestampSource::Participant}else{TimestampSource::Sip};
    let observed_ms=executed_ms.unwrap_or(sip_ms);
    let size=v["size"].as_u64()?;
    let price=v["price"].as_f64()?;
    if observed_ms<from||observed_ms>=to||size==0||size<min_contracts||!price.is_finite()||price<=0.
        ||v["correction"].as_u64().is_some_and(|n|n!=0){return None;}
    let premium=price*size as f64*100.;
    if !premium.is_finite(){return None;}
    Some(Print{ticker:c.ticker.clone(),side:c.side.clone(),expiration:c.expiration.clone(),strike:c.strike,
        executed_ms,sip_ms,timestamp_source,size,price,premium,exchange:v["exchange"].as_u64().unwrap_or(0),sequence:v["sequence_number"].as_u64()})
}

async fn get(client:&reqwest::Client,key:&str,url:reqwest::Url)->Result<Value,String>{
    let response=client.get(url).bearer_auth(key).send().await.map_err(|_|"Massive no respondió".to_string())?;
    if !response.status().is_success(){return Err(format!("Massive devolvió HTTP {}",response.status().as_u16()));}
    response.json().await.map_err(|_|"Respuesta de Massive inválida".to_string())
}

async fn contract_trades(client:reqwest::Client,key:String,c:Candidate,from:u64,to:u64,min_contracts:u64)->Result<(Vec<Print>,bool),String>{
    let path=format!("/v3/trades/{}",c.ticker);
    let mut url=reqwest::Url::parse(&format!("https://api.massive.com{path}")).map_err(|_|"Contrato inválido")?;
    url.query_pairs_mut().append_pair("timestamp.gte",&(from*1_000_000).to_string())
        .append_pair("timestamp.lt",&(to*1_000_000).to_string()).append_pair("sort","timestamp")
        .append_pair("order","desc").append_pair("limit","1000");
    let data=get(&client,&key,url).await?;
    let rows=data["results"].as_array().ok_or("Operaciones de opciones sin formato válido")?
        .iter().filter_map(|v|parse_print(v,&c,from,to,min_contracts)).collect();
    Ok((rows,data["next_url"].as_str().is_none_or(str::is_empty)))
}

/// Samples the most active contracts in a bounded chain; every displayed row is a real trade.
pub async fn scan(symbol:&str,min_contracts:u64)->Result<Report,String>{
    let symbol=symbol.trim().to_ascii_uppercase();
    if !valid_symbol(&symbol)||!(1..=10_000).contains(&min_contracts){return Err("Indica un activo y un mínimo de 1 a 10.000 contratos".into());}
    let now=now_ms();let date=calendar::date_at(now).ok_or("Fecha de Nueva York no disponible")?.to_string();
    let from=calendar::legacy_eastern_ms(&format!("{date} 09:30:00")).ok_or("Hora inválida")?;
    let end=calendar::legacy_eastern_ms(&format!("{date} 16:15:00")).ok_or("Hora inválida")?;
    if now<from{return Err("Las operaciones de opciones de hoy comienzan a las 09:30 ET".into());}
    let to=now.min(end).saturating_add(1);
    let expiry_to=crate::option_snapshot::window_end(now,183).ok_or("Vencimiento inválido")?;
    let key=Credentials::load().map_err(str::to_owned)?.0;
    let client=reqwest::Client::builder().timeout(Duration::from_secs(18)).redirect(reqwest::redirect::Policy::none())
        .build().map_err(|_|"No se pudo preparar Massive")?;
    let path=format!("/v3/snapshot/options/{symbol}");
    let mut url=reqwest::Url::parse(&format!("https://api.massive.com{path}")).map_err(|_|"Activo inválido")?;
    url.query_pairs_mut().append_pair("expiration_date.gte",&date).append_pair("expiration_date.lte",&expiry_to).append_pair("limit","250");
    let mut chain=Vec::new();let mut chain_complete=false;let mut visited=BTreeSet::new();
    for _ in 0..MAX_CHAIN_PAGES {
        let data=get(&client,&key,url.clone()).await?;
        let page=data["results"].as_array().ok_or("Cadena de opciones sin formato válido")?;
        chain.extend(page.iter().filter_map(|v|candidate(v,&symbol,&date)));
        let Some(next)=data["next_url"].as_str().filter(|s|!s.is_empty()) else{chain_complete=true;break};
        if !visited.insert(next.to_owned()){return Err("Paginación de opciones repetida".into());}
        url=checked_url(next,&path).map_err(str::to_owned)?;
    }
    let chain_examined=chain.len();
    chain.sort_by(|a,b|b.volume.cmp(&a.volume).then_with(||a.ticker.cmp(&b.ticker)));
    let selected:Vec<_>=["CALL","PUT"].into_iter().flat_map(|side|chain.iter().filter(move|c|c.side==side).take(CONTRACTS_PER_SIDE).cloned()).collect();
    let selected_contracts=selected.len();
    let results:Vec<_>=stream::iter(selected.into_iter().map(|c|contract_trades(client.clone(),key.clone(),c,from,to,min_contracts)))
        .buffer_unordered(4).collect().await;
    let mut rows=Vec::new();let mut warnings=Vec::new();let mut trades_complete=true;
    for result in results {match result{Ok((prints,complete))=>{trades_complete&=complete;rows.extend(prints);},Err(e)=>{trades_complete=false;warnings.push(e);}}}
    rows.sort_by(|a,b|b.observed_ms().cmp(&a.observed_ms()).then_with(||b.size.cmp(&a.size)));
    let matching_prints=rows.len();rows.truncate(MAX_RESULTS);
    if !chain_complete{warnings.push("Cadena limitada a 100 páginas; otros contratos pueden faltar".into());}
    if !trades_complete{warnings.push("Operaciones limitadas o incompletas en algunos contratos".into());}
    Ok(Report{symbol,date,scanned_ms:now,min_contracts,chain_examined,selected_contracts,
        chain_complete,trades_complete,matching_prints,rows,warnings})
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn parses_only_real_trade_timestamp_and_contract_size(){
        let c=Candidate{ticker:"O:QQQ261008C00500000".into(),side:"CALL".into(),expiration:"2026-10-08".into(),strike:500.,volume:100};
        let v=serde_json::json!({"participant_timestamp":1_000_000_000u64,"sip_timestamp":1_001_000_000u64,"size":50,"price":1.25,"exchange":46,"sequence_number":8});
        let p=parse_print(&v,&c,999,1001,50).unwrap();assert_eq!(p.executed_ms,Some(1000));assert_eq!(p.timestamp_source,TimestampSource::Participant);assert_eq!(p.premium,6250.);
        assert!(parse_print(&v,&c,999,1001,51).is_none());
        assert!(parse_print(&serde_json::json!({"size":50,"price":1.25}),&c,999,1001,1).is_none());
    }
    #[test]
    fn sip_only_trades_preserve_unknown_execution_time_and_window_boundaries(){
        let c=Candidate{ticker:"O:GOOGL261016C00355000".into(),side:"CALL".into(),expiration:"2026-10-16".into(),strike:355.,volume:100};
        let v=serde_json::json!({"sip_timestamp":1_001_000_000u64,"size":50,"price":1.25,"exchange":46,"sequence_number":8});
        let p=parse_print(&v,&c,1001,1002,50).unwrap();
        assert_eq!(p.executed_ms,None);assert_eq!(p.observed_ms(),1001);assert_eq!(p.timestamp_source,TimestampSource::Sip);
        assert_eq!(serde_json::to_value(&p).unwrap()["timestampSource"],"sip");
        assert!(parse_print(&v,&c,1000,1001,50).is_none());
        assert!(parse_print(&v,&c,1002,1003,50).is_none());
        for bad in [serde_json::json!(0),serde_json::json!(-1),serde_json::json!("bad")] {
            let mut invalid=v.clone();invalid["participant_timestamp"]=bad;
            assert!(parse_print(&invalid,&c,1000,1002,50).is_none());
        }
        let mut corrected=v.clone();corrected["correction"]=serde_json::json!(1);
        assert!(parse_print(&corrected,&c,1000,1002,50).is_none());
        let mut missing=v.clone();missing.as_object_mut().unwrap().remove("sip_timestamp");
        assert!(parse_print(&missing,&c,1000,1002,50).is_none());
    }
    #[test]
    fn legacy_saved_prints_keep_participant_provenance(){
        let v=serde_json::json!({"ticker":"O:QQQ261008C00500000","side":"CALL","expiration":"2026-10-08","strike":500.,"executedMs":1000,"sipMs":1001,"size":50,"price":1.25,"premium":6250.,"exchange":46,"sequence":8});
        let p:Print=serde_json::from_value(v).unwrap();
        assert_eq!(p.timestamp_source,TimestampSource::Participant);assert_eq!(p.observed_ms(),1000);
    }
    #[test]
    fn real_massive_sip_only_rows_are_retained_without_fabricating_execution_time(){
        let fixture:Value=serde_json::from_str(include_str!("../../../fixtures/option-trades-sip-real-2026-10-09.json")).unwrap();
        let c=Candidate{ticker:fixture["ticker"].as_str().unwrap().into(),side:"CALL".into(),expiration:"2026-10-16".into(),strike:355.,volume:100};
        let from=calendar::legacy_eastern_ms("2026-10-09 09:30:00").unwrap();
        let to=calendar::legacy_eastern_ms("2026-10-09 16:15:00").unwrap();
        let rows=fixture["rows"].as_array().unwrap();
        assert_eq!(rows.len(),2);
        for row in rows {
            let p=parse_print(row,&c,from,to,1).unwrap();
            assert_eq!(p.executed_ms,None);assert_eq!(p.timestamp_source,TimestampSource::Sip);
            assert_eq!(p.sip_ms,row["sip_timestamp"].as_u64().unwrap()/1_000_000);
            assert_eq!(p.premium,row["price"].as_f64().unwrap()*row["size"].as_u64().unwrap() as f64*100.);
            assert!(parse_print(row,&c,from,to,50).is_none());
        }
    }
    #[test]
    fn rejects_nonmatching_or_volume_free_candidates(){
        let v=serde_json::json!({"details":{"ticker":"O:QQQ261008P00500000","underlying_ticker":"QQQ","contract_type":"put","expiration_date":"2026-10-08","strike_price":500,"shares_per_contract":100},"day":{"volume":50}});
        assert_eq!(candidate(&v,"QQQ","2026-10-08").unwrap().side,"PUT");
        assert!(candidate(&v,"SPY","2026-10-08").is_none());
        assert!(candidate(&v,"QQQ","2026-10-09").is_none());
    }
}
