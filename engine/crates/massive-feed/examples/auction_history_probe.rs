//! Read-only check of the existing Massive history endpoint. Prints no credentials.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args:Vec<_>=std::env::args().skip(1).collect();
    let symbol=args.first().ok_or("symbol required")?;
    if snipers_core::universe::stock_asset(symbol).is_none(){return Err("unsupported symbol".into());}
    let output=args.get(1).ok_or("output required")?;
    let key=massive_feed::credentials::Credentials::load()?;
    let history=massive_feed::history::fetch_chart_history(&key,symbol,massive_feed::live::now_ms()).await?;
    std::fs::write(output,serde_json::to_vec(&history)?)?;
    println!("{}",serde_json::json!({"symbol":symbol,"sessions":history.sessions.len(),"minutes":history.minutes.len(),"latestDate":history.sessions.last().map(|s|&s.date)}));
    Ok(())
}
