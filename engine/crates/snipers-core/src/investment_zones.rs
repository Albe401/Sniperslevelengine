//! Daily price structure for investment research, independent of options OI.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Zones {
    pub supports: Vec<Zone>,
    pub resistances: Vec<Zone>,
    pub sessions: usize,
    pub atr: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    pub center: f64,
    pub low: f64,
    pub high: f64,
    pub pivots: usize,
    pub evidence: Vec<String>,
}

pub fn calculate(rows: &[&Value]) -> Zones {
    let mut out = Zones {
        sessions: rows.len(),
        ..Zones::default()
    };
    // Reject incomplete/malformed OHLC rather than silently bridging missing sessions.
    let bars = rows
        .iter()
        .map(|r| {
            let c = r["c"].as_f64()?;
            let h = r["h"].as_f64()?;
            let l = r["l"].as_f64()?;
            (c.is_finite() && h.is_finite() && l.is_finite() && l > 0. && h >= c && c >= l)
                .then_some((h, l, c))
        })
        .collect::<Option<Vec<_>>>();
    let Some(bars) = bars.filter(|b| b.len() >= 200) else {
        return out;
    };
    let closes = bars.iter().map(|b| b.2).collect::<Vec<_>>();
    let price = *closes.last().unwrap();
    let Some(atr) = crate::atr::sma14(&bars) else { return out; };
    out.atr = Some(atr);
    let tolerance = (atr * 0.35).max(price * 0.004);
    let half = tolerance * 0.55;
    let means = [
        (
            "EMA 50 diaria",
            crate::investment_structure::ema(&closes, 50).unwrap(),
        ),
        (
            "EMA 200 diaria",
            crate::investment_structure::ema(&closes, 200).unwrap(),
        ),
        (
            "SMA 200 diaria",
            closes[closes.len() - 200..].iter().sum::<f64>() / 200.,
        ),
    ];
    for support in [true, false] {
        let start = bars.len().saturating_sub(260).max(3);
        let mut pivots = Vec::new();
        for i in start..bars.len() - 3 {
            let p = if support { bars[i].1 } else { bars[i].0 };
            // One pivot per plateau; only confirmed after three completed sessions.
            let confirmed = bars[i - 3..i]
                .iter()
                .all(|b| if support { p < b.1 } else { p > b.0 })
                && bars[i + 1..=i + 3]
                    .iter()
                    .all(|b| if support { p <= b.1 } else { p >= b.0 });
            if confirmed && (if support { p < price } else { p > price }) {
                pivots.push(p);
            }
        }
        pivots.sort_by(f64::total_cmp);
        let mut groups: Vec<Vec<f64>> = Vec::new();
        for p in pivots {
            if let Some(g) = groups.last_mut().filter(|g| p - g[0] <= tolerance) {
                g.push(p)
            } else {
                groups.push(vec![p]);
            }
        }
        let mut zones = groups
            .into_iter()
            .map(|g| {
                let center = g.iter().sum::<f64>() / g.len() as f64;
                let mut evidence = vec![format!("{} pivote(s) diario(s) confirmado(s)", g.len())];
                for (name, value) in means {
                    if (value - center).abs() <= tolerance {
                        evidence.push(name.into());
                    }
                }
                Zone {
                    center,
                    low: center - half,
                    high: center + half,
                    pivots: g.len(),
                    evidence,
                }
            })
            .collect::<Vec<_>>();
        // Nearby observed zones first; strength is explained, never fabricated.
        zones.sort_by(|a, b| {
            (a.center - price)
                .abs()
                .total_cmp(&(b.center - price).abs())
        });
        zones.truncate(3);
        if support {
            out.supports = zones
        } else {
            out.resistances = zones
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(v: &[Value]) -> Zones {
        calculate(&v.iter().collect::<Vec<_>>())
    }
    #[test]
    fn no_fabricated_levels_for_trend_or_insufficient_history() {
        let v = (0..220)
            .map(|i| serde_json::json!({"h":102.+i as f64,"l":100.+i as f64,"c":101.+i as f64}))
            .collect::<Vec<_>>();
        assert!(run(&v).supports.is_empty());
        assert!(run(&v).resistances.is_empty());
        assert!(run(&v[..199]).atr.is_none());
    }
    #[test]
    fn confirmed_pivots_and_no_future_confirmation() {
        let mut v = vec![serde_json::json!({"h":102.,"l":98.,"c":100.}); 220];
        v[205] = serde_json::json!({"h":102.,"l":90.,"c":100.});
        v[210] = serde_json::json!({"h":110.,"l":98.,"c":100.});
        v[219] = serde_json::json!({"h":102.,"l":80.,"c":100.});
        let z = run(&v);
        assert_eq!(z.supports.len(), 1);
        assert_eq!(z.supports[0].center, 90.);
        assert_eq!(z.resistances[0].center, 110.);
        assert!(z.supports[0].low < z.supports[0].high);
        v[0]["h"] = serde_json::json!(1.);
        assert!(run(&v).atr.is_none());
    }
}
