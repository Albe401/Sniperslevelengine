//! Reference identity bridge. Legacy name/symbol inference is explicit, not a
//! vendor-certified put/call attribute or permission to publish trading signals.
use crate::{
    cme::{number, Error},
    cme_data::Instrument,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionMetadata {
    pub security_id: String,
    pub exchange: String,
    pub symbol: String,
    pub series_guid: String,
    pub strike: f64,
    pub side: String,
    pub strike_source: String,
    pub side_source: String,
    pub last_trade_date: String,
}
fn required(value: &Value, key: &str) -> Result<String, Error> {
    value[key]
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 256)
        .map(str::to_owned)
        .ok_or(Error::Protocol)
}
pub fn option_metadata(row: &Value) -> Result<Option<OptionMetadata>, Error> {
    let name = row["instrumentName"]
        .as_str()
        .unwrap_or("")
        .to_ascii_uppercase();
    let symbol = row["globexSymbol"].as_str().unwrap_or("");
    let mut sides = std::collections::BTreeSet::new();
    for word in name.split_whitespace() {
        if word == "CALL" {
            sides.insert("C");
        } else if word == "PUT" {
            sides.insert("P");
        }
    }
    for word in symbol.split_whitespace().skip(1) {
        if let Some(tail) = word.strip_prefix('C') {
            if tail.as_bytes().first().is_some_and(u8::is_ascii_digit) {
                sides.insert("C");
            }
        }
        if let Some(tail) = word.strip_prefix('P') {
            if tail.as_bytes().first().is_some_and(u8::is_ascii_digit) {
                sides.insert("P");
            }
        }
    }
    if sides.is_empty() {
        return Ok(None);
    }
    if sides.len() != 1 {
        return Err(Error::Protocol);
    }
    let official = number(&row["globexStrikePx"]);
    let words: Vec<_> = name.split_whitespace().collect();
    let fallback = words.windows(2).find_map(|pair| {
        if ["CALL", "PUT"].contains(&pair[1]) {
            pair[0].parse::<f64>().ok()
        } else {
            None
        }
    });
    let strike = official
        .or(fallback)
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or(Error::Protocol)?;
    let last_trade_date = required(row, "lastTradeDate")?;
    if snipers_core::calendar::day_bounds(&last_trade_date).is_none() {
        return Err(Error::Protocol);
    }
    Ok(Some(OptionMetadata {
        security_id: required(row, "globexSecurityId")?,
        exchange: required(row, "exchangeGlobex")?,
        symbol: required(row, "globexSymbol")?,
        series_guid: required(row, "seriesGuid")?,
        strike,
        side: sides.into_iter().next().unwrap().into(),
        strike_source: if official.is_some() {
            "GLOBEX_STRIKE_PX"
        } else {
            "LEGACY_NAME_INFERENCE"
        }
        .into(),
        side_source: "LEGACY_NAME_OR_SYMBOL_INFERENCE".into(),
        last_trade_date,
    }))
}
#[derive(Default)]
pub struct ReferenceMap {
    rows: BTreeMap<(String, String), OptionMetadata>,
}
impl ReferenceMap {
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    /// Replaces a catalog only after every row passes; duplicate/conflicting
    /// identities never overwrite a previous instrument silently.
    pub fn replace(&mut self, rows: &[Value]) -> Result<usize, Error> {
        if rows.len() > 50_000 {
            return Err(Error::Protocol);
        }
        let mut next = BTreeMap::new();
        for row in rows {
            if let Some(metadata) = option_metadata(row)? {
                let key = (metadata.exchange.clone(), metadata.security_id.clone());
                if next.insert(key, metadata).is_some() {
                    return Err(Error::Protocol);
                }
            }
        }
        self.rows = next;
        Ok(self.rows.len())
    }
    pub fn lookup(&self, instrument: &Instrument) -> Option<&OptionMetadata> {
        let result = self
            .rows
            .get(&(instrument.exchange.clone(), instrument.id.clone()))?;
        (instrument.product_type == "OOF" && instrument.symbol == result.symbol).then_some(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn row() -> Value {
        json!({"instrumentName":"NQZ6 21000 Call","globexSymbol":"NQZ6 C21000","globexStrikePx":"21000","globexSecurityId":"123","exchangeGlobex":"XCME","seriesGuid":"ABC123","lastTradeDate":"2026-12-18"})
    }
    #[test]
    fn bridge_marks_inference_and_matches_exchange_id_symbol_and_options() {
        let mut map = ReferenceMap::default();
        map.replace(&[row()]).unwrap();
        let mut instrument = Instrument {
            id: "123".into(),
            exchange: "XCME".into(),
            segment: "1".into(),
            symbol: "NQZ6 C21000".into(),
            product_code: "NQ".into(),
            product_type: "OOF".into(),
        };
        let meta = map.lookup(&instrument).unwrap();
        assert_eq!(meta.strike, 21000.0);
        assert_eq!(meta.side, "C");
        assert!(meta.side_source.contains("INFERENCE"));
        instrument.exchange = "XNYM".into();
        assert!(map.lookup(&instrument).is_none());
        instrument.exchange = "XCME".into();
        instrument.product_type = "FUT".into();
        assert!(map.lookup(&instrument).is_none());
        assert!(map.replace(&[row(), row()]).is_err());
        instrument.product_type = "OOF".into();
        assert!(map.lookup(&instrument).is_some());
    }
    #[test]
    fn incomplete_or_contradictory_metadata_cannot_be_certified() {
        let mut value = row();
        value["globexStrikePx"] = Value::Null;
        assert_eq!(
            option_metadata(&value).unwrap().unwrap().strike_source,
            "LEGACY_NAME_INFERENCE"
        );
        value["globexSymbol"] = json!("NQZ6 P21000");
        assert!(option_metadata(&value).is_err());
        value = row();
        value["lastTradeDate"] = json!("not-a-date");
        assert!(option_metadata(&value).is_err());
        assert!(option_metadata(&json!({"instrumentName":"NQ Future"}))
            .unwrap()
            .is_none());
    }
}
