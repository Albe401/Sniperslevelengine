//! Canonical REST/WS identity and provenance; never infer an ATS from a TRF code.
use crate::{
    calendar,
    off_exchange::{self, Print, Quote, Side},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Source {
    Live,
    History,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub symbol: String,
    pub price: f64,
    pub shares: f64,
    pub sip_ms: u64,
    pub participant_ms: Option<u64>,
    pub trf_ms: Option<u64>,
    pub exchange: u32,
    pub trf_id: u32,
    pub trade_id: Option<String>,
    pub sequence: Option<u64>,
    pub conditions: Vec<u32>,
    pub tape: Option<u32>,
    pub correction: Option<u32>,
    pub source: Source,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Identity {
    Id {
        symbol: String,
        date: String,
        exchange: u32,
        trf: u32,
        id: String,
    },
    Sequence {
        symbol: String,
        date: String,
        sequence: u64,
    },
}
pub fn normalize(row: &Value, symbol: &str, source: Source) -> Result<Option<Trade>, &'static str> {
    if crate::universe::stock_asset(symbol).is_none() {
        return Err("Activo no habilitado");
    }
    let live = source == Source::Live;
    if live && (row["ev"] != "T" || row["sym"] != symbol) {
        return Err("Evento de otro activo/canal");
    }
    let field = |rest: &str, ws: &str| row.get(if live { ws } else { rest });
    let integer = |rest: &str, ws: &str| -> Result<Option<u32>, &'static str> {
        field(rest, ws)
            .filter(|v| !v.is_null())
            .map(|v| {
                v.as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or("Metadata numérica inválida")
            })
            .transpose()
    };
    let Some(exchange) = integer("exchange", "x")? else {
        return Err("Exchange ausente");
    };
    if exchange != 4 {
        return Ok(None);
    }
    let Some(trf_id) = integer("trf_id", "trfi")? else {
        return Ok(None);
    };
    let number = |rest: &str, ws: &str| {
        field(rest, ws)
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite() && *v > 0.0)
    };
    let price = number("price", "p").ok_or("Precio inválido")?;
    // Decimal size is authoritative when provided, including its fractional component.
    let shares = match field("decimal_size", "ds").filter(|v| !v.is_null()) {
        Some(v) => v
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|n| n.is_finite() && *n > 0.0)
            .ok_or("Tamaño decimal inválido")?,
        None => number("size", "s").ok_or("Tamaño ausente o inválido")?,
    };
    if !(price * shares).is_finite() {
        return Err("Nocional fuera de rango");
    }
    let timestamp = |rest: &str, ws: &str| -> Result<Option<u64>, &'static str> {
        field(rest, ws)
            .filter(|v| !v.is_null())
            .map(|v| {
                let t = v.as_u64().filter(|t| *t > 0).ok_or("Timestamp inválido")?;
                let ms = if live { t } else { t / 1_000_000 };
                calendar::date_at(ms).ok_or("Fecha fuera de rango")?;
                Ok(ms)
            })
            .transpose()
    };
    let conditions = match field("conditions", "c").filter(|v| !v.is_null()) {
        Some(v) => {
            let a = v
                .as_array()
                .filter(|a| a.len() <= 64)
                .ok_or("Condiciones inválidas")?;
            a.iter()
                .map(|v| {
                    v.as_u64()
                        .and_then(|v| u32::try_from(v).ok())
                        .ok_or("Condición inválida")
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        None => vec![],
    };
    let trade_id = match field("id", "i").filter(|v| !v.is_null()) {
        Some(v) => Some(
            v.as_str()
                .filter(|s| !s.is_empty() && s.len() <= 256)
                .ok_or("Identidad inválida")?
                .into(),
        ),
        None => None,
    };
    let sequence = field("sequence_number", "q")
        .filter(|v| !v.is_null())
        .map(|v| v.as_u64().ok_or("Secuencia inválida"))
        .transpose()?;
    Ok(Some(Trade {
        symbol: symbol.into(),
        price,
        shares,
        sip_ms: timestamp("sip_timestamp", "t")?.ok_or("SIP timestamp ausente")?,
        participant_ms: timestamp("participant_timestamp", "pt")?,
        trf_ms: timestamp("trf_timestamp", "trft")?,
        exchange,
        trf_id,
        trade_id,
        sequence,
        conditions,
        tape: integer("tape", "z")?,
        correction: integer("correction", "correction")?,
        source,
    }))
}
impl Trade {
    pub fn identities(&self) -> Vec<Identity> {
        let Some(date) = calendar::date_at(self.sip_ms).map(|d| d.to_string()) else {
            return vec![];
        };
        let mut keys = vec![];
        if let Some(id) = &self.trade_id {
            keys.push(Identity::Id {
                symbol: self.symbol.clone(),
                date: date.clone(),
                exchange: self.exchange,
                trf: self.trf_id,
                id: id.clone(),
            });
        }
        if let Some(sequence) = self.sequence {
            keys.push(Identity::Sequence {
                symbol: self.symbol.clone(),
                date,
                sequence,
            });
        }
        keys
    }
    pub fn to_print(&self, quote: Option<Quote>) -> Option<Print> {
        if self.correction.is_some_and(|c| c != 0) {
            return None;
        }
        let execution = self.participant_ms.unwrap_or(self.sip_ms);
        // A late report is compared to execution time, never to a newer current quote.
        let side = if self.source == Source::Live {
            off_exchange::classify(self.price, execution, quote)
        } else {
            Side::Unknown
        };
        let time = chrono::DateTime::from_timestamp_millis(i64::try_from(self.sip_ms).ok()?)?
            .with_timezone(&chrono_tz::America::New_York)
            .format("%H:%M:%S")
            .to_string();
        Some(Print {
            price: self.price,
            shares: self.shares,
            notional: self.price * self.shares,
            level: off_exchange::bucket(self.price)?,
            side,
            time_et: time,
        })
    }
    /// Missing optional provider fields can differ across REST and WS.
    pub fn compatible(&self, other: &Self) -> bool {
        self.symbol == other.symbol
            && self.sip_ms == other.sip_ms
            && self.price == other.price
            && self.shares == other.shares
            && self.exchange == other.exchange
            && self.trf_id == other.trf_id
            && self.conditions == other.conditions
            && self
                .trade_id
                .as_ref()
                .zip(other.trade_id.as_ref())
                .is_none_or(|(a, b)| a == b)
            && self
                .sequence
                .zip(other.sequence)
                .is_none_or(|(a, b)| a == b)
            && self
                .participant_ms
                .zip(other.participant_ms)
                .is_none_or(|(a, b)| a == b)
            && self.trf_ms.zip(other.trf_ms).is_none_or(|(a, b)| a == b)
            && self.tape.zip(other.tape).is_none_or(|(a, b)| a == b)
            && self.correction.unwrap_or(0) == other.correction.unwrap_or(0)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn rest() -> Value {
        json!({"price":500,"size":200,"decimal_size":"200.25","exchange":4,"trf_id":12,"id":"shared","sequence_number":7,"sip_timestamp":1790258400000123456u64,"participant_timestamp":1790258399000123456u64,"conditions":[0,12]})
    }
    fn live() -> Value {
        json!({"ev":"T","sym":"QQQ","p":500,"s":200,"ds":"200.25","x":4,"trfi":12,"i":"shared","q":7,"t":1790258400000u64,"pt":1790258399000u64,"c":[0,12]})
    }
    #[test]
    fn rest_and_ws_match_after_unit_and_fractional_size_normalization() {
        let a = normalize(&rest(), "QQQ", Source::History).unwrap().unwrap();
        let b = normalize(&live(), "QQQ", Source::Live).unwrap().unwrap();
        assert!(a.compatible(&b));
        assert_eq!(a.identities(), b.identities());
        assert_eq!(a.shares, 200.25);
        let quote = Some(Quote {
            bid: 499.0,
            ask: 500.0,
            timestamp_ms: b.participant_ms.unwrap(),
        });
        assert_eq!(a.to_print(quote).unwrap().side, Side::Unknown);
        assert_eq!(b.to_print(quote).unwrap().side, Side::Buy);
        let later = Some(Quote {
            bid: 499.0,
            ask: 500.0,
            timestamp_ms: b.sip_ms,
        });
        assert_eq!(b.to_print(later).unwrap().side, Side::Unknown);
    }
    #[test]
    fn trf_identity_scoping_and_conflicts_are_explicit() {
        let a = normalize(&rest(), "QQQ", Source::History).unwrap().unwrap();
        let mut b = a.clone();
        b.trf_id = 13;
        b.sequence = None;
        assert!(!a.identities().contains(&b.identities()[0]));
        assert!(!a.compatible(&b));
        b = a.clone();
        b.sip_ms += 86_400_000;
        assert!(a
            .identities()
            .iter()
            .all(|key| !b.identities().contains(key)));
        b = a.clone();
        b.correction = Some(1);
        assert!(b.to_print(None).is_none());
        assert!(!a.compatible(&b));
        b = a.clone();
        b.price = 501.0;
        assert!(!a.compatible(&b));
    }
    #[test]
    fn foreign_lit_missing_trf_and_malformed_decimal_are_not_counted() {
        assert!(normalize(&live(), "SPY", Source::Live).is_err());
        let mut row = rest();
        row["exchange"] = json!(11);
        assert!(normalize(&row, "QQQ", Source::History).unwrap().is_none());
        row = rest();
        row.as_object_mut().unwrap().remove("trf_id");
        assert!(normalize(&row, "QQQ", Source::History).unwrap().is_none());
        row = rest();
        row["decimal_size"] = json!("NaN");
        assert!(normalize(&row, "QQQ", Source::History).is_err());
    }
}
