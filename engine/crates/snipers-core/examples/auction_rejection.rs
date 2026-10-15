//! Offline JSON adapter; no data providers or financial orders.
fn main()->Result<(),Box<dyn std::error::Error>>{
 let file=std::env::args().nth(1).ok_or("Provide an input JSON file")?;
 let input:snipers_core::auction_rejection::Input=serde_json::from_str(&std::fs::read_to_string(file)?)?;
 let result=snipers_core::auction_rejection::read(&input).map_err(std::io::Error::other)?;
 println!("{}",serde_json::to_string_pretty(&result)?);Ok(())
}
