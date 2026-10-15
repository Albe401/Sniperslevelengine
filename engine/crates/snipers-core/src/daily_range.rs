//! One-day implied move. Model context, never an entry or a guaranteed target.
use crate::{
    calendar::{self, Day},
    options::Observation,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
pub struct Contract {
    pub ticker: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub strike: f64,
    pub expiration: String,
    pub iv: Option<f64>,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub ready: bool,
    pub method: Option<String>,
    pub spot: Option<f64>,
    pub nearest_expiration: Option<String>,
    pub atm_strike: Option<f64>,
    pub atm_iv: Option<f64>,
    pub expected_move_dollars: Option<f64>,
    pub expected_move_pct: Option<f64>,
    pub snapshot_lower: Option<f64>,
    pub snapshot_upper: Option<f64>,
    pub atm_straddle: Option<f64>,
    pub straddle_daily_equivalent: Option<f64>,
    pub iv_move_dollars: Option<f64>,
    pub iv_straddle_divergence_pct: Option<f64>,
    pub trading_days_to_expiration: Option<u32>,
    pub call_ticker: Option<String>,
    pub put_ticker: Option<String>,
    pub data_quality: String,
    pub notes: Vec<String>,
    pub entry_confirmed: bool,
    pub changes_structural_alignment: bool,
}
fn positive(n: Option<f64>) -> Option<f64> {
    n.filter(|v| v.is_finite() && *v > 0.0)
}
fn iv(c: Option<&Contract>) -> Option<f64> {
    c.and_then(|c| positive(c.iv)).filter(|v| *v <= 10.0)
}
fn midpoint(c: Option<&Contract>) -> Option<f64> {
    let c = c?;
    let (bid, ask) = positive(c.bid).zip(positive(c.ask))?;
    (ask >= bid).then_some(bid / 2.0 + ask / 2.0)
}
/// Excludes the starting date, includes expiration, minimum one model day.
/// Early closes count as one day. Outside the published calendar stays unknown.
fn sessions(start: NaiveDate, end: NaiveDate) -> Option<u32> {
    if end < start || (end - start).num_days() > 3660 {
        return None;
    }
    if matches!(calendar::schedule(start), Day::Unknown) {
        return None;
    }
    let mut d = start;
    let mut count = 0;
    while d < end {
        d = d.succ_opt()?;
        match calendar::schedule(d) {
            Day::Open(_) => count += 1,
            Day::Closed => {}
            Day::Unknown => return None,
        }
    }
    Some(count.max(1))
}
pub fn calculate(rows: &[Contract], spot: f64, as_of: NaiveDate) -> Result<Range, &'static str> {
    if rows.len() > 20_000 {
        return Err("Cadena excede capacidad del modelo de rango");
    }
    let mut result = Range {
        spot: positive(Some(spot)),
        data_quality: "WAITING".into(),
        ..Default::default()
    };
    if result.spot.is_none() {
        result.notes.push("SPOT NO DISPONIBLE".into());
        return Ok(result);
    }
    let expiration = rows
        .iter()
        .filter_map(|c| NaiveDate::parse_from_str(&c.expiration, "%Y-%m-%d").ok())
        .filter(|d| *d >= as_of)
        .min();
    let Some(expiration) = expiration else {
        result
            .notes
            .push("CADENA VACÍA O SIN VENCIMIENTO VIGENTE".into());
        return Ok(result);
    };
    let label = expiration.to_string();
    let mut indices = BTreeMap::new();
    let mut pairs: Vec<(f64, Option<&Contract>, Option<&Contract>)> = vec![];
    for c in rows.iter().filter(|c| {
        c.expiration == label
            && positive(Some(c.strike)).is_some()
            && matches!(c.kind.as_str(), "call" | "put")
    }) {
        let index = *indices.entry(c.strike.to_bits()).or_insert_with(|| {
            pairs.push((c.strike, None, None));
            pairs.len() - 1
        });
        if c.kind == "call" {
            pairs[index].1 = Some(c);
        } else {
            pairs[index].2 = Some(c);
        }
    }
    let distance = |strike: f64| (strike - spot).abs();
    let pair = pairs
        .iter()
        .filter(|(_, c, p)| c.is_some() && p.is_some())
        .min_by(|a, b| distance(a.0).total_cmp(&distance(b.0)));
    let (strike, call, put) = if let Some((s, c, p)) = pair {
        (Some(*s), *c, *p)
    } else {
        let nearest = |call: bool| {
            pairs
                .iter()
                .filter_map(|(_, c, p)| if call { *c } else { *p })
                .min_by(|a, b| distance(a.strike).total_cmp(&distance(b.strike)))
        };
        let c = nearest(true);
        let p = nearest(false);
        // An unmatched call/put is not an ATM straddle; retain only valid IV context.
        let strike = match (c, p) {
            (Some(c), None) => Some(c.strike),
            (None, Some(p)) => Some(p.strike),
            _ => None,
        };
        (strike, c, p)
    };
    result.nearest_expiration = Some(label);
    result.atm_strike = strike;
    result.call_ticker = call.map(|c| c.ticker.clone());
    result.put_ticker = put.map(|p| p.ticker.clone());
    let call_iv = iv(call);
    let put_iv = iv(put);
    let atm_iv = match (call_iv, put_iv) {
        (Some(c), Some(p)) => Some(c / 2.0 + p / 2.0),
        (c, p) => c.or(p),
    };
    let call_mid = midpoint(call);
    let put_mid = midpoint(put);
    let straddle = if pair.is_some() {
        call_mid.zip(put_mid).map(|(c, p)| c + p)
    } else {
        None
    };
    let days = sessions(as_of, expiration);
    let iv_move = atm_iv.map(|v| spot * v / 252f64.sqrt());
    let straddle_daily = straddle.zip(days).map(|(s, d)| s / f64::from(d).sqrt());
    let (expected, method) = if let Some(m) = iv_move {
        (Some(m), Some("ATM_IV_1D"))
    } else if let Some(m) = straddle_daily {
        (Some(m), Some("ATM_STRADDLE_DAILY_EQUIVALENT_FALLBACK"))
    } else {
        (None, None)
    };
    let divergence = iv_move
        .zip(straddle_daily)
        .map(|(i, s)| (i - s).abs() / i * 100.0);
    let pct = expected.map(|m| m / spot * 100.0);
    let lower = expected.map(|m| spot - m);
    let upper = expected.map(|m| spot + m);
    if [
        atm_iv,
        straddle,
        iv_move,
        straddle_daily,
        expected,
        divergence,
        pct,
        lower,
        upper,
    ]
    .into_iter()
    .flatten()
    .any(|n| !n.is_finite())
    {
        return Err("Desbordamiento en el modelo de rango");
    }
    if lower.is_some_and(|n| n <= 0.0) {
        return Err("Rango fuera del dominio de precios positivos");
    }
    if call_iv.is_none() {
        result.notes.push("IV CALL ATM NO DISPONIBLE".into());
    }
    if put_iv.is_none() {
        result.notes.push("IV PUT ATM NO DISPONIBLE".into());
    }
    if call_mid.is_none() {
        result.notes.push("MIDPOINT CALL ATM NO DISPONIBLE".into());
    }
    if put_mid.is_none() {
        result.notes.push("MIDPOINT PUT ATM NO DISPONIBLE".into());
    }
    if pair.is_none() {
        result
            .notes
            .push("SIN PAR CALL/PUT DEL MISMO STRIKE; STRADDLE NO CALCULADO".into());
    }
    if days.is_none() {
        result
            .notes
            .push("CALENDARIO FUERA DE COBERTURA; ESCALA STRADDLE NO DISPONIBLE".into());
    }
    result.ready = expected.is_some_and(|n| n > 0.0);
    result.data_quality = if !result.ready {
        "WAITING"
    } else if pair.is_some()
        && call_iv.is_some()
        && put_iv.is_some()
        && call_mid.is_some()
        && put_mid.is_some()
    {
        "HIGH"
    } else if pair.is_some() && call_iv.is_some() && put_iv.is_some() {
        "MEDIUM"
    } else {
        "LOW"
    }
    .into();
    result.method = method.map(str::to_owned);
    result.atm_iv = atm_iv;
    result.atm_straddle = straddle;
    result.expected_move_dollars = expected;
    result.expected_move_pct = pct;
    result.snapshot_lower = lower;
    result.snapshot_upper = upper;
    result.straddle_daily_equivalent = straddle_daily;
    result.iv_move_dollars = iv_move;
    result.iv_straddle_divergence_pct = divergence;
    result.trading_days_to_expiration = days;
    Ok(result)
}

/// Snapshot quotes need their own timestamp; current reception never refreshes old NBBO.
pub fn from_observation(observation: &Observation, spot: f64) -> Result<Range, &'static str> {
    let date = calendar::date_at(observation.observed_ms).ok_or("Fecha de observación inválida")?;
    let rows: Vec<_> = observation
        .contracts
        .values()
        .filter(|c| c.premium_multiplier == Some(100.0))
        .map(|c| {
            let fresh = c.quote_ms.is_some_and(|t| {
                t <= observation.observed_ms
                    && observation.observed_ms - t <= 60_000
                    && calendar::date_at(t) == Some(date)
            }) && c.quote_timeframe.as_deref() == Some("REAL-TIME");
            Contract {
                ticker: c.ticker.clone(),
                kind: c.kind.clone(),
                strike: c.strike,
                expiration: c.expiration.clone(),
                iv: c.iv,
                bid: if fresh { c.bid } else { None },
                ask: if fresh { c.ask } else { None },
            }
        })
        .collect();
    let mut result = calculate(&rows, spot, date)?;
    result.notes.push(
        "IV SIN TIMESTAMP DE PUBLICACIÓN; REFERENCIA DE SNAPSHOT, NO PREDICCIÓN GARANTIZADA".into(),
    );
    if rows.len() != observation.contracts.len() {
        result
            .notes
            .push("CONTRATOS SIN MULTIPLICADOR ESTÁNDAR VERIFICADO EXCLUIDOS".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_fallback_requires_verified_contracts_and_recent_realtime_nbbo() {
        use crate::options::normalize;
        let now = 1_790_260_200_000; // 2026-09-24 10:30 ET
        let contracts = [("call", "C"), ("put", "P")]
            .into_iter()
            .map(|(kind, letter)| {
                let mut c = normalize(
                    &serde_json::json!({
                        "details":{"ticker":format!("O:QQQ260924{letter}00100000"),
                        "expiration_date":"2026-09-24","contract_type":kind,"strike_price":100},
                        "last_quote":{"bid":2,"ask":4,"last_updated":now*1_000_000,
                        "timeframe":"REAL-TIME"}
                    }),
                    "QQQ",
                )
                .unwrap();
                c.premium_multiplier = Some(100.0);
                (c.ticker.clone(), c)
            })
            .collect();
        let observation = Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: now,
            observed_date: calendar::date_at(now).unwrap().to_string(),
            oi_reference_date: None,
            contracts,
        };
        assert_eq!(
            from_observation(&observation, 100.0)
                .unwrap()
                .expected_move_dollars,
            Some(6.0)
        );
        for timestamp in [None, Some(now + 1), Some(now - 60_001)] {
            let mut o = observation.clone();
            o.contracts.values_mut().next().unwrap().quote_ms = timestamp;
            assert!(!from_observation(&o, 100.0).unwrap().ready);
        }
        for timeframe in [None, Some("DELAYED"), Some("unknown")] {
            let mut o = observation.clone();
            o.contracts.values_mut().next().unwrap().quote_timeframe = timeframe.map(str::to_owned);
            assert!(!from_observation(&o, 100.0).unwrap().ready);
        }
        let mut o = observation.clone();
        for c in o.contracts.values_mut() {
            c.premium_multiplier = None;
        }
        assert!(!from_observation(&o, 100.0).unwrap().ready);
        let mut o = observation.clone();
        for c in o.contracts.values_mut() {
            c.quote_ms = None;
            c.iv = Some(0.2);
        }
        let range = from_observation(&o, 100.0).unwrap();
        assert!(range.ready);
        assert_eq!(range.method.as_deref(), Some("ATM_IV_1D"));
        assert_eq!(range.atm_straddle, None);
        assert_eq!(range.data_quality, "MEDIUM");
        let mut o = observation;
        for c in o.contracts.values_mut() {
            c.bid = Some(90.0);
            c.ask = Some(100.0);
        }
        assert!(from_observation(&o, 100.0).is_err());
    }
    #[test]
    fn thirteen_legacy_fixtures_match_move_selection_quality_and_bands() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-daily-range.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let rows: Vec<Contract> = serde_json::from_value(case["contracts"].clone()).unwrap();
            let date =
                NaiveDate::parse_from_str(case["date"].as_str().unwrap(), "%Y-%m-%d").unwrap();
            let actual = calculate(&rows, case["spot"].as_f64().unwrap(), date).unwrap();
            let expected = &case["expected"];
            assert_eq!(
                actual.ready,
                expected["ready"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
            assert_eq!(actual.method.as_deref(), expected["method"].as_str());
            assert_eq!(
                actual.data_quality,
                expected["data_quality"].as_str().unwrap()
            );
            for (a, k) in [
                (actual.atm_iv, "atm_iv"),
                (actual.atm_strike, "atm_strike"),
                (actual.expected_move_dollars, "expected_move_dollars"),
                (actual.expected_move_pct, "expected_move_pct"),
                (actual.snapshot_lower, "snapshot_lower"),
                (actual.snapshot_upper, "snapshot_upper"),
                (actual.atm_straddle, "atm_straddle"),
                (
                    actual.straddle_daily_equivalent,
                    "straddle_daily_equivalent",
                ),
                (
                    actual.iv_straddle_divergence_pct,
                    "iv_straddle_divergence_pct",
                ),
            ] {
                match (a, expected[k].as_f64()) {
                    (Some(a), Some(b)) => assert!((a - b).abs() < 1e-10, "{} {k}", case["name"]),
                    (None, None) => {}
                    _ => panic!("{} {k} availability", case["name"]),
                }
            }
            assert!(!actual.entry_confirmed && !actual.changes_structural_alignment);
        }
    }
    #[test]
    fn holidays_unknown_calendar_unmatched_strikes_and_overflow_are_explicit() {
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        assert_eq!(sessions(d("2026-09-04"), d("2026-09-08")), Some(1));
        assert_eq!(sessions(d("2028-12-29"), d("2029-01-02")), None);
        let mut rows = vec![
            Contract {
                ticker: "call".into(),
                kind: "call".into(),
                strike: 99.0,
                expiration: "2026-09-24".into(),
                iv: None,
                bid: Some(2.0),
                ask: Some(4.0),
            },
            Contract {
                ticker: "put".into(),
                kind: "put".into(),
                strike: 101.0,
                expiration: "2026-09-24".into(),
                iv: None,
                bid: Some(2.0),
                ask: Some(4.0),
            },
        ];
        let r = calculate(&rows, 100.0, d("2026-09-24")).unwrap();
        assert!(!r.ready);
        assert_eq!(r.atm_straddle, None);
        assert_eq!(r.atm_strike, None);
        rows[0].iv = Some(10.0);
        assert!(calculate(&rows, f64::MAX, d("2026-09-24")).is_err());
    }
}
