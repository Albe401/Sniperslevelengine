#[tokio::main]
async fn main(){
 let output=std::env::args().nth(1).expect("Output path required");
 match massive_feed::investment::benzinga_intraday_news("").await{
  Ok(report)=>{let captured=massive_feed::live::now_ms();let value=serde_json::json!({"capturedMs":captured,"source":"Massive /benzinga/v2/news","windowHours":24,"maxPages":3,"limitPerPage":200,"report":report});std::fs::write(output,serde_json::to_vec_pretty(&value).unwrap()).expect("Cannot save audit input");println!("Saved dated news input; articles={}; partial={}",value["report"]["items"].as_array().unwrap().len(),value["report"]["truncated"]);},
  Err(error)=>{eprintln!("News audit failed: {error}");std::process::exit(1);}
 }
}
