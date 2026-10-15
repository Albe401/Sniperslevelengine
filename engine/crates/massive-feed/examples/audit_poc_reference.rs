//! Offline independent engine comparison on preserved REST rows. No network or orders.
use snipers_core::auction_map::{self,Input,Trade};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let dir=std::path::PathBuf::from(std::env::args().nth(1).ok_or("Audit directory required")?);
 let result:serde_json::Value=serde_json::from_slice(&std::fs::read(dir.join("results.json"))?)?;
 let start=result["startMs"].as_u64().ok_or("Start required")?;let end=result["endMs"].as_u64().ok_or("End required")?;
 let symbol=result["symbol"].as_str().ok_or("Symbol required")?;
 let mut old=Vec::new();let mut current=Vec::new();let mut seen=std::collections::BTreeSet::new();
 for p in result["pages"].as_array().ok_or("Pages required")?{
  let value:serde_json::Value=serde_json::from_slice(&std::fs::read(dir.join(p["file"].as_str().unwrap()))?)?;
  for row in value["results"].as_array().ok_or("Rows required")?{
   let Some(q)=row["sequence_number"].as_u64()else{continue};if !seen.insert(q){continue;}
   let t=row["sip_timestamp"].as_u64().unwrap_or(0)/1_000_000;
   if t<start||t>end||row.get("correction").is_some_and(|v|!v.is_null()&&v.as_u64()!=Some(0)){continue;}
   let Some(price)=row["price"].as_f64().filter(|v|v.is_finite()&&*v>0.)else{continue};
   let Some(volume)=massive_feed::trade_units::stock_trade_size(row["size"].as_f64().unwrap_or(0.),row["decimal_size"].as_str())else{continue};
   let conditions:Vec<u32>=row["conditions"].as_array().into_iter().flatten().filter_map(|v|v.as_u64().and_then(|v|u32::try_from(v).ok())).collect();
   let trade=Trade{symbol:symbol.into(),at:t,id:q.to_string(),price,volume};
   if conditions.iter().all(|c|[0,12,37].contains(c)){old.push(trade.clone());}
   if snipers_core::stock_volume_policy::permits_volume_at(&conditions,t){current.push(trade);}
  }
 }
 let mut readings=std::collections::BTreeMap::new();
 for (name,trades) in [("old",old),("current",current)]{
  let r=auction_map::read(&Input{symbol:symbol.into(),start_ms:start,end_ms:end+1,as_of_ms:end,bin_size:0.01,bracket_ms:1_800_000,value_fraction:0.7,complete_window:false,trades,bars:vec![]})?;
  readings.insert(name,r);
 }
 std::fs::write(dir.join("native-reference.json"),serde_json::to_vec_pretty(&serde_json::json!({"readings":readings,"ordersSent":0,"runtimeChanged":false}))?)?;
 println!("Native old/current reference profiles saved");Ok(())
}
