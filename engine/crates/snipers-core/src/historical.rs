//! Descriptive historical session activity; never feeds live scanner selection.
use crate::{
    bars::{self, Minute},
    calendar::{self, Day, Session},
};
use serde::Serialize;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub symbol: String,
    pub date: String,
    pub status: String,
    pub minute_count: usize,
    pub expected_minutes: usize,
    pub coverage: f64,
    pub open_first_minute: Option<f64>,
    pub close_last_minute: Option<f64>,
    pub session_change_pct: Option<f64>,
    pub session_high: Option<f64>,
    pub session_low: Option<f64>,
    pub share_volume: Option<u64>,
    pub notional_approx: Option<f64>,
    pub rvol: Option<f64>,
    pub rvol_status: String,
    pub entry_confirmed: bool,
    pub option_liquidity_verified: bool,
    pub source: &'static str,
}
pub fn session(date: &str, now: u64) -> Result<Session, &'static str> {
    let (start, _) = calendar::day_bounds(date).ok_or("Fecha histórica inválida")?;
    let requested = calendar::date_at(start).ok_or("Fecha histórica inválida")?;
    if Some(requested) >= calendar::date_at(now) {
        return Err("Selecciona una fecha anterior a hoy en Nueva York");
    }
    match calendar::schedule(requested) {
        Day::Open(s) => Ok(s),
        Day::Closed => Err("La fecha seleccionada no tiene sesión regular"),
        Day::Unknown => Err("Fecha fuera del calendario verificado 2024–2028"),
    }
}
fn valid(m: &Minute) -> bool {
    [m.o, m.h, m.l, m.c]
        .into_iter()
        .all(|n| n.is_finite() && n > 0.0)
        && m.v.is_finite()
        && m.v >= 0.0
        && m.l <= m.o.min(m.c)
        && m.h >= m.o.max(m.c)
        && m.t.is_multiple_of(60_000)
}
fn normalized<'a>(input: &'a [Minute], s: &Session) -> BTreeMap<u32, &'a Minute> {
    let mut out = BTreeMap::new();
    for m in input {
        if valid(m) && m.t >= s.open_ms && m.t < s.close_ms {
            out.insert(((m.t - s.open_ms) / 60_000) as u32, m);
        }
    }
    out
}
fn round(n: f64, digits: usize) -> f64 {
    format!("{n:.digits$}")
        .parse()
        .expect("finite historical statistic")
}
pub fn summarize(
    symbol: &str,
    date: &str,
    input: &[Minute],
    now: u64,
) -> Result<Read, &'static str> {
    if crate::universe::stock_asset(symbol).is_none() || input.len() > 50_000 {
        return Err("Activo o cantidad de velas no válidos");
    }
    let day = session(date, now)?;
    let expected = ((day.close_ms - day.open_ms) / 60_000) as usize;
    let minutes = normalized(input, &day);
    let coverage = minutes.len() as f64 / expected as f64;
    let mut out = Read {
        symbol: symbol.into(),
        date: date.into(),
        status: "SIN VELAS DE LA FECHA".into(),
        minute_count: minutes.len(),
        expected_minutes: expected,
        coverage: round(coverage, 3),
        open_first_minute: None,
        close_last_minute: None,
        session_change_pct: None,
        session_high: None,
        session_low: None,
        share_volume: None,
        notional_approx: None,
        rvol: None,
        rvol_status: "PENDIENTE DE CALCULAR".into(),
        entry_confirmed: false,
        option_liquidity_verified: false,
        source: "MASSIVE STOCKS 1M HISTORICO",
    };
    let (Some((first, opening)), Some((last, closing))) =
        (minutes.first_key_value(), minutes.last_key_value())
    else {
        return Ok(out);
    };
    if *first > 10 || (*last as usize) < expected.saturating_sub(10) || coverage < 0.80 {
        out.status = "COBERTURA INSUFICIENTE".into();
        return Ok(out);
    }
    let volume: f64 = minutes.values().map(|m| m.v).sum();
    let notional: f64 = minutes
        .values()
        .map(|m| m.v * m.vw.filter(|n| n.is_finite() && *n > 0.0).unwrap_or(m.c))
        .sum();
    let change = (closing.c / opening.o - 1.0) * 100.0;
    if !volume.is_finite()
        || volume >= u64::MAX as f64
        || !notional.is_finite()
        || !change.is_finite()
    {
        return Err("Desbordamiento en estadísticas históricas");
    }
    out.status = "HISTORICO OBSERVADO".into();
    out.open_first_minute = Some(round(opening.o, 4));
    out.close_last_minute = Some(round(closing.c, 4));
    out.session_change_pct = Some(round(change, 2));
    out.session_high = Some(round(
        minutes
            .values()
            .map(|m| m.h)
            .fold(f64::NEG_INFINITY, f64::max),
        4,
    ));
    out.session_low = Some(round(
        minutes.values().map(|m| m.l).fold(f64::INFINITY, f64::min),
        4,
    ));
    out.share_volume = Some(volume as u64);
    out.notional_approx = Some(round(notional, 2));
    Ok(out)
}
pub fn top_activity(rows: &[Read]) -> Vec<Read> {
    let mut rows: Vec<_> = rows
        .iter()
        .filter(|r| {
            r.status == "HISTORICO OBSERVADO"
                && r.share_volume.is_some_and(|n| n > 0)
                && r.notional_approx.is_some_and(|n| n.is_finite() && n >= 0.0)
        })
        .cloned()
        .collect();
    rows.sort_by(|a, b| {
        b.notional_approx
            .unwrap()
            .total_cmp(&a.notional_approx.unwrap())
            .then(a.symbol.cmp(&b.symbol))
    });
    rows.truncate(5);
    rows
}
pub fn relative_volume(
    read: &mut Read,
    target: &[Minute],
    history: &[(String, Vec<Minute>)],
    now: u64,
) {
    read.rvol = None;
    read.rvol_status = "SIN CINCO SESIONES COMPARABLES".into();
    if read.status != "HISTORICO OBSERVADO" {
        return;
    }
    let Ok(day) = session(&read.date, now) else {
        return;
    };
    let slot = ((day.close_ms - day.open_ms) / 60_000) as u32 - 2;
    let target = normalized(target, &day)
        .into_iter()
        .map(|(i, m)| (i, m.clone()))
        .collect();
    let mut dates = std::collections::BTreeSet::new();
    let mut ordered: Vec<_> = history.iter().filter(|(d, _)| d < &read.date).collect();
    ordered.sort_by(|a, b| b.0.cmp(&a.0));
    let previous: Vec<_> = ordered
        .into_iter()
        .filter_map(|(date, bars)| {
            if !dates.insert(date) {
                return None;
            }
            let previous = session(date, now).ok()?;
            Some(
                normalized(bars, &previous)
                    .into_iter()
                    .map(|(i, m)| (i, m.clone()))
                    .collect(),
            )
        })
        .collect();
    if let Some(r) = bars::relative_volume(&target, &previous, slot) {
        read.rvol = Some(r.value);
        read.rvol_status = r.class.into();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    fn now() -> u64 {
        calendar::rfc3339_ms("2026-12-02T15:00:00Z").unwrap()
    }
    fn bars(date: &str) -> Vec<Minute> {
        let s = session(date, now()).unwrap();
        (0..((s.close_ms - s.open_ms) / 60_000))
            .map(|i| Minute {
                t: s.open_ms + i * 60_000,
                o: 100.0,
                h: 102.0,
                l: 99.0,
                c: 101.0,
                v: 100.0,
                vw: Some(100.5),
            })
            .collect()
    }
    #[test]
    fn statistics_match_twelve_legacy_sessions_and_coverage_edges() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-historical.json")).unwrap();
        for c in fixture["cases"].as_array().unwrap() {
            let bars: Vec<Minute> = serde_json::from_value(c["bars"].clone()).unwrap();
            let actual = summarize("QQQ", c["date"].as_str().unwrap(), &bars, now()).unwrap();
            let e = &c["expected"];
            assert_eq!(actual.status, e["status"].as_str().unwrap());
            assert_eq!(
                actual.minute_count,
                e["minute_count"].as_u64().unwrap() as usize
            );
            assert_eq!(actual.coverage, e["coverage"].as_f64().unwrap());
            if actual.status == "HISTORICO OBSERVADO" {
                assert_eq!(actual.notional_approx, e["notional_approx"].as_f64());
                assert_eq!(actual.share_volume, e["share_volume"].as_u64());
                assert_eq!(actual.session_change_pct, e["session_change_pct"].as_f64());
                assert_eq!(actual.session_high, e["session_high"].as_f64());
                assert_eq!(actual.session_low, e["session_low"].as_f64());
            }
        }
    }
    #[test]
    fn official_early_close_excludes_after_hours_and_dates_never_fall_back() {
        let mut input = bars("2026-11-27");
        let last = input.last().unwrap().clone();
        input.push(Minute {
            t: last.t + 60000,
            v: 99999999.0,
            ..last
        });
        let r = summarize("QQQ", "2026-11-27", &input, now()).unwrap();
        assert_eq!(r.expected_minutes, 210);
        assert_eq!(r.minute_count, 210);
        assert_eq!(r.share_volume, Some(21000));
        assert!(session("2026-11-26", now()).is_err());
        let historical = session("2025-09-23", now()).unwrap();
        assert_eq!((historical.close_ms - historical.open_ms) / 60_000, 390);
        assert!(session("2023-09-23", now()).is_err());
        assert!(session("2026-12-02", now()).is_err());
        assert!(summarize("SPX", "2026-11-27", &input, now()).is_err());
    }
    #[test]
    fn rvol_requires_five_distinct_prior_sessions_and_ranking_is_descriptive() {
        let input = bars("2026-09-23");
        let mut r = summarize("QQQ", "2026-09-23", &input, now()).unwrap();
        let mut baseline: Vec<_> = [
            "2026-09-22",
            "2026-09-21",
            "2026-09-18",
            "2026-09-17",
            "2026-09-16",
        ]
        .into_iter()
        .map(|d| (d.into(), bars(d)))
        .collect();
        relative_volume(&mut r, &input, &baseline, now());
        assert_eq!(r.rvol, Some(1.0));
        baseline[4] = baseline[3].clone();
        relative_volume(&mut r, &input, &baseline, now());
        assert_eq!(r.rvol, None);
        let mut b = r.clone();
        b.symbol = "AAPL".into();
        let ranked = top_activity(&[r, b]);
        assert_eq!(ranked[0].symbol, "AAPL");
        assert!(ranked
            .iter()
            .all(|r| !r.entry_confirmed && !r.option_liquidity_verified));
    }
}
