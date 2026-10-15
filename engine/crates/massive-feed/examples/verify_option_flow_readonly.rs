//! Current-feed diagnostic; no broker client, order submission, or entry arming.
use massive_feed::{credentials::Credentials,live};
use serde_json::json;
#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error>>{
 let key=Credentials::load()?;let state=live::new_state();
 {let mut s=state.lock().unwrap();s.bot_enabled=true;s.bot_qqq_only=true;s.set_etf_scanner_enabled(true);}
 let feed=state.clone();let worker=tokio::spawn(async move{live::run(key,feed).await});
 let mut samples=Vec::new();
 for _ in 0..8 {tokio::time::sleep(std::time::Duration::from_secs(5)).await;let now=live::now_ms();let s=live::snapshot(&state);let qqq=live::bot_frame(&state,"QQQ",now);let spy=live::bot_frame(&state,"SPY",now);
  samples.push(json!({"at":now,"stocksPhase":s.stocks.phase,"optionsPhase":s.options.phase,"optionTiming":s.option_timing,"qqqFlowStatus":qqq["optionFlowStatus"],"spyFlowStatus":spy["optionFlowStatus"],"qqqWindowEvents":qqq["flow"]["intraday_windows"].as_array().map(|rows|rows.iter().map(|r|json!({"minutes":r["minutes"],"events":r["event_count"]})).collect::<Vec<_>>()),"spyWindowEvents":spy["flow"]["intraday_windows"].as_array().map(|rows|rows.iter().map(|r|json!({"minutes":r["minutes"],"events":r["event_count"]})).collect::<Vec<_>>())}));
 }
 worker.abort();let at=live::now_ms();
 let rth=snipers_core::calendar::date_at(at).is_some_and(|d|matches!(snipers_core::calendar::schedule(d),snipers_core::calendar::Day::Open(s) if at>=s.open_ms&&at<s.close_ms));
 let report=json!({"version":"option-flow-settlement-v1","at":at,"regularMarketOpen":rth,"ordersSent":0,"samples":samples});
 std::fs::write("docs/option-flow-live-verification-2026-10-06.json",serde_json::to_vec_pretty(&report)?)?;
 println!("{}",json!({"regularMarketOpen":rth,"ordersSent":0,"lastSample":samples.last()}));Ok(())
}
