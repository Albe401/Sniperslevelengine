//! Read-only verification, stores only public company/market data.
use massive_feed::investment::{research_mode, scan, Config};
use std::sync::{atomic::AtomicBool, Arc};
#[tokio::main]
async fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let symbol = args.first().map(String::as_str).unwrap_or("AAPL");
    let mut report = scan(
        Config {
            symbols: symbol.into(),
            limit: 1,
            ..Default::default()
        },
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .await
    .expect("Scan failed");
    let c = report.rows.first_mut().expect("Company missing");
    let price = c
        .metrics
        .iter()
        .find(|m| m.id == "last_close")
        .and_then(|m| m.value)
        .expect("No close");
    let data = research_mode(
        symbol,
        price,
        &c.price_date,
        "",
        "all",
        100_000.,
        args.iter().any(|s| s == "levels"),
    )
    .await
    .expect("Research failed");
    println!(
        "{}: {} option rows, complete {}; {} trades, complete {}; {} blocks; errors {:?}",
        symbol,
        data.chain_rows,
        data.chain_complete,
        data.examined_trades,
        data.trade_complete,
        data.blocks.len(),
        data.errors
    );
    c.research = Some(data);
    std::fs::write(
        args.get(1)
            .map(String::as_str)
            .unwrap_or("artifacts/investment-research-validation.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
