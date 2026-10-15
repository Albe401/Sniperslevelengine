//! Bounded read-only entitlement/freshness probe. No new websocket (may evict an existing session).
use crate::{credentials::Credentials, live::now_ms};
use serde_json::{json, Value};
use std::time::Duration;

async fn get(
    client: &reqwest::Client,
    key: &Credentials,
    path: &str,
    query: &[(&str, String)],
) -> Result<Value, String> {
    let response = client
        .get(format!("https://api.massive.com{path}"))
        .bearer_auth(&key.0)
        .query(query)
        .send()
        .await
        .map_err(|_| "Massive: error de conexión".to_owned())?;
    let status = response.status().as_u16();
    let date = response
        .headers()
        .get("date")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    if status != 200 {
        return Ok(json!({"http_status":status,"server_date":date}));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| "Massive: JSON inválido")?;
    Ok(json!({"http_status":status,"server_date":date,"received_ms":now_ms(),"body":body}))
}
pub fn quote_summary(quote: &Value, received_ms: u64, snapshot: bool) -> Value {
    let timestamp = quote[if snapshot {
        "last_updated"
    } else {
        "sip_timestamp"
    }]
    .as_u64();
    let age = timestamp.map(|ns| received_ms as i128 - (ns / 1_000_000) as i128);
    let bid = quote["bid"]
        .as_f64()
        .or_else(|| quote["bid_price"].as_f64());
    let ask = quote["ask"]
        .as_f64()
        .or_else(|| quote["ask_price"].as_f64());
    let valid =
        matches!((bid,ask),(Some(b),Some(a)) if b.is_finite() && a.is_finite() && b>0.0 && a>=b);
    json!({"bid":bid,"ask":ask,"bid_size":quote["bid_size"],"ask_size":quote["ask_size"],"timeframe":quote["timeframe"],"timestamp_ns":timestamp,"age_ms_local_clock":age.map(|v|v as i64),"valid_two_sided":valid,"within_5s_local_clock":age.map(|a|(0..=5000).contains(&a)).unwrap_or(false)})
}
pub async fn run() -> Result<Value, String> {
    let key = Credentials::load().map_err(str::to_owned)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Cliente no disponible")?;
    let date = snipers_core::calendar::date_at(now_ms())
        .ok_or("Fecha no disponible")?
        .to_string();
    let stock = get(
        &client,
        &key,
        "/v2/snapshot/locale/us/markets/stocks/tickers/QQQ",
        &[],
    )
    .await?;
    let spot = stock["body"]["ticker"]["lastTrade"]["p"]
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or("No se pudo obtener precio actual de QQQ")?;
    let chain = get(
        &client,
        &key,
        "/v3/snapshot/options/QQQ",
        &[
            ("expiration_date", date.clone()),
            ("strike_price.gte", (spot - 5.).to_string()),
            ("strike_price.lte", (spot + 5.).to_string()),
            ("limit", "250".into()),
        ],
    )
    .await?;
    let rows = chain["body"]["results"]
        .as_array()
        .ok_or("No se pudo consultar la cadena de opciones")?;
    let mut chosen = vec![];
    for side in ["call", "put"] {
        let mut contracts: Vec<_> = rows
            .iter()
            .filter(|r| {
                r["details"]["contract_type"] == side
                    && r["details"]["expiration_date"] == date
                    && r["details"]["strike_price"].as_f64().is_some()
            })
            .collect();
        contracts.sort_by(|a, b| {
            (a["details"]["strike_price"].as_f64().unwrap() - spot)
                .abs()
                .total_cmp(&(b["details"]["strike_price"].as_f64().unwrap() - spot).abs())
        });
        for row in contracts.into_iter().take(2) {
            let symbol = row["details"]["ticker"]
                .as_str()
                .ok_or("Contrato sin símbolo")?;
            let occ = symbol
                .strip_prefix("O:")
                .ok_or("Símbolo de opciones no válido")?;
            crate::alpaca::OrderIntent {
                symbol: occ.into(),
                qty: 1,
                limit_price: "1".into(),
                side: crate::alpaca::OrderSide::Buy,
                client_order_id: "snipers-diagnostic".into(),
            }
            .validate()?;
            chosen.push(symbol.to_owned());
        }
    }
    if chosen.len() != 4 {
        return Err("No hay cuatro contratos comparables para la prueba".into());
    }
    let mut samples = vec![];
    for round in 0..3 {
        for symbol in &chosen {
            let s = get(
                &client,
                &key,
                &format!("/v3/snapshot/options/QQQ/{symbol}"),
                &[],
            )
            .await?;
            let q = get(
                &client,
                &key,
                &format!("/v3/quotes/{symbol}"),
                &[
                    ("order", "desc".into()),
                    ("sort", "timestamp".into()),
                    ("limit", "1".into()),
                ],
            )
            .await?;
            samples.push(json!({"round":round,"symbol":symbol,"snapshot_http":s["http_status"],"snapshot_server_date":s["server_date"],"snapshot":quote_summary(&s["body"]["results"]["last_quote"],s["received_ms"].as_u64().unwrap_or(0),true),"quotes_http":q["http_status"],"quotes_server_date":q["server_date"],"quote":quote_summary(&q["body"]["results"][0],q["received_ms"].as_u64().unwrap_or(0),false)}));
        }
        if round < 2 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    // Check the exact OCC identifiers in the broker catalogue; never submit orders.
    let alpaca = crate::alpaca::PaperClient::load()?;
    let account = alpaca.snapshot().await?;
    let mut matches = vec![];
    let mut cursor = None;
    let mut pages = 0;
    loop {
        let page = alpaca.contracts("QQQ", &date, cursor.as_deref()).await?;
        pages += 1;
        if let Some(rows) = page["option_contracts"].as_array() {
            for r in rows {
                if chosen.iter().any(|s| Some(&s[2..]) == r["symbol"].as_str()) {
                    matches.push(json!({"symbol":r["symbol"],"tradable":r["tradable"],"status":r["status"],"expiration_date":r["expiration_date"],"strike_price":r["strike_price"],"type":r["type"]}));
                }
            }
        }
        cursor = page["next_page_token"].as_str().map(str::to_owned);
        if cursor.is_none() || matches.len() == chosen.len() || pages >= 20 {
            break;
        }
    }
    Ok(
        json!({"checked_ms":now_ms(),"date_et":date,"underlying":"QQQ","spot_reference":spot,"chain_contracts":rows.len(),"chain_has_more":chain["body"]["next_url"].is_string(),"samples":samples,"alpaca_contracts":matches,"alpaca_catalogue_pages":pages,"market_open":account["clock"]["is_open"],"orders_sent":0,"websocket_tested":false,"websocket_note":"No se abre otra conexión porque hay aplicaciones de mercado en ejecución"}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamp_units_and_missing_data() {
        let q = json!({"last_updated":1_000_000_000_u64,"bid":1.,"ask":1.1});
        let s = quote_summary(&q, 1100, true);
        assert_eq!(s["age_ms_local_clock"], 100);
        assert_eq!(s["within_5s_local_clock"], true);
        let s = quote_summary(&Value::Null, 1100, true);
        assert!(s["timestamp_ns"].is_null());
        assert_eq!(s["within_5s_local_clock"], false);
    }
    #[test]
    fn delayed_future_and_crossed_quotes_are_not_certified() {
        for (t, b, a) in [
            (1_000_000_000_u64, 1., 1.1),
            (902_000_000_000, 1., 1.1),
            (900_000_000_000, 2., 1.),
        ] {
            let s = quote_summary(
                &json!({"sip_timestamp":t,"bid_price":b,"ask_price":a}),
                901000,
                false,
            );
            assert!(s["within_5s_local_clock"] == false || s["valid_two_sided"] == false);
        }
    }
}
