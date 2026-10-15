#[tokio::main]
async fn main(){
 if std::env::args().any(|arg|arg=="--live"){
  let mut report=massive_feed::investment::benzinga_intraday_news("").await.expect("Consulta Benzinga");
  let dir=std::path::PathBuf::from(std::env::var("APPDATA").expect("APPDATA")).join("com.snipers.workspace");
  massive_feed::news_translation::translate_news(&mut report,&dir.join("benzinga-spanish-local-v1.json")).await.expect("Traducción local completa");
  let translated=report.items.iter().filter(|item|item["translated"]==true).count();
  println!("Spanish live news verified: {translated}/{}; partial={}",report.items.len(),report.truncated);
  for item in report.items.iter().take(3){println!("{}: {}",item["ticker"],item["title"]);}
  return;
 }
 let mut report=massive_feed::investment::NewsReport{items:vec![serde_json::json!({"id":"translation-probe-v1","ticker":"TEST","title":"Company raises guidance after record revenue","summary":"The company raised its full-year revenue forecast to $250 million.","sentiment":"unknown","horizon":"intraday"})],fetched_ms:0,truncated:false};
 let path=std::path::Path::new("target/news-spanish-probe.json");
 match massive_feed::news_translation::translate_news(&mut report,path).await{Ok(())=>println!("Spanish translation verified: {}",report.items[0]["title"]),Err(error)=>{eprintln!("Translation diagnostic: {error}");std::process::exit(1);}}
}
