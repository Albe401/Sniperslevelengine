//! Read-only authenticated smoke test. Never prints credentials or account identifiers.
use massive_feed::alpaca::PaperClient;
#[tokio::main]
async fn main() -> Result<(), String> {
    let client = PaperClient::load()?;
    let s = client.snapshot().await?;
    println!(
        "{}",
        serde_json::json!({"environment":s["environment"],"status":s["account"]["status"],"cash":s["account"]["cash"],"options_level":s["account"]["options_trading_level"],"trading_blocked":s["account"]["trading_blocked"],"positions":s["positions"].as_array().map(Vec::len),"orders":s["orders"].as_array().map(Vec::len),"clock":s["clock"]})
    );
    if let Some(expiration) = std::env::args().nth(1) {
        let contracts = client.contracts("QQQ", &expiration, None).await?;
        println!(
            "{}",
            serde_json::json!({"underlying":"QQQ","expiration":expiration,"contracts_returned":contracts["option_contracts"].as_array().map(Vec::len),"has_next_page":contracts["next_page_token"].is_string()})
        );
        if let Some(next) = contracts["next_page_token"].as_str() {
            let second = client.contracts("QQQ", &expiration, Some(next)).await?;
            println!(
                "{}",
                serde_json::json!({"second_page_count":second["option_contracts"].as_array().map(Vec::len)})
            );
        }
        if let Some(symbol) = contracts["option_contracts"][0]["symbol"].as_str() {
            match client.option_quote(symbol).await {
                Ok(q) => println!(
                    "{}",
                    serde_json::json!({"opra_access":true,"quote_present":q["data"]["quotes"][symbol].is_object()})
                ),
                Err(e) => println!("{}", serde_json::json!({"opra_access":false,"reason":e})),
            }
        }
    }
    Ok(())
}
