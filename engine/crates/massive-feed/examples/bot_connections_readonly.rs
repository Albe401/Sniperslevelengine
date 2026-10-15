//! Loads local clients only: no account queries, order requests, or arming.
fn main(){
 let paper=massive_feed::alpaca::PaperClient::load();
 let quotes=massive_feed::broker_quotes::Quotes::load();
 println!("{}",serde_json::json!({"at":massive_feed::live::now_ms(),"paperLoaded":paper.is_ok(),"paperError":paper.err(),"massiveQuotesLoaded":quotes.is_ok(),"massiveError":quotes.err(),"ordersSent":0}));
}
