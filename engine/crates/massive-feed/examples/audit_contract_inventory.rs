use massive_feed::etf_scanner;
#[tokio::main]
async fn main(){
 let symbol=std::env::args().nth(1).unwrap_or_else(||"QQQ".into());
 match etf_scanner::fetch_stock(&symbol,183,None).await {
  Ok(r)=>{let ds=&r.enrichment.as_ref().unwrap().details;
   let sides=|kind:&str|{let rows:Vec<_>=ds.iter().filter(|c|c.kind==kind).collect();serde_json::json!({"series":rows.len(),"oiKnown":rows.iter().filter(|c|c.oi.is_some()).count(),"oiObserved":rows.iter().filter_map(|c|c.oi).sum::<f64>(),"volumeObserved":rows.iter().filter_map(|c|c.volume).sum::<f64>()})};
   println!("{}",serde_json::json!({"symbol":r.symbol,"at":r.at,"contracts":r.contracts,"expirations":r.expirations,"call":sides("call"),"put":sides("put")}));
  },Err(e)=>{eprintln!("CONTRACT_INVENTORY_ERROR: {e}");std::process::exit(1);}
 }
}
