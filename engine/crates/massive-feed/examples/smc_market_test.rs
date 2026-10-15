//! Read-only current-session replay. Never opens a broker or a websocket.
use massive_feed::{credentials::Credentials, history, live, smc};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = Credentials::load()?;
    let start = snipers_core::calendar::rfc3339_ms("2026-10-06T09:30:00-04:00").unwrap();
    let mut reports = Vec::new();
    for symbol in ["QQQ", "SPY", "NVDA", "AAPL", "TSLA"] {
        let now = live::now_ms();
        let rows = history::fetch(&key, symbol, start, now).await?;
        let closed: Vec<_> = rows.into_iter().filter(|b| b.timestamp_ms >= start && b.timestamp_ms + 60000 <= now).collect();
        let mut evidence = BTreeMap::<String, Value>::new();
        let mut replayed = 0;
        let began = std::time::Instant::now();
        let mut latest = Value::Null;
        for i in 1..=closed.len() {
            let b = &closed[i-1];
            let at = b.timestamp_ms + 60000;
            if (at-start)%120000 != 0 { continue; }
            let frame = json!({"symbol":symbol,"publishedMs":at,"stocks":{"phase":"streaming"},"ticks":{symbol:{"symbol":symbol,"price":b.close,"tradeMs":at}},"chartHistory":{"minutes":&closed[..i]}});
            latest = smc::read(&frame, symbol, at);
            replayed += 1;
            for layer in latest["layers"].as_array().into_iter().flatten() {
                for event in layer["events"].as_array().into_iter().flatten() {
                    evidence.entry(format!("event:{}",event["id"])).or_insert(event.clone());
                }
                for setup in layer["setups"].as_array().into_iter().flatten() {
                    if setup["state"] == "CONFIRMED" {
                        evidence.entry(format!("setup:{}",setup["id"])).or_insert(json!({"firstObservedAt":at,"setup":setup}));
                    }
                }
            }
        }
        let elapsed = began.elapsed().as_secs_f64()*1000.;
        let minutes: Vec<massive_feed::route_4h::Minute> = closed.iter().map(Into::into).collect();
        let mut outcomes=Vec::new();
        for e in evidence.values().filter(|e|e["setup"].is_object()) {
            let setup=&e["setup"];
            let available=e["firstObservedAt"].as_u64().unwrap();
            if let Some(entry)=closed.iter().find(|b|b.timestamp_ms>=available) {
                let side=setup["side"].as_i64().unwrap_or(0) as i8;
                let anchor=setup["invalidation"].as_f64().unwrap_or(0.);
                let risk=(entry.open-anchor)*(side as f64);
                if risk<=0. {continue;}
                let candidate=massive_feed::smc_study::Candidate{symbol:symbol.into(),setup_id:setup["id"].as_str().unwrap_or("").into(),side,available_at:available,entry_at:entry.timestamp_ms,entry:entry.open,anchor,target:entry.open+(side as f64)*2.*risk,deadline:start+390*60000};
                outcomes.push(json!({"candidate":candidate,"targetBasis":"illustrative-2R-proxy-not-bot-target","measurement":massive_feed::smc_study::measure(&candidate,&minutes,now)}));
            }
        }
        let report = json!({"symbol":symbol,"capturedAt":now,"closedMinutes":closed.len(),"lastMinuteAt":closed.last().map(|b|b.timestamp_ms),"replayedFrames":replayed,"meanReplayMs":elapsed/(replayed.max(1) as f64),"evidence":evidence,"outcomes":outcomes,"latest":latest,"minutes":closed});
        println!("{}",json!({"symbol":symbol,"closedMinutes":closed.len(),"replayedFrames":replayed,"evidenceCount":evidence.len(),"meanReplayMs":elapsed/(replayed.max(1) as f64)}));
        reports.push(report);
    }
    let out=json!({"version":"smc-market-replay-v1","capturedAt":live::now_ms(),"basis":"real-Massive-closed-minute-bars-asof-prefix-replay","tickReference":"aggregate-close-for-replay-not-a-historical-tick","fullStrategyBacktestAvailable":false,"missing":["historical-option-quotes","historical-classified-option-flow","historical-CVD-snapshots"],"optionsProfitability":null,"ordersSent":0,"reports":reports});
    std::fs::write("docs/smc-market-2026-10-06.json",serde_json::to_vec_pretty(&out)?)?;
    Ok(())
}
