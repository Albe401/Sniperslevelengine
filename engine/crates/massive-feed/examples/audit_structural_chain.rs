//! Read-only Massive audit. Never writes observations, credentials or orders.
use massive_feed::{credentials::Credentials,option_snapshot};
use snipers_core::{options::Observation,horizons};
use serde_json::json;
fn audit(o:&Observation,spot:f64,source:&str)->Result<serde_json::Value,Box<dyn std::error::Error>> {
 let start=std::time::Instant::now();let hs=horizons::calculate(o,spot,"diagnostic saved reference")?;
 let mut rows=Vec::new();
 for h in hs {
  let mut call=0.0;let mut put=0.0;let mut missing_delta=0;let mut missing_oi=0;let mut missing_multiplier=0;let mut n=0;
  for c in o.contracts.values().filter(|c|h.expirations.contains(&c.expiration)) {
   n+=1;
   if c.open_interest.is_none(){missing_oi+=1;continue;}
   let oi=c.open_interest.unwrap();if oi<=0.0{continue;}
   let Some(m)=c.premium_multiplier else{missing_multiplier+=1;continue;};
   let Some(d)=c.delta else{missing_delta+=1;continue;};
   if c.kind=="call"{call+=d.abs()*oi*m;}else if c.kind=="put"{put-=d.abs()*oi*m;}
  }
  let tol=(call.abs()+put.abs()).max(1.0)*1e-9;
  if (call-h.exposure.call_dex).abs()>tol||(put-h.exposure.put_dex).abs()>tol{return Err("Independent DEX mismatch".into());}
  let malformed=o.contracts.values().filter(|c|h.expirations.contains(&c.expiration)).filter(|c| {
   let t=c.ticker.strip_prefix("O:").unwrap_or("");if t.len()<15{return true;}
   let tail=&t[t.len()-15..];
   let expiry=format!("20{}-{}-{}",&tail[..2],&tail[2..4],&tail[4..6]);
   let strike=tail[7..].parse::<f64>().unwrap_or(-1.0)/1000.0;
   expiry!=c.expiration||(strike-c.strike).abs()>1e-6||((tail.as_bytes()[6]==b'C')!=(c.kind=="call"))
  }).count();
  if malformed>0{return Err("Encoded option identity mismatch".into());}
  rows.push(json!({"mode":h.mode,"expirations":h.expirations,"contracts":n,"missingOi":missing_oi,"positiveOiMissingDelta":missing_delta,"positiveOiMissingMultiplier":missing_multiplier,"independentDexMatches":true,"identityMatches":true,"exposure":h.exposure}));
 }
 Ok(json!({"source":source,"symbol":o.symbol,"observedMs":o.observed_ms,"observedDate":o.observed_date,"oiReferenceDate":o.oi_reference_date,"referencePrice":spot,"referenceIsLive":false,"contracts":o.contracts.len(),"calculationMs":start.elapsed().as_secs_f64()*1000.0,"horizons":rows}))
}
#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error>> {
 let args=std::env::args().skip(1).collect::<Vec<_>>();let path=args.first().ok_or("Saved observation path required")?;
 let saved:Observation=serde_json::from_slice(&std::fs::read(path)?)?;
 let spot=saved.contracts.values().find_map(|c|c.underlying_price).ok_or("No saved reference")?;
 let mut reports=vec![audit(&saved,spot,"saved Massive observation")?];
 if args.iter().any(|a|a=="--refresh") {
  let key=Credentials::load()?;let t=std::time::Instant::now();
  match tokio::time::timeout(std::time::Duration::from_secs(120),option_snapshot::fetch(&key,&saved.symbol,spot,4)).await {
   Ok(Ok(fresh))=>{let mut r=audit(&fresh,spot,"fresh Massive REST")?;r["fetchSeconds"]=json!(t.elapsed().as_secs_f64());reports.push(r);}
   Ok(Err(e))=>reports.push(json!({"source":"fresh Massive REST","error":e})),
   Err(_)=>reports.push(json!({"source":"fresh Massive REST","error":"120 second timeout"})),
  }
 }
 let report=json!({"scope":"nearest expirations and this week, bounded strike range; not full underlying chain","reports":reports});
 if let Some(out)=args.get(1){if out!="--refresh"{std::fs::write(out,serde_json::to_vec_pretty(&report)?)?;}}
 println!("{}",report);Ok(())
}
