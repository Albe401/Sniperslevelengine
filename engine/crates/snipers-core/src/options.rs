//! Snapshot observations retain field provenance: OI is not a tick-level quantity.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractSnapshot {
    pub ticker: String,
    pub expiration: String,
    pub strike: f64,
    pub kind: String,
    pub open_interest: Option<f64>,
    pub volume: Option<f64>,
    pub shares_per_contract: Option<f64>,
    /// Verified from the reference endpoint, never inferred from deliverable size alone.
    pub premium_multiplier: Option<f64>,
    pub delta: Option<f64>,
    pub gamma: Option<f64>,
    pub theta: Option<f64>,
    pub vega: Option<f64>,
    pub iv: Option<f64>,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub quote_ms: Option<u64>,
    pub trade_ms: Option<u64>,
    pub underlying_price: Option<f64>,
    pub underlying_ms: Option<u64>,
    pub quote_timeframe: Option<String>,
    pub underlying_timeframe: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub schema: u32,
    pub symbol: String,
    pub observed_ms: u64,
    pub observed_date: String,
    /// The chain API documents prior-session OI but supplies no per-contract OI timestamp.
    pub oi_reference_date: Option<String>,
    pub contracts: BTreeMap<String, ContractSnapshot>,
}
pub fn underlying(ticker: &str) -> Option<&str> {
    let raw = ticker.strip_prefix("O:")?;
    if !raw.is_ascii() || raw.len() <= 15 {
        return None;
    }
    let (root, tail) = raw.split_at(raw.len() - 15);
    if !tail[..6].bytes().all(|c| c.is_ascii_digit())
        || !b"CP".contains(&tail.as_bytes()[6])
        || !tail[7..].bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    // OCC/Massive option roots concatenate the class suffix, while stock
    // snapshots and the registry use BRK.B. Keep adjusted roots distinct.
    Some(match root {
        "BRKB" => "BRK.B",
        _ => root,
    })
}
fn finite(value: &Value) -> Option<f64> {
    value.as_f64().filter(|v| v.is_finite())
}
fn nonnegative(value: &Value) -> Option<f64> {
    finite(value).filter(|v| *v >= 0.0)
}
fn nanoseconds(value: &Value) -> Option<u64> {
    value.as_u64().filter(|t| *t > 0).map(|t| t / 1_000_000)
}
pub fn verified_multiplier(reference: &Value, symbol: &str) -> Option<f64> {
    let no_extra = reference["additional_underlyings"].is_null()
        || reference["additional_underlyings"]
            .as_array()
            .is_some_and(|a| a.is_empty());
    let root = reference["ticker"].as_str().and_then(underlying);
    let identity = if symbol == "SPX" {
        matches!(root, Some("SPX" | "SPXW"))
            && reference["exercise_style"] == "european"
            && reference["primary_exchange"] == "XCBO"
    } else {
        root == Some(symbol)
    };
    (reference["underlying_ticker"] == symbol
        && identity
        && reference["shares_per_contract"].as_f64() == Some(100.0)
        && no_extra)
        .then_some(100.0)
}
pub fn normalize(row: &Value, symbol: &str) -> Result<ContractSnapshot, &'static str> {
    let d = &row["details"];
    let ticker = d["ticker"].as_str().ok_or("Contrato sin ticker")?;
    let root = underlying(ticker).ok_or("Ticker de opciones inválido")?;
    if symbol == "SPX" {
        if !matches!(root, "SPX" | "SPXW")
            || row["underlying_asset"]["ticker"]
                .as_str()
                .is_some_and(|s| !matches!(s, "SPX" | "I:SPX"))
        {
            return Err("Identidad de índice SPX inconsistente");
        }
        let raw = ticker.strip_prefix("O:").ok_or("Ticker SPX inválido")?;
        let tail = &raw[raw.len() - 15..];
        let expiry = format!("20{}-{}-{}", &tail[..2], &tail[2..4], &tail[4..6]);
        let strike = tail[7..]
            .parse::<u64>()
            .map_err(|_| "Strike SPX inválido")? as f64
            / 1000.0;
        let kind = if &tail[6..7] == "C" { "call" } else { "put" };
        if d["expiration_date"] != expiry
            || d["contract_type"] != kind
            || d["strike_price"].as_f64() != Some(strike)
        {
            return Err("Metadata SPX no coincide con el contrato");
        }
    } else if root != symbol && row["underlying_asset"]["ticker"] != symbol {
        return Err("Contrato de otro subyacente");
    }
    if row["underlying_asset"]["ticker"]
        .as_str()
        .is_some_and(|s| s != symbol && !(symbol == "SPX" && s == "I:SPX"))
    {
        return Err("Subyacente inconsistente");
    }
    let expiration = d["expiration_date"]
        .as_str()
        .ok_or("Contrato sin vencimiento")?;
    chrono::NaiveDate::parse_from_str(expiration, "%Y-%m-%d")
        .map_err(|_| "Vencimiento inválido")?;
    let kind = d["contract_type"]
        .as_str()
        .filter(|k| ["call", "put", "other"].contains(k))
        .ok_or("Tipo de contrato inválido")?;
    let strike = finite(&d["strike_price"])
        .filter(|v| *v > 0.0)
        .ok_or("Strike inválido")?;
    let g = &row["greeks"];
    let q = &row["last_quote"];
    let u = &row["underlying_asset"];
    Ok(ContractSnapshot {
        ticker: ticker.into(),
        expiration: expiration.into(),
        strike,
        kind: kind.into(),
        open_interest: nonnegative(&row["open_interest"]),
        volume: nonnegative(&row["day"]["volume"]),
        shares_per_contract: nonnegative(&d["shares_per_contract"]),
        premium_multiplier: None,
        delta: finite(&g["delta"]).filter(|v| v.abs() <= 1.0),
        gamma: nonnegative(&g["gamma"]),
        theta: finite(&g["theta"]),
        vega: nonnegative(&g["vega"]),
        iv: nonnegative(&row["implied_volatility"]),
        bid: nonnegative(&q["bid"]),
        ask: nonnegative(&q["ask"]),
        quote_ms: nanoseconds(&q["last_updated"]),
        trade_ms: nanoseconds(&row["last_trade"]["sip_timestamp"]),
        underlying_price: finite(&u["price"]).filter(|v| *v > 0.0),
        underlying_ms: nanoseconds(&u["last_updated"]),
        quote_timeframe: q["timeframe"].as_str().map(str::to_owned),
        underlying_timeframe: u["timeframe"].as_str().map(str::to_owned),
    })
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OiChange {
    pub ticker: String,
    pub current: Option<f64>,
    pub previous: Option<f64>,
    pub change: Option<f64>,
    pub change_pct: Option<f64>,
    pub previous_observed_date: Option<String>,
}
pub fn changes(current: &Observation, history: &[Observation]) -> Vec<OiChange> {
    // Compare observation days, not timestamps of quotes/trades or assumed OI publication dates.
    let previous = history
        .iter()
        .filter(|h| h.symbol == current.symbol && h.observed_date < current.observed_date)
        .max_by_key(|h| h.observed_ms);
    current
        .contracts
        .values()
        .map(|contract| {
            let old = previous
                .and_then(|h| h.contracts.get(&contract.ticker))
                .and_then(|c| c.open_interest);
            let change = contract.open_interest.zip(old).map(|(new, old)| new - old);
            OiChange {
                ticker: contract.ticker.clone(),
                current: contract.open_interest,
                previous: old,
                change,
                change_pct: change
                    .zip(old)
                    .filter(|(_, old)| *old > 0.0)
                    .map(|(d, old)| d / old * 100.0),
                previous_observed_date: previous.map(|p| p.observed_date.clone()),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn berkshire_class_b_maps_only_its_standard_root() {
        assert_eq!(underlying("O:BRKB261002C00505000"), Some("BRK.B"));
        assert_eq!(underlying("O:BRKB261002P00505000"), Some("BRK.B"));
        assert_eq!(underlying("O:BRKB1261002C00505000"), Some("BRKB1"));
        let reference = serde_json::json!({
            "ticker": "O:BRKB261002C00505000",
            "underlying_ticker": "BRK.B",
            "shares_per_contract": 100
        });
        assert_eq!(verified_multiplier(&reference, "BRK.B"), Some(100.0));
        assert_eq!(verified_multiplier(&reference, "BRK.A"), None);
        assert_eq!(verified_multiplier(&reference, "AAPL"), None);
    }

    #[test]
    fn index_alias_requires_exact_spx_root_and_encoded_metadata() {
        let mut row = serde_json::json!({"details":{"ticker":"O:SPXW260928C06000000","expiration_date":"2026-09-28","contract_type":"call","strike_price":6000},"underlying_asset":{"ticker":"I:SPX"}});
        assert!(normalize(&row, "SPX").is_ok());
        assert!(normalize(&row, "SPY").is_err());
        row["details"]["strike_price"] = 6005.into();
        assert!(normalize(&row, "SPX").is_err());
        row["details"]["strike_price"] = 6000.into();
        row["details"]["ticker"] = "O:SPY260928C06000000".into();
        assert!(normalize(&row, "SPX").is_err());
        let mut reference = serde_json::json!({"ticker":"O:SPXW260928C06000000","underlying_ticker":"SPX","shares_per_contract":100,"exercise_style":"european","primary_exchange":"XCBO"});
        assert_eq!(verified_multiplier(&reference, "SPX"), Some(100.));
        reference["exercise_style"] = "american".into();
        assert_eq!(verified_multiplier(&reference, "SPX"), None);
        reference["exercise_style"] = "european".into();
        reference["additional_underlyings"] = serde_json::json!([{"type":"cash"}]);
        assert_eq!(verified_multiplier(&reference, "SPX"), None);
    }
    use super::*;
    fn row() -> Value {
        serde_json::json!({"details":{"ticker":"O:QQQ260925C00500000","expiration_date":"2026-09-25","strike_price":500,"contract_type":"call"},"last_quote":{"last_updated":1790172000123456789u64},"underlying_asset":{"ticker":"QQQ"}})
    }
    #[test]
    fn absent_oi_and_greeks_remain_missing_and_endpoint_nanoseconds_are_normalized() {
        let contract = normalize(&row(), "QQQ").unwrap();
        assert_eq!(contract.open_interest, None);
        assert_eq!(contract.gamma, None);
        assert_eq!(contract.quote_ms, Some(1_790_172_000_123));
        assert!(normalize(&row(), "SPY").is_err());
        let mut zero = row();
        zero["open_interest"] = serde_json::json!(0);
        assert_eq!(normalize(&zero, "QQQ").unwrap().open_interest, Some(0.0));
    }
    #[test]
    fn snapshot_multiplier_requires_standard_reference_metadata() {
        let mut reference = serde_json::json!({"ticker":"O:QQQ260925C00500000","underlying_ticker":"QQQ","shares_per_contract":100});
        assert_eq!(verified_multiplier(&reference, "QQQ"), Some(100.0));
        reference["additional_underlyings"] = serde_json::json!([{"type":"cash"}]);
        assert_eq!(verified_multiplier(&reference, "QQQ"), None);
        reference["additional_underlyings"] = serde_json::json!([]);
        reference["shares_per_contract"] = serde_json::json!(10);
        assert_eq!(verified_multiplier(&reference, "QQQ"), None);
        assert_eq!(normalize(&row(), "QQQ").unwrap().premium_multiplier, None);
    }
    #[test]
    fn daily_comparison_ignores_intraday_observation_and_preserves_unknown_baseline() {
        let mut contract = normalize(&row(), "QQQ").unwrap();
        contract.open_interest = Some(120.0);
        let current = Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: 2000,
            observed_date: "2026-09-24".into(),
            oi_reference_date: None,
            contracts: BTreeMap::from([(contract.ticker.clone(), contract)]),
        };
        let mut previous = current.clone();
        previous.observed_ms = 1000;
        previous.observed_date = "2026-09-23".into();
        previous
            .contracts
            .values_mut()
            .next()
            .unwrap()
            .open_interest = Some(100.0);
        let mut intraday = current.clone();
        intraday.observed_ms = 1500;
        intraday
            .contracts
            .values_mut()
            .next()
            .unwrap()
            .open_interest = Some(90.0);
        let result = changes(&current, &[previous, intraday]);
        assert_eq!(result[0].change, Some(20.0));
        assert_eq!(result[0].change_pct, Some(20.0));
        assert_eq!(changes(&current, &[])[0].change, None);
    }
    #[test]
    fn legacy_daily_oi_fixtures_match_with_explicit_unknown_baseline() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-oi.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let mut contract = normalize(&row(), "QQQ").unwrap();
            contract.open_interest = case["current"].as_f64();
            let current = Observation {
                schema: 1,
                symbol: "QQQ".into(),
                observed_ms: 100,
                observed_date: "2026-09-24".into(),
                oi_reference_date: None,
                contracts: BTreeMap::from([(contract.ticker.clone(), contract)]),
            };
            let history: Vec<_> = case["history"]
                .as_object()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(i, (date, values))| {
                    let mut old = current.clone();
                    old.observed_date = date.clone();
                    old.observed_ms = i as u64;
                    old.contracts.values_mut().next().unwrap().open_interest =
                        values["contract"].as_f64();
                    old
                })
                .collect();
            let result = changes(&current, &history).remove(0);
            assert_eq!(
                result.previous,
                case["expected"]["previous_oi"].as_f64(),
                "{}",
                case["name"]
            );
            let expected = result.previous.and(case["expected"]["oi_change"].as_f64());
            assert_eq!(result.change, expected, "{}", case["name"]);
            if result.previous == Some(0.0) {
                assert_eq!(result.change_pct, None);
            }
        }
    }
}
