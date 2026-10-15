//! Offline adapter to the production four-hour route reader. No network or orders.
use massive_feed::route_4h::{self,Minute};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let a:Vec<_>=std::env::args().skip(1).collect();
    if a.len()!=5 {return Err("Usage: SYMBOL minute-archive output THROUGH TRAINING-END".into());}
    if snipers_core::universe::stock_asset(&a[0]).is_none(){return Err("Unknown asset".into());}
    let value:serde_json::Value=serde_json::from_slice(&std::fs::read(&a[1])?)?;
    if value["symbol"]!=a[0]{return Err("Foreign archive".into());}
    let rows=value["rows"].as_array().ok_or("Missing minute rows")?;
    let (_,now)=snipers_core::calendar::day_bounds(&a[3]).ok_or("Invalid end")?;
    let (_,split)=snipers_core::calendar::day_bounds(&a[4]).ok_or("Invalid split")?;
    let mut input=Vec::with_capacity(rows.len());
    for b in rows {
        let number=|key:&str|b[key].as_f64().ok_or("Invalid minute");
        input.push(Minute{timestamp_ms:b["t"].as_u64().ok_or("Invalid timestamp")?,
          open:number("o")?,high:number("h")?,low:number("l")?,close:number("c")?,volume:number("v")?});
    }
    input.sort_by_key(|b|b.timestamp_ms);
    if input.windows(2).any(|w|w[0].timestamp_ms==w[1].timestamp_ms){return Err("Duplicate minute".into());}
    let report=route_4h::study(&a[0],&input,now,split);
    std::fs::write(&a[2],serde_json::to_vec_pretty(&report)?)?;
    println!("{}",serde_json::json!({"symbol":a[0],"closedBlocks":report.quality.closed_blocks,"outcomes":report.outcomes.len(),"probabilityClaim":false}));
    Ok(())
}
