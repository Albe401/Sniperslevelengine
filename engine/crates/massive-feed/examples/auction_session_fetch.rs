use massive_feed::{auction_session,credentials::Credentials,live::now_ms};
use snipers_core::calendar::{self,Day};
#[tokio::main]async fn main()->Result<(),Box<dyn std::error::Error>>{
 let symbol=std::env::args().nth(1).ok_or("Symbol required")?;
 let output=std::env::args().nth(2).ok_or("Evidence path required")?;
 let now=now_ms();let date=calendar::date_at(now).ok_or("Date unavailable")?;
 let Day::Open(session)=calendar::schedule(date)else{return Err("Closed session".into())};
 let key=Credentials::load()?;
 let (mut engine,pages,rows)=auction_session::fetch(&key,&symbol,session.open_ms,now,|page,rows|{if page%5==0{eprintln!("AMT {symbol}: {page} pages, {rows} rows");}true}).await?;
 let reading=engine.snapshot(&symbol,now,true,vec![]).ok_or("No fresh profile at request cutoff")?;
 let report=serde_json::json!({"symbol":symbol,"openMs":session.open_ms,"throughMs":now,"pages":pages,"rows":rows,"source":"Massive REST /v3/trades; eligible RTH underlying prints; all venues","reading":reading});
 std::fs::write(output,serde_json::to_vec_pretty(&report)?)?;
 println!("{}",serde_json::json!({"symbol":symbol,"pages":pages,"rows":rows,"eligible":engine.history_count(),"totalVolume":report["reading"]["totalVolume"],"poc":[report["reading"]["pocLow"].clone(),report["reading"]["pocHigh"].clone()],"val":report["reading"]["val"],"vah":report["reading"]["vah"]}));Ok(())
}
