//! Read-only provider audit. Prints counts/levels only; never URLs or credentials.
use massive_feed::{
    credentials::Credentials,
    live::now_ms,
    off_exchange::{dates, fetch_report},
};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let symbol = std::env::args().nth(1).unwrap_or("QQQ".into());
    let now = now_ms();
    let date = std::env::args().nth(2).unwrap_or(dates(now)?[1].clone());
    let key = Credentials::load()?;
    let day = fetch_report(&key, &symbol, &date, now, |pages, trades, off| {
        println!("pages={pages} stock_trades={trades} off_exchange={off}")
    })
    .await?;
    let mut corrections = std::collections::BTreeMap::new();
    for trade in &day.trades {
        *corrections.entry(trade.correction).or_insert(0usize) += 1;
    }
    println!("Correction indicators: {corrections:?}");
    if std::env::args().nth(3).as_deref() == Some("--save-audit") {
        let path = std::path::PathBuf::from("target")
            .join(format!("trf-audit-{symbol}-{date}-{now}.json"));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        serde_json::to_writer(std::io::BufWriter::new(file), &day.trades)?;
        println!("Local audit fixture: {}", path.display());
    }
    let mut engine = snipers_core::off_exchange_tape::Engine::new(&symbol)?;
    engine.history(&date, day.trades, now, true)?;
    let snapshot = engine.snapshot(now);
    let summary = if snapshot.session.date == date {
        &snapshot.session
    } else {
        snapshot
            .history
            .iter()
            .find(|h| h.date == date)
            .ok_or("No retained session")?
    };
    println!("{}", serde_json::to_string_pretty(summary)?);
    Ok(())
}
