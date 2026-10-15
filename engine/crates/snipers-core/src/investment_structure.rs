//! Research-only concentrations. OI does not identify institutions or guarantee support.
use crate::{investment::number, options};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub fn ema(closes: &[f64], period: usize) -> Option<f64> {
    if period == 0 || closes.len() < period || closes.iter().any(|v| !v.is_finite() || *v <= 0.) {
        return None;
    }
    let seed = closes[..period].iter().sum::<f64>() / period as f64;
    let alpha = 2. / (period as f64 + 1.);
    Some(
        closes[period..]
            .iter()
            .fold(seed, |ema, price| ema + alpha * (price - ema)),
    )
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    pub strike: f64,
    pub open_interest: f64,
    pub relative_strength: f64,
    pub strength: String,
    pub expirations: Vec<String>,
    pub contracts: usize,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Structure {
    pub supports: Vec<Level>,
    pub resistances: Vec<Level>,
    pub dominant_support: Option<Level>,
    pub dominant_resistance: Option<Level>,
    pub accepted: usize,
    pub rejected: usize,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Research {
    pub ticker: String,
    pub fetched_ms: u64,
    pub reference_price: f64,
    pub reference_date: String,
    pub expiration_from: String,
    pub expiration_to: String,
    pub chain_complete: bool,
    pub chain_rows: usize,
    pub structure: Structure,
    pub trade_date: String,
    pub session: String,
    pub trade_complete: bool,
    #[serde(default = "default_true")]
    pub trades_requested: bool,
    pub examined_trades: usize,
    pub from_ms: u64,
    pub through_ms: u64,
    pub minimum_notional: f64,
    pub blocks: Vec<Block>,
    pub errors: Vec<String>,
}
fn default_true() -> bool {
    true
}
pub fn levels(rows: &[Value], symbol: &str, price: f64, from: &str, through: &str) -> Structure {
    let mut out = Structure::default();
    if !price.is_finite() || price <= 0. {
        return out;
    }
    let mut strikes: BTreeMap<(bool, u64), Level> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        let Ok(c) = options::normalize(row, symbol) else {
            out.rejected += 1;
            continue;
        };
        if c.expiration.as_str() < from
            || c.expiration.as_str() > through
            || c.shares_per_contract != Some(100.)
            || options::underlying(&c.ticker) != Some(symbol)
        {
            out.rejected += 1;
            continue;
        }
        if !seen.insert(c.ticker) {
            continue;
        }
        let Some(oi) = c.open_interest.filter(|v| *v > 0.) else {
            out.rejected += 1;
            continue;
        };
        out.accepted += 1;
        let support = c.kind == "put" && c.strike <= price;
        let resistance = c.kind == "call" && c.strike >= price;
        if !support && !resistance {
            continue;
        }
        let l = strikes
            .entry((support, c.strike.to_bits()))
            .or_insert(Level {
                strike: c.strike,
                open_interest: 0.,
                relative_strength: 0.,
                strength: String::new(),
                expirations: Vec::new(),
                contracts: 0,
            });
        l.open_interest += oi;
        l.contracts += 1;
        if !l.expirations.contains(&c.expiration) {
            l.expirations.push(c.expiration);
        }
    }
    for support in [true, false] {
        let mut ls = strikes
            .iter()
            .filter(|((side, _), _)| *side == support)
            .map(|(_, l)| l.clone())
            .collect::<Vec<_>>();
        let max = ls
            .iter()
            .map(|l| l.open_interest)
            .reduce(f64::max)
            .unwrap_or(1.);
        for l in &mut ls {
            l.relative_strength = 100. * l.open_interest / max;
            l.strength = if l.relative_strength >= 80. {
                "Extremo"
            } else if l.relative_strength >= 50. {
                "Alto"
            } else if l.relative_strength >= 20. {
                "Medio"
            } else {
                "Bajo"
            }
            .into();
            l.expirations.sort();
        }
        ls.sort_by(|a, b| {
            b.open_interest.total_cmp(&a.open_interest).then_with(|| {
                (a.strike - price)
                    .abs()
                    .total_cmp(&(b.strike - price).abs())
            })
        });
        let dominant = ls.first().cloned();
        ls.truncate(3);
        // Rank 1 is nearest among the three strongest observed concentrations.
        ls.sort_by(|a, b| {
            (a.strike - price)
                .abs()
                .total_cmp(&(b.strike - price).abs())
        });
        if support {
            out.supports = ls;
            out.dominant_support = dominant;
        } else {
            out.resistances = ls;
            out.dominant_resistance = dominant;
        }
    }
    out
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    #[serde(default)]
    pub id: String,
    pub price: f64,
    pub shares: f64,
    pub notional: f64,
    pub execution_ms: Option<u64>,
    pub sip_ms: u64,
    pub trf_ms: Option<u64>,
    pub trf_id: u64,
    pub conditions: Vec<u64>,
    pub nbbo: String,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub quote_ms: Option<u64>,
}
pub fn blocks(rows: &[Value], from: u64, to: u64, minimum: f64) -> Vec<Block> {
    let mut out = daily_blocks(rows, from, to, minimum);
    out.truncate(12);
    out
}
pub fn daily_blocks(rows: &[Value], from: u64, to: u64, minimum: f64) -> Vec<Block> {
    let identity = |v: &Value| format!("{}:{}:{}", v["exchange"], v["trf_id"], v["id"]);
    let corrected = rows
        .iter()
        .filter(|v| v["correction"].as_u64().is_some_and(|n| n != 0))
        .map(identity)
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for r in rows {
        let key = identity(r);
        if r["exchange"] != 4
            || r["trf_id"].as_u64().is_none()
            || r["id"].as_str().is_none_or(str::is_empty)
            || corrected.contains(&key)
            || !seen.insert(key)
        {
            continue;
        }
        let Some(price) = number(r, "price").filter(|n| *n > 0.) else {
            continue;
        };
        let shares = if r.get("decimal_size").is_some_and(|v| !v.is_null()) {
            r["decimal_size"]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
        } else {
            number(r, "size")
        };
        let Some(shares) = shares.filter(|n| n.is_finite() && *n > 0.) else {
            continue;
        };
        let notional = price * shares;
        if !notional.is_finite() || notional < minimum {
            continue;
        }
        let Some(sip) = r["sip_timestamp"]
            .as_u64()
            .map(|t| t / 1_000_000)
            .filter(|t| *t >= from && *t < to)
        else {
            continue;
        };
        let execution = r["participant_timestamp"]
            .as_u64()
            .map(|t| t / 1_000_000)
            .filter(|t| *t > 0 && *t <= sip);
        out.push(Block {
            id: r["id"].as_str().unwrap().into(),
            price,
            shares,
            notional,
            execution_ms: execution,
            sip_ms: sip,
            trf_ms: r["trf_timestamp"].as_u64().map(|t| t / 1_000_000),
            trf_id: r["trf_id"].as_u64().unwrap(),
            conditions: r["conditions"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_u64).collect())
                .unwrap_or_default(),
            nbbo: "Sin NBBO verificable".into(),
            bid: None,
            ask: None,
            quote_ms: None,
        });
    }
    out.sort_by(|a, b| b.notional.total_cmp(&a.notional));
    out
}
pub fn attach_quote(block: &mut Block, q: &Value) {
    let Some(t) = block.execution_ms else {
        return;
    };
    let Some(qt) = q["sip_timestamp"]
        .as_u64()
        .map(|t| t / 1_000_000)
        .filter(|qt| *qt <= t && t - *qt <= 2000)
    else {
        return;
    };
    let (Some(bid), Some(ask)) = (number(q, "bid_price"), number(q, "ask_price")) else {
        return;
    };
    if bid <= 0. || ask <= bid {
        return;
    }
    block.bid = Some(bid);
    block.ask = Some(ask);
    block.quote_ms = Some(qt);
    // NBBO position is an observable price comparison even for contingent prints.
    // Whether it supports an aggressor inference is a separate decision in the UI.
    block.nbbo = if block.price > ask {
        "Sobre ask"
    } else if block.price == ask {
        "En ask"
    } else if block.price < bid {
        "Bajo bid"
    } else if block.price == bid {
        "En bid"
    } else {
        "Entre bid y ask"
    }
    .into();
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn ema_seed_and_recursive() {
        assert_eq!(ema(&[1., 2., 3., 4.], 3), Some(3.));
        assert_eq!(ema(&[2.; 200], 200), Some(2.));
        assert_eq!(ema(&[1.; 199], 200), None);
        assert_eq!(ema(&[f64::NAN; 200], 200), None);
    }
    #[test]
    fn supports_are_below_and_dominance_is_not_proximity() {
        let row = |strike: f64, oi: f64, kind: &str| json!({"details":{"ticker":format!("O:ABC270115{}{:08}",if kind=="put"{"P"}else{"C"},(strike*1000.)as u64),"expiration_date":"2027-01-15","strike_price":strike,"contract_type":kind,"shares_per_contract":100},"open_interest":oi});
        let rows = vec![
            row(90., 100., "put"),
            row(95., 60., "put"),
            row(110., 500., "put"),
            row(105., 90., "call"),
            row(80., 1000., "call"),
        ];
        let s = levels(&rows, "ABC", 100., "2026-09-26", "2027-09-26");
        assert_eq!(s.supports[0].strike, 95.);
        assert_eq!(s.dominant_support.unwrap().strike, 90.);
        assert_eq!(s.resistances.len(), 1);
        assert!(levels(&rows, "XYZ", 100., "2026-09-26", "2027-09-26")
            .supports
            .is_empty());
    }
    #[test]
    fn daily_large_blocks_are_not_truncated_and_include_threshold() {
        let rows=(0..20).map(|i|json!({"exchange":4,"trf_id":1,"id":format!("b{i}"),"price":100.,"size":100000.,"sip_timestamp":1000000000000u64,"participant_timestamp":999000000000u64})).collect::<Vec<_>>();
        assert_eq!(daily_blocks(&rows,900000,1100000,10_000_000.).len(),20);
        assert_eq!(blocks(&rows,900000,1100000,10_000_000.).len(),12);
        assert!(daily_blocks(&rows,900000,1100000,10_000_001.).is_empty());
    }
    #[test]
    fn blocks_dedupe_corrections_and_quote_time() {
        let r = json!({"exchange":4,"trf_id":1,"id":"a","price":100.,"size":10000.,"sip_timestamp":1000000000000u64,"participant_timestamp":999000000000u64});
        let mut bs = blocks(&[r.clone(), r.clone()], 900000, 1100000, 100000.);
        assert_eq!(bs.len(), 1);
        attach_quote(
            &mut bs[0],
            &json!({"sip_timestamp":1000000000000u64,"bid_price":99.,"ask_price":100.}),
        );
        assert!(bs[0].quote_ms.is_none());
        attach_quote(
            &mut bs[0],
            &json!({"sip_timestamp":998000000000u64,"bid_price":99.,"ask_price":100.}),
        );
        assert_eq!(bs[0].nbbo, "En ask");
        let mut correction = r.clone();
        correction["correction"] = json!(1);
        assert!(blocks(&[r, correction], 900000, 1100000, 100000.).is_empty());
    }
    #[test]
    fn contingent_print_retains_its_observable_nbbo_position() {
        let row=json!({"exchange":4,"trf_id":202,"id":"smh","price":623.38,"size":160000,"sip_timestamp":1000000000000u64,"participant_timestamp":999000000000u64,"conditions":[41,52,53]});
        let mut block=daily_blocks(&[row],900000,1100000,100000.).remove(0);
        attach_quote(&mut block,&json!({"sip_timestamp":998949000000u64,"bid_price":623.04,"ask_price":623.15}));
        assert_eq!(block.nbbo,"Sobre ask");
        assert_eq!(block.conditions,vec![41,52,53]);
    }
}
