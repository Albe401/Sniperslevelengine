//! Read-only diagnostics. Account connectivity never implies permission to enter.
use crate::{alpaca::PaperClient, broker_quotes::Quotes, live::now_ms};
use serde_json::{json, Value};
use snipers_core::calendar::{date_at, rfc3339_ms};

pub fn account_enabled(account: &Value) -> bool {
    account["status"] == "ACTIVE"
        && account["trading_blocked"] == false
        && account["account_blocked"] == false
        && account["options_trading_level"].as_u64().unwrap_or(0) >= 2
}

fn session_date(clock: &Value) -> Option<String> {
    let field = if clock["is_open"] == true {
        "timestamp"
    } else {
        "next_open"
    };
    date_at(rfc3339_ms(clock[field].as_str()?)?).map(|d| d.to_string())
}

pub async fn check(client: &PaperClient) -> Result<Value, String> {
    let snapshot = client.snapshot().await?;
    let account = &snapshot["account"];
    let clock = &snapshot["clock"];
    let expiry = session_date(clock);
    let selected = match expiry.as_deref() {
        Some(date) => crate::paper_expiry::select(client, date).await,
        None => Err("Sesión no disponible".into()),
    };
    let selected_expiry = selected.as_ref().ok().cloned();
    let expiry_error = selected.err();
    let sample = async {
        let expiry = selected_expiry
            .as_deref()
            .ok_or("Sesión de Alpaca no disponible")?;
        let quotes = Quotes::load()?;
        let spot = quotes.spot().await?;
        quotes.select("CALL", expiry, spot).await
    }
    .await;
    let (quote, quote_error, quote_fresh) = match sample {
        Ok(q) => {
            let error = q.validate(now_ms(), false, 0.).err();
            let fresh = error.is_none();
            (json!(q), error, fresh)
        }
        Err(e) => (Value::Null, Some(e), false),
    };
    Ok(json!({
        "environment":"paper", "account_status":account["status"],
        "account_enabled":account_enabled(account),
        "options_level":account["options_trading_level"], "cash":account["cash"],
        "market_open":clock["is_open"], "next_open":clock["next_open"],
        "session_date":expiry,"selected_expiry":selected_expiry,"expiry_error":expiry_error, "positions":snapshot["positions"].as_array().map(Vec::len),
        "source":"Massive", "quote":quote, "quote_error":quote_error,
        "quote_fresh":quote_fresh, "orders_sent":0
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_market_uses_broker_next_session_in_eastern_time() {
        assert_eq!(
            session_date(&json!({"is_open":false,"next_open":"2026-09-28T13:30:00Z"})).as_deref(),
            Some("2026-09-28")
        );
        assert_eq!(
            session_date(&json!({"is_open":true,"timestamp":"2026-09-28T14:00:00Z"})).as_deref(),
            Some("2026-09-28")
        );
        assert_eq!(
            session_date(&json!({"is_open":false,"next_open":"invalid"})),
            None
        );
    }
    #[test]
    fn missing_or_blocked_account_never_enables_options() {
        let mut a = json!({"status":"ACTIVE","trading_blocked":false,"account_blocked":false,"options_trading_level":3});
        assert!(account_enabled(&a));
        for field in ["trading_blocked", "account_blocked"] {
            a[field] = true.into();
            assert!(!account_enabled(&a));
            a[field] = false.into();
        }
        assert!(!account_enabled(
            &json!({"status":"ACTIVE","options_trading_level":3})
        ));
        a["options_trading_level"] = 1.into();
        assert!(!account_enabled(&a));
    }
}
