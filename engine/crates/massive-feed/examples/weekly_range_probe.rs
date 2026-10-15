#[tokio::main]async fn main()->Result<(),Box<dyn std::error::Error>>{
 let symbol=std::env::args().nth(1).ok_or("Symbol required")?;
 let path=std::env::args().nth(2).ok_or("Evidence path required")?;
 let repeats=std::env::args().nth(3).and_then(|s|s.parse::<usize>().ok()).unwrap_or(1).clamp(1,10);
 let mut evidence=vec![];
 for _ in 0..repeats{
 let r=massive_feed::etf_scanner::fetch_stock(&symbol,7,None).await?;
 let now=massive_feed::live::now_ms();let value=serde_json::to_value(&r)?;
 evidence.push(serde_json::json!({"checkedMs":now,"report":value}));
 println!("{}",serde_json::json!({"symbol":symbol,"checkedMs":now,"at":r.at,"spot":r.spot,"priceAt":r.price_at,"priceAgeMs":r.price_at.map(|t|now as i128-t as i128),"contracts":r.contracts,"rows":r.rows.len(),"expirations":r.expirations,"details":value["enrichment"]["details"].as_array().map(|v|v.len())}));
 if repeats>1{tokio::time::sleep(std::time::Duration::from_millis(500)).await;}
 }
 let output=if repeats==1{evidence.remove(0)}else{serde_json::json!({"runs":evidence})};std::fs::write(path,serde_json::to_vec_pretty(&output)?)?;Ok(())
}
