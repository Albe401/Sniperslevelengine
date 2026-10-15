fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<String>=std::env::args().collect();if args.len()!=3{return Err("Usage: option_data_coverage source.json result.json".into());}
 let input:serde_json::Value=serde_json::from_slice(&std::fs::read(&args[1])?)?;
 let report=snipers_core::option_data_coverage::evaluate(input["symbol"].as_str().ok_or("symbol missing")?,&input["scope"].to_string(),Some(input["reference"].as_array().ok_or("reference missing")?),true,input["snapshots"].as_array().ok_or("snapshots missing")?,input["checkedMs"].as_u64().ok_or("timestamp missing")?,120000)?;
 std::fs::write(&args[2],serde_json::to_vec_pretty(&report)?)?;println!("{}",serde_json::to_string(&report.metrics)?);Ok(())
}
