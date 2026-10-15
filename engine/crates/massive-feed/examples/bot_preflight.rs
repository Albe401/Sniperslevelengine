//! Read-only production preflight; never sends orders or arms the controller.
use massive_feed::{alpaca::PaperClient, paper_readiness};
#[tokio::main]
async fn main() -> Result<(), String> {
    let broker = PaperClient::load()?;
    let mut report = paper_readiness::check(&broker).await?;
    report.as_object_mut().unwrap().remove("cash");
    report["open_orders"] = serde_json::json!(broker.open_orders().await?.as_array().map(Vec::len));
    println!("{report}");
    Ok(())
}
