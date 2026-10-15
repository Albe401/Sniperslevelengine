//! Price gaps from regular-session OHLC. Never FVGs, order-book liquidity or fill probabilities.
use crate::calendar::{self, Day as Schedule};
use serde::{Deserialize, Serialize};
pub const VERSION: &str = "price-gap-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub complete: bool,
    pub ex_dividend: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gap {
    pub id: String,
    pub kind: String,
    pub born: String,
    pub origin: String,
    pub lower: f64,
    pub upper: f64,
    pub state: String,
    pub fill_pct: f64,
    pub remaining_lower: f64,
    pub remaining_upper: f64,
    pub age_sessions: usize,
    pub filled_at: Option<String>,
    pub reclaimed_at: Option<String>,
    pub verified: bool,
    pub ex_dividend: bool,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub version: String,
    pub symbol: String,
    pub phase: String,
    pub through: String,
    pub at: u64,
    pub coverage_pct: f64,
    pub actions_verified: bool,
    pub gaps: Vec<Gap>,
    pub detail: String,
}
pub fn sessions(now: u64, count: usize) -> Option<Vec<calendar::Session>> {
    let mut date = calendar::date_at(now)?;
    let mut out = Vec::new();
    for _ in 0..200 {
        match calendar::schedule(date) {
            Schedule::Open(s) if s.close_ms < now => out.push(s),
            Schedule::Unknown => return None,
            _ => {}
        }
        if out.len() == count {
            out.reverse();
            return Some(out);
        }
        date = date.pred_opt()?;
    }
    None
}
fn valid(d: &Day) -> bool {
    d.complete
        && [d.open, d.high, d.low, d.close]
            .iter()
            .all(|v| v.is_finite() && *v > 0.)
        && d.high >= d.open.max(d.close)
        && d.low <= d.open.min(d.close)
}
fn follow(g: &mut Gap, d: &Day) {
    if !valid(d) {
        g.verified = false;
        return;
    }
    if g.state == "FILLED" || g.state == "RECLAIMED" {
        if g.state == "FILLED"
            && g.filled_at.as_deref().is_some_and(|t| t < d.date.as_str())
            && if g.origin == "UP" {
                d.close >= g.upper
            } else {
                d.close <= g.lower
            }
        {
            g.state = "RECLAIMED".into();
            g.reclaimed_at = Some(d.date.clone());
        }
        return;
    }
    let width = g.upper - g.lower;
    let fill = if g.origin == "UP" {
        (g.upper - d.low) / width
    } else {
        (d.high - g.lower) / width
    };
    g.fill_pct = g.fill_pct.max((fill * 100.).clamp(0., 100.));
    if g.origin == "UP" {
        g.remaining_upper = g.upper - width * g.fill_pct / 100.;
    } else {
        g.remaining_lower = g.lower + width * g.fill_pct / 100.;
    }
    g.state = if g.fill_pct >= 100. {
        g.filled_at = Some(d.date.clone());
        "FILLED"
    } else if g.fill_pct > 0. {
        "PARTIAL"
    } else {
        "OPEN"
    }
    .into();
}
pub fn calculate(symbol: &str, days: &[Day], now: u64, actions_verified: bool) -> Report {
    let mut r = Report {
        version: VERSION.into(),
        symbol: symbol.into(),
        phase: "pending".into(),
        at: now,
        actions_verified,
        detail: "Historial RTH insuficiente; no se fabrican gaps.".into(),
        ..Default::default()
    };
    if days.len() < 2 || days.len() > 61 {
        return r;
    }
    let Some(expected) = sessions(now, days.len()) else {
        return r;
    };
    if days.iter().zip(&expected).any(|(d, s)| d.date != s.date) {
        r.detail = "Fechas no consecutivas, duplicadas, futuras o calendario incompleto.".into();
        return r;
    }
    r.through = days.last().unwrap().date.clone();
    r.coverage_pct = 100. * days.iter().filter(|d| valid(d)).count() as f64 / days.len() as f64;
    for i in 1..days.len() {
        let d = &days[i];
        for g in &mut r.gaps {
            g.age_sessions += 1;
            follow(g, d);
        }
        let p = &days[i - 1];
        if !valid(p) || !valid(d) {
            continue;
        }
        let mut add = |kind: &str, lower: f64, upper: f64, origin: &str, opening: bool| {
            if upper - lower < 0.01 {
                return;
            }
            let mut g = Gap {
                id: format!("{symbol}:{kind}:{}:{origin}", d.date),
                kind: kind.into(),
                born: d.date.clone(),
                origin: origin.into(),
                lower,
                upper,
                state: "OPEN".into(),
                fill_pct: 0.,
                remaining_lower: lower,
                remaining_upper: upper,
                age_sessions: 0,
                filled_at: None,
                reclaimed_at: None,
                verified: actions_verified,
                ex_dividend: d.ex_dividend,
            };
            if opening {
                follow(&mut g, d)
            }
            r.gaps.push(g);
        };
        if d.open > p.close {
            add("OPENING", p.close, d.open, "UP", true)
        } else if d.open < p.close {
            add("OPENING", d.open, p.close, "DOWN", true)
        }
        if d.low > p.high {
            add("SESSION_RANGE", p.high, d.low, "UP", false)
        } else if d.high < p.low {
            add("SESSION_RANGE", d.high, p.low, "DOWN", false)
        }
    }
    r.phase = if r.coverage_pct == 100. && actions_verified {
        "ready"
    } else {
        "partial"
    }
    .into();
    r.detail="Hasta 60 sesiones RTH cerradas; mínimo $0,01. OPENING = cierre previo/apertura; SESSION_RANGE = separación entre rangos completos, conocida solo al cierre. Exdividendo excluido de atracción; splits ajustados. Relleno confirmado solo con OHLC válido; datos faltantes suspenden verificación. RECLAIMED requiere una sesión posterior al relleno cerrada en el lado original. No implica liquidez pasiva ni probabilidad de cierre.".into();
    r
}
/// Current complete minute sequence updates the archive copy, without erasing historical fill.
pub fn update(gaps: &mut [Gap], date: &str, low: f64, high: f64, close: f64) {
    if ![low, high, close].iter().all(|v| v.is_finite() && *v > 0.) || low > close || high < close {
        return;
    }
    let d = Day {
        date: date.into(),
        open: close,
        high,
        low,
        close,
        complete: true,
        ex_dividend: false,
    };
    for g in gaps {
        if g.born.as_str() <= date && !matches!(g.state.as_str(), "FILLED" | "RECLAIMED") {
            follow(g, &d)
        }
    }
}
pub fn opening(r: &mut Report, date: &str, previous_close: f64, open: f64, ex_dividend: bool) {
    if ![previous_close, open]
        .iter()
        .all(|v| v.is_finite() && *v > 0.)
        || (previous_close - open).abs() < 0.01
    {
        return;
    }
    let id = format!(
        "{}:OPENING:{date}:{}",
        r.symbol,
        if open > previous_close { "UP" } else { "DOWN" }
    );
    if r.gaps.iter().any(|g| g.id == id) {
        return;
    }
    r.gaps.push(Gap {
        id,
        kind: "OPENING".into(),
        born: date.into(),
        origin: if open > previous_close { "UP" } else { "DOWN" }.into(),
        lower: open.min(previous_close),
        upper: open.max(previous_close),
        state: "OPEN".into(),
        fill_pct: 0.,
        remaining_lower: open.min(previous_close),
        remaining_upper: open.max(previous_close),
        age_sessions: 0,
        filled_at: None,
        reclaimed_at: None,
        verified: r.actions_verified,
        ex_dividend,
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (u64, Vec<Day>) {
        let n = calendar::rfc3339_ms("2026-10-06T08:00:00-04:00").unwrap();
        let ds = sessions(n, 4)
            .unwrap()
            .iter()
            .map(|s| Day {
                date: s.date.clone(),
                open: 100.,
                high: 101.,
                low: 99.,
                close: 100.,
                complete: true,
                ex_dividend: false,
            })
            .collect();
        (n, ds)
    }
    #[test]
    fn kinds_partial_fill_and_later_reclaim() {
        let (n, mut ds) = fixture();
        ds[1].open = 104.;
        ds[1].high = 105.;
        ds[1].low = 103.;
        ds[1].close = 104.;
        ds[2].open = 104.;
        ds[2].high = 105.;
        ds[2].low = 100.;
        ds[2].close = 103.;
        ds[3].open = 104.;
        ds[3].high = 106.;
        ds[3].low = 103.;
        ds[3].close = 105.;
        let r = calculate("QQQ", &ds, n, true);
        let g = &r.gaps[0];
        assert_eq!(g.state, "RECLAIMED");
        assert_eq!(g.fill_pct, 100.);
        assert_eq!(g.filled_at.as_deref(), Some(ds[2].date.as_str()));
        assert!(r.gaps.iter().any(|g| g.kind == "SESSION_RANGE"));
    }
    #[test]
    fn missing_duplicate_future_and_dividend_do_not_confirm() {
        let (n, mut ds) = fixture();
        ds[1].open = 104.;
        ds[1].high = 105.;
        ds[1].low = 103.;
        ds[1].close = 104.;
        ds[2].complete = false;
        assert!(calculate("QQQ", &ds, n, true)
            .gaps
            .iter()
            .filter(|g| g.born == ds[1].date)
            .all(|g| !g.verified));
        ds[2].complete = true;
        ds[1].ex_dividend = true;
        assert!(calculate("QQQ", &ds, n, true).gaps[0].ex_dividend);
        ds[1].date = ds[0].date.clone();
        assert_eq!(calculate("QQQ", &ds, n, true).phase, "pending");
    }
    #[test]
    fn partial_and_downside_are_symmetric() {
        let (n, mut ds) = fixture();
        for d in &mut ds[1..] {
            d.open = 96.;
            d.high = 97.;
            d.low = 95.;
            d.close = 96.;
        }
        let r = calculate("QQQ", &ds, n, true);
        assert_eq!(r.gaps[0].state, "PARTIAL");
        assert_eq!(r.gaps[0].fill_pct, 25.);
        assert_eq!(r.gaps[0].remaining_lower, 97.);
        assert_eq!(r.gaps[0].remaining_upper, 100.);
    }
    #[test]
    fn intraday_repeated_updates_are_idempotent() {
        let (n, mut ds) = fixture();
        for d in &mut ds[1..] {
            d.open = 104.;
            d.high = 105.;
            d.low = 103.;
            d.close = 104.;
        }
        let mut r = calculate("QQQ", &ds, n, true);
        let today = calendar::date_at(n).unwrap().to_string();
        update(&mut r.gaps, &today, 102., 104., 103.);
        let json = serde_json::to_string(&r.gaps).unwrap();
        update(&mut r.gaps, &today, 102., 104., 103.);
        assert_eq!(serde_json::to_string(&r.gaps).unwrap(), json);
        assert_eq!(r.gaps[0].fill_pct, 50.);
    }
    #[test]
    fn no_lookahead_or_unverified_actions() {
        let (n, ds) = fixture();
        assert_eq!(calculate("QQQ", &ds, n - 86400000, true).phase, "pending");
        assert_eq!(calculate("QQQ", &ds, n, false).phase, "partial");
    }
}
