//! Explicit, bounded REST exploration; never mutates live selection or trading state.
use crate::{credentials::Credentials, history, live::Bar};
use futures_util::{stream, StreamExt};
use serde::Serialize;
use snipers_core::{calendar, universe};
use std::{collections::BTreeMap, time::Duration};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub symbol: String,
    pub sector: String,
    pub volume: f64,
    pub price: f64,
    pub change_pct: f64,
    pub last_bar_ms: u64,
    pub bars: usize,
    pub volume_metrics: crate::top_five_metrics::VolumeMetrics,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub date: String,
    pub start_ms: u64,
    pub cutoff_ms: u64,
    pub total: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub empty: usize,
    pub errors: Vec<String>,
    pub candidates: Vec<Candidate>,
    pub complete: bool,
}
fn bounds(now: u64) -> Result<(String, u64, u64), &'static str> {
    let date = calendar::date_at(now).ok_or("Fecha no válida")?;
    let day =
        match calendar::schedule(date) {
            calendar::Day::Open(day) => day,
            calendar::Day::Closed => return Err(
                "Hoy no hay sesión de mercado. Busca en un día de negociación desde las 04:01 ET.",
            ),
            calendar::Day::Unknown => return Err("Fecha fuera del calendario verificado"),
        };
    let start = calendar::legacy_eastern_ms(&format!("{date} 04:00:00")).ok_or("Hora no válida")?;
    let cutoff = (now / 60_000 * 60_000).min(day.open_ms);
    if cutoff <= start {
        return Err("La búsqueda requiere al menos un minuto cerrado desde las 04:00 ET.");
    }
    Ok((date.to_string(), start, cutoff))
}
fn summarize(
    symbol: String,
    bars: Vec<Bar>,
    start: u64,
    cutoff: u64,
) -> Result<Option<Candidate>, &'static str> {
    let mut minutes = BTreeMap::new();
    for b in bars {
        if b.timestamp_ms < start || b.timestamp_ms.saturating_add(60_000) > cutoff {
            continue;
        }
        if !b.timestamp_ms.is_multiple_of(60_000)
            || ![b.open, b.high, b.low, b.close]
                .iter()
                .all(|v| v.is_finite() && *v > 0.0)
            || !b.volume.is_finite()
            || b.volume < 0.0
            || b.high < b.open.max(b.close)
            || b.low > b.open.min(b.close)
        {
            return Err("Velas inválidas");
        }
        if minutes.insert(b.timestamp_ms, b).is_some() {
            return Err("Minutos duplicados");
        }
    }
    let Some(first) = minutes.values().next() else {
        return Ok(None);
    };
    let last = minutes.values().next_back().expect("nonempty");
    let volume: f64 = minutes.values().map(|b| b.volume).sum();
    if !volume.is_finite() {
        return Err("Volumen inválido");
    }
    if volume <= 0.0 {
        return Ok(None);
    }
    let change_pct = (last.close / first.open - 1.0) * 100.0;
    if !change_pct.is_finite() {
        return Err("Variación inválida");
    }
    Ok(Some(Candidate {
        sector: universe::stock_asset(&symbol)
            .and_then(|a| a.sector.clone())
            .unwrap_or_default(),
        symbol,
        volume,
        price: last.close,
        change_pct,
        last_bar_ms: last.timestamp_ms + 60_000,
        bars: minutes.len(),
        volume_metrics: crate::top_five_metrics::calculate(&minutes, start, cutoff),
    }))
}
pub async fn run(
    key: &Credentials,
    publish: impl FnMut(&Report) -> bool,
) -> Result<Report, &'static str> {
    let symbols = universe::assets()
        .keys()
        .filter(|s| universe::stock_asset(s).is_some())
        .cloned()
        .collect();
    query(
        crate::live::now_ms(),
        symbols,
        publish,
        |symbol, start, end| async move { history::fetch(key, &symbol, start, end).await },
    )
    .await
}
async fn query<F, Fut>(
    now: u64,
    symbols: Vec<String>,
    mut publish: impl FnMut(&Report) -> bool,
    fetch: F,
) -> Result<Report, &'static str>
where
    F: Fn(String, u64, u64) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<Bar>, &'static str>>,
{
    let (date, start, cutoff) = bounds(now)?;
    let mut report = Report {
        date,
        start_ms: start,
        cutoff_ms: cutoff,
        total: symbols.len(),
        processed: 0,
        succeeded: 0,
        empty: 0,
        errors: vec![],
        candidates: vec![],
        complete: false,
    };
    if !publish(&report) {
        return Err("Búsqueda cancelada");
    }
    let mut pending = stream::iter(symbols.into_iter().map(|symbol| {
        let request = fetch(symbol.clone(), start, cutoff - 1);
        async move {
            (
                symbol,
                tokio::time::timeout(Duration::from_secs(12), request)
                    .await
                    .unwrap_or(Err("Tiempo de espera agotado")),
            )
        }
    }))
    .buffer_unordered(4);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        let next = match tokio::time::timeout_at(deadline, pending.next()).await {
            Ok(next) => next,
            Err(_) => {
                report
                    .errors
                    .push("Búsqueda parcial: tiempo máximo de consulta alcanzado".into());
                break;
            }
        };
        let Some((symbol, result)) = next else {
            break;
        };
        report.processed += 1;
        let blocked = result
            .as_ref()
            .err()
            .is_some_and(|e| e.contains("401/403") || e.contains("limitado"));
        match result.and_then(|bars| summarize(symbol.clone(), bars, start, cutoff)) {
            Ok(Some(candidate)) => {
                report.succeeded += 1;
                report.candidates.push(candidate);
            }
            Ok(None) => {
                report.succeeded += 1;
                report.empty += 1;
            }
            Err(e) => report.errors.push(format!("{symbol}: {e}")),
        }
        report
            .candidates
            .sort_by(|a, b| b.volume.total_cmp(&a.volume).then(a.symbol.cmp(&b.symbol)));
        if !publish(&report) {
            return Err("Búsqueda cancelada");
        }
        if blocked {
            break;
        }
    }
    drop(pending);
    report.complete = report.processed == report.total && report.errors.is_empty();
    // Comparable RVOL for the same bounded twenty-candidate native options pool.
    let leaders: Vec<_> = report
        .candidates
        .iter()
        .take(20)
        .map(|c| c.symbol.clone())
        .collect();
    let mut history_requests = stream::iter(leaders.into_iter().map(|symbol| {
        let request = fetch(
            symbol.clone(),
            start.saturating_sub(14 * 86_400_000),
            start - 1,
        );
        async move {
            (
                symbol,
                tokio::time::timeout(Duration::from_secs(15), request).await,
            )
        }
    }))
    .buffer_unordered(3);
    while let Some((symbol, result)) = history_requests.next().await {
        if let Ok(Ok(bars)) = result {
            let count = bars.len();
            let minutes: BTreeMap<_, _> = bars.into_iter().map(|b| (b.timestamp_ms, b)).collect();
            if minutes.len() == count {
                if let Some(candidate) = report.candidates.iter_mut().find(|c| c.symbol == symbol) {
                    (
                        candidate.volume_metrics.rvol_premarket,
                        candidate.volume_metrics.rvol_sessions,
                    ) = crate::top_five_metrics::premarket_rvol(&minutes, cutoff, candidate.volume);
                }
            }
        }
        if !publish(&report) {
            return Err("Búsqueda cancelada");
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(s: &str) -> u64 {
        calendar::legacy_eastern_ms(s).unwrap()
    }
    fn bar(t: u64, v: f64) -> Bar {
        Bar {
            timestamp_ms: t,
            open: 100.,
            high: 102.,
            low: 99.,
            close: 101.,
            volume: v,
            vwap: None,
        }
    }
    #[test]
    fn early_search_and_calendar_cutoffs() {
        for hour in ["07:00:00", "07:30:00", "08:00:00"] {
            let now = at(&format!("2026-09-28 {hour}"));
            let (_, start, end) = bounds(now + 30_000).unwrap();
            assert_eq!(start, at("2026-09-28 04:00:00"));
            assert_eq!(end, now);
        }
        assert!(bounds(at("2026-09-27 08:00:00")).is_err());
        assert!(bounds(at("2026-09-28 04:00:30")).is_err());
        // Repeating the search after the bell must never import RTH volume.
        for hour in ["09:30:00", "10:00:00", "16:00:00"] {
            assert_eq!(
                bounds(at(&format!("2026-09-28 {hour}"))).unwrap().2,
                at("2026-09-28 09:30:00")
            );
        }
        assert_eq!(
            bounds(at("2026-11-27 16:00:00")).unwrap().2,
            at("2026-11-27 09:30:00")
        );
    }
    #[test]
    fn ignores_unfinished_and_old_bars_rejects_duplicates() {
        let start = at("2026-09-28 04:00:00");
        let end = start + 120_000;
        let rows = vec![
            bar(start - 60_000, 999.),
            bar(start, 10.),
            bar(start + 60_000, 20.),
            bar(end, 999.),
        ];
        let r = summarize("QQQ".into(), rows, start, end).unwrap().unwrap();
        assert_eq!(r.volume, 30.);
        assert_eq!(r.bars, 2);
        assert_eq!(r.last_bar_ms, end);
        assert!(summarize(
            "QQQ".into(),
            vec![bar(start, 1.), bar(start, 1.)],
            start,
            end
        )
        .is_err());
        assert!(summarize("QQQ".into(), vec![bar(start, f64::NAN)], start, end).is_err());
    }
    #[tokio::test]
    async fn query_ranks_received_volume_and_reports_partial_errors() {
        let now = at("2026-09-28 07:00:00");
        let r = query(
            now,
            vec!["A".into(), "B".into(), "C".into()],
            |_| true,
            |s, start, _| async move {
                if s == "C" {
                    Err("Sin permiso")
                } else {
                    Ok(vec![bar(start, if s == "A" { 10. } else { 20. })])
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(r.candidates[0].symbol, "B");
        assert_eq!(r.processed, 3);
        assert_eq!(r.succeeded, 2);
        assert!(!r.complete);
    }
    #[tokio::test]
    async fn empty_is_not_failure_and_cancel_does_not_fetch() {
        let now = at("2026-09-28 08:00:00");
        let r = query(
            now,
            vec!["A".into()],
            |_| true,
            |_, _, _| async { Ok(vec![]) },
        )
        .await
        .unwrap();
        assert!(r.complete);
        assert_eq!(r.empty, 1);
        assert!(r.candidates.is_empty());
        assert!(query(
            now,
            vec!["A".into()],
            |_| false,
            |_, _, _| async {
                panic!("must not fetch");
                #[allow(unreachable_code)]
                Ok(vec![])
            }
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn enriches_native_shortlist_without_changing_volume_preselection() {
        let now = at("2026-09-28 07:00:00");
        let today = at("2026-09-28 04:00:00");
        let calls = std::sync::Mutex::new(Vec::new());
        let r = query(
            now,
            (0..7).map(|i| format!("A{i}")).collect(),
            |_| true,
            |symbol, start, end| {
                calls.lock().unwrap().push((symbol.clone(), start, end));
                async move {
                    if start == today {
                        let size = symbol[1..].parse::<f64>().unwrap() + 1.;
                        Ok(vec![bar(today, size * 100.)])
                    } else if symbol == "A6" {
                        Err("Historial no disponible")
                    } else {
                        Ok([
                            "2026-09-21",
                            "2026-09-22",
                            "2026-09-23",
                            "2026-09-24",
                            "2026-09-25",
                        ]
                        .iter()
                        .map(|d| bar(at(&format!("{d} 04:00:00")), 100.))
                        .collect())
                    }
                }
            },
        )
        .await
        .unwrap();
        assert!(r.complete);
        assert_eq!(
            r.candidates
                .iter()
                .take(5)
                .map(|r| r.symbol.as_str())
                .collect::<Vec<_>>(),
            vec!["A6", "A5", "A4", "A3", "A2"]
        );
        assert_eq!(r.candidates[0].volume_metrics.rvol_premarket, None);
        assert_eq!(r.candidates[1].volume_metrics.rvol_premarket, Some(6.));
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 14);
        assert!(calls
            .iter()
            .filter(|(_, s, _)| *s < today)
            .all(|(_, _, end)| *end == today - 1));
    }
}
