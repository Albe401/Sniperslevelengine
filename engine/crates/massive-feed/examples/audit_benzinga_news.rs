#[tokio::main]
async fn main(){
 match massive_feed::investment::benzinga_intraday_news("").await {
  Ok(report)=>println!("Benzinga access OK: {} intraday articles; partial={}",report.items.len(),report.truncated),
  Err(error)=>{eprintln!("Benzinga diagnostic: {error}");std::process::exit(1);}
 }
}
