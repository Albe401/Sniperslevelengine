//! Explicit single-contract collection. No WebSocket, bot state or automatic bulk download.
use massive_feed::{
    credentials::Credentials,
    option_activity_history::{self, Request},
};
use std::path::PathBuf;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 || args.len() > 5 {
        return Err("Usage: collect_option_activity_history UNDERLYING OPTION_TICKER DATE OUTPUT_DIRECTORY [MAX_PAGES]".into());
    }
    let request = Request {
        underlying: args[0].clone(),
        ticker: args[1].clone(),
        date: args[2].clone(),
        window_minutes: 5,
        max_pages: args.get(4).map(|s| s.parse()).transpose()?.unwrap_or(10),
    };
    let report = option_activity_history::collect(&Credentials::load()?, request).await?;
    let file = option_activity_history::save(&PathBuf::from(&args[3]), &report)?;
    println!(
        "{}",
        serde_json::json!({"file":file,"pages":report.pages,"rowsReceived":report.rows_received,
        "uniqueTrades":report.trades.len(),"retrievalComplete":report.retrieval_complete,
        "aggregationComplete":report.aggregation_complete,"windows":report.windows.len(),
        "warnings":report.warnings,"websocketOpened":false,"ordersSent":0})
    );
    Ok(())
}
