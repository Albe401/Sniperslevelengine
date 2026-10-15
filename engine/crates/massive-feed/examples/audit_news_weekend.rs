#[tokio::main]
async fn main(){
 let report=massive_feed::investment::benzinga_intraday_news("").await.expect("Consulta de noticias falló");
 let mut dates=std::collections::BTreeMap::<String,usize>::new();
 for item in &report.items {if let Some(date)=item["publishedAt"].as_str().and_then(snipers_core::calendar::rfc3339_ms).and_then(snipers_core::calendar::date_at){*dates.entry(date.to_string()).or_default()+=1;}}
 println!("{}",serde_json::json!({"count":report.items.len(),"date_counts_et":dates,"latest":report.items.first().map(|item|serde_json::json!({"title":item["title"],"publishedAt":item["publishedAt"]})),"truncated":report.truncated}));
}
