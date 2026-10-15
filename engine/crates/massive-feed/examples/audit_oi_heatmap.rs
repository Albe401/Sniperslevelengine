//! Explicit read-only Massive capture for the new heatmap audit; never sends orders.
#[tokio::main]
async fn main(){
    let output=std::env::args().nth(1).expect("output path required");
    match massive_feed::etf_scanner::fetch_stock("QQQ",183,None).await {
        Ok(report)=>{let data=serde_json::to_vec(&report).expect("serialize");std::fs::write(output,data).expect("save audit");println!("QQQ capture saved; contracts={}",report.contracts);},
        Err(e)=>{eprintln!("Capture failed: {e}");std::process::exit(1);}
    }
}
