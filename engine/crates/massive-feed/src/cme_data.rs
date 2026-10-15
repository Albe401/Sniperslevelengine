//! CME messages normalized without converting missing observations into zero.
use crate::cme::{number, Error};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Instrument {
    pub id: String,
    pub exchange: String,
    pub segment: String,
    pub symbol: String,
    pub product_code: String,
    pub product_type: String,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    pub price: f64,
    pub quantity: f64,
    pub last_update: String,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Observation {
    Trade {
        instrument: Instrument,
        price: f64,
        quantity: f64,
        last_update: String,
    },
    Book {
        instrument: Instrument,
        bid: Option<Level>,
        ask: Option<Level>,
    },
    Statistics {
        instrument: Instrument,
        open_interest: Option<f64>,
        cleared_volume: Option<f64>,
        settlement_price: Option<f64>,
        raw_statistics: Value,
    },
}
fn text(value: &Value, key: &str) -> Result<String, Error> {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 256)
        .map(str::to_owned)
        .ok_or(Error::Protocol)
}
fn instrument(value: &Value) -> Result<Instrument, Error> {
    Ok(Instrument {
        id: text(value, "id")?,
        exchange: text(value, "exchangeMic")?,
        segment: text(value, "marketSegmentId").or_else(|_| text(value, "marketSeqmentId"))?,
        symbol: text(value, "symbol")?,
        product_code: text(value, "productCode")?,
        product_type: text(value, "productType")?,
    })
}
fn optional_number(value: &Value, positive: bool) -> Result<Option<f64>, Error> {
    if value.is_null() {
        return Ok(None);
    }
    let result = number(value).ok_or(Error::Protocol)?;
    if positive && result < 0.0 {
        return Err(Error::Protocol);
    }
    Ok(Some(result))
}
fn level(value: &Value) -> Result<Option<Level>, Error> {
    let rows = value.as_array().ok_or(Error::Protocol)?;
    if rows.len() > 1 {
        return Err(Error::Protocol);
    }
    rows.first()
        .map(|v| {
            Ok(Level {
                price: number(&v["price"]).ok_or(Error::Protocol)?,
                quantity: optional_number(&v["qty"], true)?.ok_or(Error::Protocol)?,
                last_update: text(v, "lastUpdateTime")?,
            })
        })
        .transpose()
}
/// Entire malformed batches fail; callers must invalidate continuity on failure.
pub fn normalize(message: &Value) -> Result<Vec<Observation>, Error> {
    let kind = message["header"]["messageType"]
        .as_str()
        .ok_or(Error::Protocol)?;
    if !["TRD", "TOB", "STAT"].contains(&kind) {
        return Ok(Vec::new());
    }
    let payload = &message["payload"];
    let singleton;
    let rows = if let Some(rows) = payload.as_array() {
        rows.as_slice()
    } else if payload.is_object() {
        singleton = [payload.clone()];
        &singleton
    } else {
        return Err(Error::Protocol);
    };
    if rows.len() > 1024 {
        return Err(Error::Protocol);
    }
    rows.iter()
        .map(|row| {
            let instrument = instrument(&row["instrument"])?;
            match kind {
                "TRD" => Ok(Observation::Trade {
                    instrument,
                    price: number(&row["tradeSummary"]["tradePrice"]).ok_or(Error::Protocol)?,
                    quantity: optional_number(&row["tradeSummary"]["tradeQty"], true)?
                        .ok_or(Error::Protocol)?,
                    last_update: text(row, "lastUpdateTime")?,
                }),
                "TOB" => Ok(Observation::Book {
                    instrument,
                    bid: level(&row["bidLevel"])?,
                    ask: level(&row["askLevel"])?,
                }),
                _ => {
                    let stats = &row["tradeStatistics"];
                    if !stats.is_object() {
                        return Err(Error::Protocol);
                    }
                    Ok(Observation::Statistics {
                        instrument,
                        open_interest: optional_number(&stats["openInterest"], true)?,
                        cleared_volume: optional_number(&stats["clearedVolume"], true)?,
                        settlement_price: optional_number(&stats["settlementPrice"], false)?,
                        raw_statistics: stats.clone(),
                    })
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn instrument_value() -> Value {
        json!({"id":"123","exchangeMic":"XCME","marketSegmentId":"1","symbol":"NQZ6","productCode":"NQ","productType":"FUT"})
    }
    #[test]
    fn market_rows_preserve_identity_missing_statistics_and_book_removal() {
        let mut message = json!({"header":{"messageType":"STAT"},"payload":[{"instrument":instrument_value(),"tradeStatistics":{"openInterest":"0","settlementPrice":"-1","tradeDate":"2026-09-23"}}]});
        assert!(matches!(
            &normalize(&message).unwrap()[0],
            Observation::Statistics {
                open_interest: Some(0.0),
                cleared_volume: None,
                settlement_price: Some(-1.0),
                ..
            }
        ));
        message["header"]["messageType"] = json!("TOB");
        message["payload"][0]["bidLevel"] = json!([]);
        message["payload"][0]["askLevel"] = json!([]);
        assert!(matches!(
            &normalize(&message).unwrap()[0],
            Observation::Book {
                bid: None,
                ask: None,
                ..
            }
        ));
        message["payload"][0]["askLevel"] =
            json!([{"price":"10","qty":"2","lastUpdateTime":"2026-09-24T14:00:00Z"}]);
        assert!(matches!(
            &normalize(&message).unwrap()[0],
            Observation::Book {
                ask: Some(Level {
                    price: 10.0,
                    quantity: 2.0,
                    ..
                }),
                ..
            }
        ));
        message["payload"][0]["askLevel"][0]["qty"] = json!("-2");
        assert!(normalize(&message).is_err());
    }
    #[test]
    fn trade_batch_fails_atomically_on_missing_identity_or_invalid_quantity() {
        let row = json!({"instrument":instrument_value(),"lastUpdateTime":"2026-09-24T14:00:00Z","tradeSummary":{"tradePrice":"-1.5","tradeQty":"1"}});
        let mut message = json!({"header":{"messageType":"TRD"},"payload":[row.clone(),row]});
        assert_eq!(normalize(&message).unwrap().len(), 2);
        message["payload"][1]["instrument"]["id"] = Value::Null;
        assert!(normalize(&message).is_err());
    }
}
