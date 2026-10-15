//! Explicit historical requests, independent of the live state and its sockets.
use crate::{credentials::Credentials, history, live::Bar};
use serde::Serialize;
use snipers_core::{bars::Minute, calendar, historical};
use std::collections::BTreeMap;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub symbol: String,
    pub summary: Option<historical::Read>,
    pub error: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema: u32,
    pub date: String,
    pub phase: String,
    pub total: usize,
    pub processed: usize,
    pub rows: Vec<Row>,
    pub top: Vec<historical::Read>,
    pub generated_ms: u64,
    pub error: Option<String>,
    pub saved_path: Option<String>,
    pub entry_confirmed: bool,
}
fn minute(b: &Bar) -> Minute {
    Minute {
        t: b.timestamp_ms,
        o: b.open,
        h: b.high,
        l: b.low,
        c: b.close,
        v: b.volume,
        vw: b.vwap,
    }
}
pub async fn run(
    key: &Credentials,
    date: &str,
    rvol: bool,
    publish: impl FnMut(&Report) -> bool,
) -> Result<Report, &'static str> {
    let symbols = snipers_core::universe::assets()
        .keys()
        .filter(|s| snipers_core::universe::stock_asset(s).is_some())
        .cloned()
        .collect();
    query(
        date,
        rvol,
        crate::live::now_ms(),
        symbols,
        publish,
        |symbol, from, to| async move { history::fetch(key, &symbol, from, to).await },
    )
    .await
}
async fn query<F, Fut>(
    date: &str,
    rvol: bool,
    now: u64,
    symbols: Vec<String>,
    mut publish: impl FnMut(&Report) -> bool,
    mut fetch: F,
) -> Result<Report, &'static str>
where
    F: FnMut(String, u64, u64) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<Bar>, &'static str>>,
{
    let day = historical::session(date, now)?;
    let mut report = Report {
        schema: 1,
        date: date.into(),
        phase: "loading".into(),
        total: symbols.len(),
        processed: 0,
        rows: Vec::new(),
        top: Vec::new(),
        generated_ms: now,
        error: None,
        saved_path: None,
        entry_confirmed: false,
    };
    let mut tapes = BTreeMap::new();
    if !publish(&report) {
        return Err("Consulta histórica cancelada");
    }
    for symbol in symbols {
        let result = fetch(symbol.clone(), day.open_ms, day.close_ms - 1).await;
        let summary = match result {
            Ok(rows) => {
                let minutes: Vec<_> = rows.iter().map(minute).collect();
                match historical::summarize(&symbol, date, &minutes, now) {
                    Ok(summary) => {
                        tapes.insert(symbol.clone(), minutes);
                        Row {
                            symbol: symbol.clone(),
                            summary: Some(summary),
                            error: None,
                        }
                    }
                    Err(e) => Row {
                        symbol: symbol.clone(),
                        summary: None,
                        error: Some(e.into()),
                    },
                }
            }
            Err(e) => {
                if e.contains("limitado") || e.contains("401/403") {
                    report.phase = "paused".into();
                    report.error = Some(e.into());
                }
                Row {
                    symbol: symbol.clone(),
                    summary: None,
                    error: Some(e.into()),
                }
            }
        };
        report.rows.push(summary);
        report.processed = report.rows.len();
        report.generated_ms = crate::live::now_ms();
        report.top = historical::top_activity(
            &report
                .rows
                .iter()
                .filter_map(|r| r.summary.clone())
                .collect::<Vec<_>>(),
        );
        if !publish(&report) {
            return Err("Consulta histórica cancelada");
        }
        if report.phase == "paused" {
            return Ok(report);
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    if rvol {
        report.phase = "rvol".into();
        if !publish(&report) {
            return Err("Consulta histórica cancelada");
        }
        for index in 0..report.top.len() {
            let symbol = report.top[index].symbol.clone();
            match fetch(
                symbol.clone(),
                day.open_ms.saturating_sub(21 * 86_400_000),
                day.open_ms - 1,
            )
            .await
            {
                Ok(bars) => {
                    let mut sessions = BTreeMap::<String, Vec<Minute>>::new();
                    for b in bars {
                        if let Some(date) = calendar::date_at(b.timestamp_ms) {
                            sessions
                                .entry(date.to_string())
                                .or_default()
                                .push(minute(&b));
                        }
                    }
                    let sessions: Vec<_> = sessions.into_iter().collect();
                    if let Some(target) = tapes.get(&symbol) {
                        historical::relative_volume(&mut report.top[index], target, &sessions, now);
                    }
                }
                Err(e) => {
                    report.top[index].rvol_status = e.into();
                    if e.contains("limitado") || e.contains("401/403") {
                        report.phase = "paused".into();
                        report.error = Some(e.into());
                    }
                }
            }
            if let Some(row) = report.rows.iter_mut().find(|r| r.symbol == symbol) {
                row.summary = Some(report.top[index].clone());
            }
            report.generated_ms = crate::live::now_ms();
            if !publish(&report) {
                return Err("Consulta histórica cancelada");
            }
            if report.phase == "paused" {
                return Ok(report);
            }
        }
    }
    report.phase = if report.rows.iter().any(|r| r.error.is_some()) {
        "partial"
    } else {
        "complete"
    }
    .into();
    report.generated_ms = crate::live::now_ms();
    Ok(report)
}
/// New immutable report; no legacy path or existing report is overwritten.
pub fn save(
    directory: &std::path::Path,
    report: &Report,
) -> Result<std::path::PathBuf, &'static str> {
    use std::io::Write;
    static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if calendar::day_bounds(&report.date).is_none() || report.rows.len() > 256 {
        return Err("Reporte histórico inválido");
    }
    let bytes =
        serde_json::to_vec_pretty(report).map_err(|_| "Reporte histórico no serializable")?;
    if bytes.len() > 4_000_000 {
        return Err("Reporte histórico demasiado grande");
    }
    std::fs::create_dir_all(directory).map_err(|_| "No se pudo crear la carpeta de reportes")?;
    let path = directory.join(format!(
        "history-{}-{}-{}-{}.json",
        report.date,
        report.generated_ms,
        std::process::id(),
        ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|_| "No se pudo crear un reporte nuevo")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "No se pudo completar el reporte histórico")?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn now() -> u64 {
        calendar::rfc3339_ms("2026-09-24T15:00:00Z").unwrap()
    }
    fn bars(date: &str) -> Vec<Bar> {
        let day = historical::session(date, now()).unwrap();
        (0..390)
            .map(|i| Bar {
                timestamp_ms: day.open_ms + i * 60000,
                open: 100.0,
                high: 102.0,
                low: 99.0,
                close: 101.0,
                volume: 100.0,
                vwap: Some(100.5),
            })
            .collect()
    }
    #[tokio::test]
    async fn historical_query_runs_rvol_only_after_ranking_and_preserves_partial_results() {
        let calls = std::cell::RefCell::new(Vec::new());
        let progress = std::cell::RefCell::new(Vec::new());
        let report = query(
            "2026-09-23",
            true,
            now(),
            vec!["QQQ".into(), "SPY".into()],
            |r| {
                progress.borrow_mut().push(r.phase.clone());
                true
            },
            |symbol, from, to| {
                let baseline = to - from > 86_400_000;
                calls.borrow_mut().push((symbol.clone(), baseline));
                std::future::ready(if symbol == "SPY" {
                    Err("Fixture de fuente incompleta")
                } else if baseline {
                    Ok([
                        "2026-09-22",
                        "2026-09-21",
                        "2026-09-18",
                        "2026-09-17",
                        "2026-09-16",
                    ]
                    .into_iter()
                    .flat_map(bars)
                    .collect())
                } else {
                    Ok(bars("2026-09-23"))
                })
            },
        )
        .await
        .unwrap();
        assert_eq!(report.phase, "partial");
        assert_eq!(report.processed, 2);
        assert_eq!(report.top.len(), 1);
        assert_eq!(report.top[0].rvol, Some(1.0));
        assert_eq!(
            *calls.borrow(),
            vec![
                ("QQQ".into(), false),
                ("SPY".into(), false),
                ("QQQ".into(), true)
            ]
        );
        assert!(progress.borrow().contains(&"rvol".into()));
        assert!(!report.entry_confirmed);
    }
    #[tokio::test]
    async fn rate_limit_and_cancel_stop_further_requests_and_reports_are_immutable() {
        let calls = std::cell::Cell::new(0);
        let report = query(
            "2026-09-23",
            false,
            now(),
            vec!["QQQ".into(), "SPY".into()],
            |_| true,
            |_, _, _| {
                calls.set(calls.get() + 1);
                std::future::ready(Err("Historial temporalmente limitado por Massive."))
            },
        )
        .await
        .unwrap();
        assert_eq!(report.phase, "paused");
        assert_eq!(report.processed, 1);
        assert_eq!(calls.get(), 1);
        assert!(query(
            "2026-09-23",
            false,
            now(),
            vec!["QQQ".into()],
            |_| false,
            |_, _, _| {
                calls.set(calls.get() + 1);
                std::future::ready(Ok(Vec::new()))
            }
        )
        .await
        .is_err());
        assert_eq!(calls.get(), 1);
        let directory = std::env::temp_dir().join(format!(
            "snipers-history-test-{}-{}",
            std::process::id(),
            crate::live::now_ms()
        ));
        assert!(!directory.exists());
        let a = save(&directory, &report).unwrap();
        let b = save(&directory, &report).unwrap();
        assert_ne!(a, b);
        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        std::fs::remove_file(a).unwrap();
        std::fs::remove_file(b).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
