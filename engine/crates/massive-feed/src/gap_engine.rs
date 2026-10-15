//! Isolated bounded RTH archive worker. No orders, FVGs or UI markup.
use crate::{
    credentials::Credentials,
    live::{now_ms, Bar, SharedState},
};
use serde::{Deserialize, Serialize};
use snipers_core::{
    calendar,
    gap_engine::{self as core, Day, Report},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::Duration,
};
#[derive(Clone, Serialize, Deserialize)]
struct Archive {
    version: String,
    symbol: String,
    through: String,
    fetched_date: String,
    days: Vec<Day>,
    actions_verified: bool,
    dividends: BTreeSet<String>,
}
fn aggregate(session: &calendar::Session, rows: &[Bar], dividends: &BTreeSet<String>) -> Day {
    let expected = ((session.close_ms - session.open_ms) / 60000) as usize;
    let complete = rows.len() == expected
        && rows.iter().enumerate().all(|(i, b)| {
            b.timestamp_ms == session.open_ms + i as u64 * 60000
                && [b.open, b.high, b.low, b.close, b.volume]
                    .iter()
                    .all(|v| v.is_finite())
                && b.low > 0.
                && b.volume >= 0.
                && b.high >= b.open.max(b.close)
                && b.low <= b.open.min(b.close)
        });
    Day {
        date: session.date.clone(),
        open: rows.first().map(|b| b.open).unwrap_or(0.),
        close: rows.last().map(|b| b.close).unwrap_or(0.),
        high: rows.iter().map(|b| b.high).fold(0., f64::max),
        low: rows
            .iter()
            .map(|b| b.low)
            .fold(f64::INFINITY, f64::min)
            .min(if rows.is_empty() { 0. } else { f64::INFINITY }),
        complete,
        ex_dividend: dividends.contains(&session.date),
    }
}
async fn actions(
    key: &Credentials,
    symbol: &str,
    from: &str,
    to: &str,
) -> Option<BTreeSet<String>> {
    let path = "/stocks/v1/dividends";
    let url=crate::history::checked_url(&format!("https://api.massive.com{path}?ticker={symbol}&ex_dividend_date.gte={from}&ex_dividend_date.lte={to}&limit=250"),path).ok()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;
    let mut response = client.get(url).bearer_auth(&key.0).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(c) = response.chunk().await.ok()? {
        if bytes.len() + c.len() > 1000000 {
            return None;
        }
        bytes.extend_from_slice(&c)
    }
    let page: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    if page["status"] != "OK" || page["next_url"].as_str().is_some_and(|s| !s.is_empty()) {
        return None;
    }
    let mut dates = BTreeSet::new();
    for row in page["results"].as_array()? {
        if row["ticker"] != symbol {
            return None;
        }
        let d = row["ex_dividend_date"].as_str()?;
        calendar::day_bounds(d)?;
        if d < from || d > to {
            return None;
        }
        dates.insert(d.into());
    }
    Some(dates)
}
fn cached(path: &Path, symbol: &str, through: &str, today: &str) -> Option<Archive> {
    let m = std::fs::metadata(path).ok()?;
    if m.len() > 200000 {
        return None;
    }
    let a: Archive = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    (a.version == core::VERSION
        && a.symbol == symbol
        && a.through == through
        && a.fetched_date == today
        && a.days.len() == 60
        && a.actions_verified
        && a.days.iter().all(|d| d.complete))
    .then_some(a)
}
async fn load(
    key: &Credentials,
    symbol: &str,
    now: u64,
    directory: Option<PathBuf>,
) -> Result<Archive, String> {
    if snipers_core::universe::stock_asset(symbol).is_none(){return Err("Activo no admitido para historial de gaps Stocks".into());}
    let sessions = core::sessions(now, 60).ok_or("Calendario para 60 sesiones pendiente")?;
    let today = calendar::date_at(now).ok_or("Fecha inválida")?.to_string();
    let through = sessions.last().unwrap().date.clone();
    let path = directory.map(|p| p.join("gap-engine").join(format!("{symbol}-v1.json")));
    if let Some(p) = path.clone() {
        let sym = symbol.to_string();
        let day = today.clone();
        let last = through.clone();
        if let Ok(Some(a)) =
            tokio::task::spawn_blocking(move || cached(&p, &sym, &last, &day)).await
        {
            return Ok(a);
        }
    }
    let div = actions(key, symbol, &sessions[0].date, &today).await;
    let actions_verified = div.is_some();
    let dividends = div.unwrap_or_default();
    let mut cursor = sessions[0].open_ms;
    let end = sessions.last().unwrap().close_ms - 1;
    let mut grouped: BTreeMap<String, Vec<Bar>> = BTreeMap::new();
    while cursor <= end {
        let to = (cursor + 21 * 86400000 - 1).min(end);
        let bars = crate::history::fetch(key, symbol, cursor, to)
            .await
            .map_err(str::to_string)?;
        for b in bars {
            let Some(d) = calendar::date_at(b.timestamp_ms) else {
                continue;
            };
            if let calendar::Day::Open(s) = calendar::schedule(d) {
                if b.timestamp_ms >= s.open_ms && b.timestamp_ms < s.close_ms {
                    grouped.entry(s.date).or_default().push(b)
                }
            }
        }
        cursor = to + 1;
    }
    let days = sessions
        .iter()
        .map(|s| {
            let mut rows = grouped.remove(&s.date).unwrap_or_default();
            rows.sort_by_key(|b| b.timestamp_ms);
            aggregate(s, &rows, &dividends)
        })
        .collect();
    let a = Archive {
        version: core::VERSION.into(),
        symbol: symbol.into(),
        through,
        fetched_date: today,
        days,
        actions_verified,
        dividends,
    };
    if let Some(p) = path {
        let copy = a.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let tmp = p.with_extension(format!("{}.{}.tmp",std::process::id(),now));
            std::fs::write(&tmp, serde_json::to_vec(&copy)?)?;
            if p.exists() {
                std::fs::remove_file(&p)?;
            }
            std::fs::rename(tmp, p)
        })
        .await;
    }
    Ok(a)
}
pub async fn worker(key: &Credentials, state: SharedState) {
    let mut cache: BTreeMap<String, (u64, Result<Archive, String>)> = BTreeMap::new();
    loop {
        let now = now_ms();
        let (symbols, directory) = {
            let s = state.lock().unwrap();
            { let mut symbols=vec![s.symbol.clone()]; if s.bot_enabled { for symbol in &s.bot_targets { if (!s.bot_qqq_only || symbol=="QQQ") && !symbols.contains(symbol){symbols.push(symbol.clone());} } } for symbol in &s.bot_held {if !symbols.contains(symbol){symbols.push(symbol.clone());}} (symbols,s.oi_directory.clone()) }
        };
        let today = calendar::date_at(now)
            .map(|d| d.to_string())
            .unwrap_or_default();
        let through = core::sessions(now, 1)
            .and_then(|v| v.last().map(|s| s.date.clone()))
            .unwrap_or_default();
        for symbol in symbols {
        let hit = cache
            .get(&symbol)
            .filter(|(at, r)| {
                *at <= now
                    && match r {
                        Ok(a) => {
                            a.fetched_date == today
                                && a.through == through
                                && (a.actions_verified && a.days.iter().all(|d| d.complete)
                                    || now - *at < 300000)
                        }
                        Err(_) => now - *at < 300000,
                    }
            })
            .map(|(_, r)| r.clone());
        let result = if let Some(r) = hit {
            r
        } else {
            let r = load(key, &symbol, now, directory.clone()).await;
            if cache.len() >= 16 {
                cache.clear()
            }
            cache.insert(symbol.clone(), (now, r.clone()));
            r
        };
        let mut report = match &result {
            Ok(a) => core::calculate(&symbol, &a.days, now, a.actions_verified),
            Err(e) => Report {
                version: core::VERSION.into(),
                symbol: symbol.clone(),
                phase: "pending".into(),
                at: now,
                detail: e.clone(),
                ..Default::default()
            },
        };
        if let Ok(a) = &result {
            let window = {
                let s = state.lock().unwrap();
                let minutes=if s.symbol==symbol {s.minutes.values().cloned().collect::<Vec<_>>()} else {s.scanner_store.bot_history(&symbol,now).map(|(_,rows)|rows).unwrap_or_default()};
                if s.stocks.phase == "streaming"
                {
                    calendar::date_at(now).and_then(|d| match calendar::schedule(d) {
                        calendar::Day::Open(session)
                            if now >= session.open_ms && now < session.close_ms =>
                        {
                            let rows = minutes.iter()
                                .filter(|b|b.timestamp_ms>=session.open_ms && b.timestamp_ms<now/60000*60000)
                                .cloned()
                                .collect::<Vec<_>>();
                            Some((session, rows))
                        }
                        _ => None,
                    })
                } else {
                    None
                }
            };
            if let Some((session, rows)) = window {
                current(&mut report, a, &session, &rows, now);
            }
        }
        {
            let mut s = state.lock().unwrap();
            if s.symbol == symbol {
                s.gap_report = report.clone();
            }
            s.gap_reports.insert(symbol.clone(),report);
            if s.gap_reports.len()>16 {s.gap_reports.retain(|key,_|key==&symbol);}
        }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
fn current(report: &mut Report, a: &Archive, session: &calendar::Session, rows: &[Bar], now: u64) {
    if now < session.open_ms || now >= session.close_ms {
        return;
    }
    let end = now / 60000 * 60000;
    let expected = ((end - session.open_ms) / 60000) as usize;
    let complete = expected > 0
        && rows.len() == expected
        && rows.iter().enumerate().all(|(i, b)| {
            b.timestamp_ms == session.open_ms + i as u64 * 60000
                && [b.open, b.high, b.low, b.close, b.volume]
                    .iter()
                    .all(|v| v.is_finite())
                && b.low > 0.
                && b.volume >= 0.
                && b.high >= b.open.max(b.close)
                && b.low <= b.open.min(b.close)
        });
    for g in &mut report.gaps {
        if g.born < session.date {
            g.age_sessions += 1;
        }
    }
    if complete {
        if let Some(last) = a
            .days
            .last()
            .filter(|d| d.complete && d.date < session.date)
        {
            core::opening(
                report,
                &session.date,
                last.close,
                rows[0].open,
                a.dividends.contains(&session.date),
            );
        }
        core::update(
            &mut report.gaps,
            &session.date,
            rows.iter().map(|b| b.low).fold(f64::INFINITY, f64::min),
            rows.iter().map(|b| b.high).fold(0., f64::max),
            rows.last().unwrap().close,
        );
    } else {
        for g in &mut report.gaps {
            if matches!(g.state.as_str(), "OPEN" | "PARTIAL") {
                g.verified = false;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (u64, Archive) {
        let now = calendar::rfc3339_ms("2026-10-06T10:00:00-04:00").unwrap();
        let sessions = core::sessions(now, 60).unwrap();
        let days = sessions
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
        (
            now,
            Archive {
                version: core::VERSION.into(),
                symbol: "QQQ".into(),
                through: sessions.last().unwrap().date.clone(),
                fetched_date: "2026-10-06".into(),
                days,
                actions_verified: true,
                dividends: BTreeSet::new(),
            },
        )
    }
    #[test]
    fn archive_identity_and_daily_split_refresh() {
        let (_, mut a) = fixture();
        let path = std::env::temp_dir().join(format!(
            "snipers-gap-cache-test-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, serde_json::to_vec(&a).unwrap()).unwrap();
        assert!(cached(&path, "QQQ", &a.through, "2026-10-06").is_some());
        assert!(cached(&path, "SPY", &a.through, "2026-10-06").is_none());
        assert!(cached(&path, "QQQ", &a.through, "2026-10-07").is_none());
        a.actions_verified = false;
        std::fs::write(&path, serde_json::to_vec(&a).unwrap()).unwrap();
        assert!(cached(&path, "QQQ", &a.through, "2026-10-06").is_none());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn opening_current_fill_and_premarket_or_close_cannot_reclaim() {
        let (now, a) = fixture();
        let calendar::Day::Open(session) = calendar::schedule(calendar::date_at(now).unwrap())
        else {
            panic!()
        };
        let rows: Vec<_> = (session.open_ms..now)
            .step_by(60000)
            .map(|t| Bar {
                timestamp_ms: t,
                open: 104.,
                high: 105.,
                low: 102.,
                close: 104.,
                volume: 1.,
                vwap: None,
            })
            .collect();
        let mut report = core::calculate("QQQ", &a.days, now, true);
        current(&mut report, &a, &session, &rows, now);
        assert_eq!(report.gaps.len(), 1);
        assert_eq!(report.gaps[0].fill_pct, 50.);
        assert_eq!(report.gaps[0].state, "PARTIAL");
        let before = serde_json::to_string(&report).unwrap();
        current(&mut report, &a, &session, &[], session.open_ms - 1);
        current(&mut report, &a, &session, &[], session.close_ms);
        assert_eq!(serde_json::to_string(&report).unwrap(), before);
        current(&mut report, &a, &session, &rows[1..], now);
        assert!(!report.gaps[0].verified);
    }
    #[test]
    fn history_reconstruction_is_bounded_and_fast() {
        let (now, mut a) = fixture();
        for (i, d) in a.days.iter_mut().enumerate() {
            let price = 100. + (i % 2) as f64 * 5.;
            d.open = price;
            d.high = price + 1.;
            d.low = price - 1.;
            d.close = price;
        }
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let r = core::calculate("QQQ", &a.days, now, true);
            assert!(r.gaps.len() <= 118);
        }
        let mean = start.elapsed().as_secs_f64() * 1000. / 1000.;
        println!("GAP history: {mean:.3} ms / 60 sessions (debug)");
        assert!(mean < 10.);
    }
    #[test]
    fn rth_coverage_cannot_be_imputed() {
        let t = calendar::rfc3339_ms("2026-10-05T10:00:00-04:00").unwrap();
        let calendar::Day::Open(s) = calendar::schedule(calendar::date_at(t).unwrap()) else {
            panic!()
        };
        let rows: Vec<_> = (s.open_ms..s.close_ms)
            .step_by(60000)
            .map(|t| Bar {
                timestamp_ms: t,
                open: 100.,
                high: 101.,
                low: 99.,
                close: 100.,
                volume: 1.,
                vwap: None,
            })
            .collect();
        assert!(aggregate(&s, &rows, &BTreeSet::new()).complete);
        assert!(!aggregate(&s, &rows[1..], &BTreeSet::new()).complete);
        let mut bad = rows;
        bad[1].timestamp_ms = bad[0].timestamp_ms;
        assert!(!aggregate(&s, &bad, &BTreeSet::new()).complete);
    }
}
