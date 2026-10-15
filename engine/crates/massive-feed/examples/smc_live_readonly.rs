//! Thirty-second real-feed diagnostic. No broker client, orders, or strategy arming.
use massive_feed::{credentials::Credentials,live,intraday_strategy::{self,SnapshotBuffer},smc,smc_bot};
use serde_json::json;
#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error>>{
 let key=Credentials::load()?;let state=live::new_state();let feed=state.clone();
 let worker=tokio::spawn(async move{live::run(key,feed).await});
 let mut buffer=SnapshotBuffer::default();let mut observations=Vec::new();
 for _ in 0..15 {
  tokio::time::sleep(std::time::Duration::from_secs(2)).await;
  let now=live::now_ms();let snapshot=live::snapshot(&state);
  let frame=intraday_strategy::snapshot_input(&snapshot);
  let mut reading=buffer.read_paper(frame.clone(),now);
  let before=reading.eligible;let started=std::time::Instant::now();
  let settled=buffer.settled_frame(now).unwrap_or_else(||frame.clone());
  smc_bot::apply(smc::read(&settled,"QQQ",now),&mut reading,now);
  observations.push(json!({"at":now,"baselineEligible":before,"finalEligible":reading.eligible,"calculationMs":started.elapsed().as_secs_f64()*1000.,"stocksPhase":snapshot.stocks.phase,"optionsPhase":snapshot.options.phase,"reading":reading,"frame":frame,"settledFrame":settled}));
 }
 worker.abort();
 let report=json!({"version":"smc-live-readonly-v1","basis":"actual-current-feed-snapshots","fullDesktopStrategySimulation":false,"ordersSent":0,"observations":observations});
 std::fs::write("docs/smc-live-readonly-2026-10-06.json",serde_json::to_vec_pretty(&report)?)?;
 println!("{}",json!({"samples":observations.len(),"streamingSamples":observations.iter().filter(|v|v["stocksPhase"]=="streaming").count(),"smcObservingSamples":observations.iter().filter(|v|v["reading"]["quick_context"]["smc"]["context"]["state"]=="observing").count(),"ordersSent":0}));Ok(())
}
