//! Explicit historical sampling for a research baseline. No live feed, orders or scores.
use crate::{
    credentials::Credentials,
    history::{self, checked_url},
    live::{now_ms, Bar},
    option_activity_history::{self, Report, Request},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snipers_core::{
    calendar::{self, Day},
    option_activity_baseline::{self, Baseline, Sample},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dataset {
    pub version: String,
    pub underlying: String,
    pub date: String,
    pub collected_ms: u64,
    pub selection: String,
    pub opening_reference_price: Option<f64>,
    pub opening_reference_available_ms: Option<u64>,
    pub stock_minutes: usize,
    pub reports: Vec<String>,
    pub aggregate_reports: Vec<String>,
    #[serde(default)]
    pub context_reports: Vec<String>,
    pub skipped_windows: BTreeMap<String, usize>,
    pub samples: Vec<Sample>,
    pub errors: Vec<String>,
}
fn aggregate_samples(
    request: &Request,
    metadata: &option_activity_history::Contract,
    session: &calendar::Session,
    option_bars: &[Bar],
    stock_bars: &[Bar],
    selected_after: u64,
    collected_ms: u64,
) -> (Vec<Sample>, BTreeMap<String, usize>) {
    let option_minutes: BTreeMap<_, _> = option_bars.iter().map(|b| (b.timestamp_ms, b)).collect();
    let stock_minutes: BTreeMap<_, _> = stock_bars.iter().map(|b| (b.timestamp_ms, b)).collect();
    let step = request.window_minutes * 60_000;
    let mut samples = vec![];
    let mut skipped = BTreeMap::new();
    for start in (session.open_ms..session.close_ms).step_by(step as usize) {
        let end = (start + step).min(session.close_ms);
        if start < selected_after {
            *skipped
                .entry("aggregate-selection-not-yet-available".into())
                .or_default() += 1;
            continue;
        }
        let price = match stock_minutes.get(&(start - 60_000)) {
            Some(bar) => bar.close,
            None => {
                *skipped
                    .entry("aggregate-missing-prior-stock-minute".into())
                    .or_default() += 1;
                continue;
            }
        };
        let volumes: Option<Vec<_>> = (start..end)
            .step_by(60_000)
            .map(|t| option_minutes.get(&t).map(|b| b.volume))
            .collect();
        let volume = match volumes {
            Some(volumes) => volumes.iter().sum::<f64>(),
            None => {
                *skipped
                    .entry("missing-option-minute-not-filled-as-zero".into())
                    .or_default() += 1;
                continue;
            }
        };
        samples.push(Sample {
            underlying: request.underlying.clone(),
            ticker: request.ticker.clone(),
            kind: metadata.kind.clone(),
            expiration: metadata.expiration.clone(),
            strike: metadata.strike,
            start_ms: start,
            end_ms: end,
            underlying_price: price,
            price_available_ms: start,
            contracts: volume,
            premium: None,
            measurement: "eligible-option-bars".into(),
            source_collected_ms: collected_ms,
            quality_complete: volume.is_finite() && volume >= 0. && price.is_finite() && price > 0.,
        });
    }
    (samples, skipped)
}
async fn collect_aggregate_volume(
    key: &Credentials,
    request: &Request,
    session: &calendar::Session,
    stocks: &[Bar],
    selected_after: u64,
) -> Result<(Vec<Sample>, BTreeMap<String, usize>, Value), String> {
    collect_aggregate_volume_until(
        key,
        request,
        session,
        stocks,
        selected_after,
        session.close_ms,
    )
    .await
}
pub(crate) async fn collect_aggregate_volume_until(
    key: &Credentials,
    request: &Request,
    session: &calendar::Session,
    stocks: &[Bar],
    selected_after: u64,
    through_ms: u64,
) -> Result<(Vec<Sample>, BTreeMap<String, usize>, Value), String> {
    if through_ms > session.close_ms || through_ms <= session.open_ms || through_ms > now_ms() {
        return Err("Límite de captura aún abierto o fuera de sesión".into());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "REST no disponible")?;
    let reference_path = format!("/v3/reference/options/contracts/{}", request.ticker);
    let mut url = checked_url(
        &format!("https://api.massive.com{reference_path}"),
        &reference_path,
    )
    .map_err(str::to_owned)?;
    url.query_pairs_mut().append_pair("as_of", &request.date);
    let reference = option_activity_history::get(&client, key, url).await?;
    let metadata = option_activity_history::contract(request, &reference["results"])?;
    let path = format!(
        "/v2/aggs/ticker/{}/range/1/minute/{}/{}",
        request.ticker,
        session.open_ms,
        through_ms - 1
    );
    let mut url =
        checked_url(&format!("https://api.massive.com{path}"), &path).map_err(str::to_owned)?;
    url.query_pairs_mut()
        .append_pair("adjusted", "false")
        .append_pair("sort", "asc")
        .append_pair("limit", "50000");
    let mut visited = BTreeSet::new();
    let mut bars: BTreeMap<u64, Bar> = BTreeMap::new();
    let mut fingerprints = BTreeMap::new();
    let mut terminal = false;
    let mut pages = 0;
    for _ in 0..4 {
        if !visited.insert(url.to_string()) {
            return Err("Velas de opciones con cursor repetido".into());
        }
        let page = option_activity_history::get(&client, key, url).await?;
        pages += 1;
        if page["ticker"].as_str().is_some_and(|t| t != request.ticker) {
            return Err("Velas de otro contrato".into());
        }
        let rows = page["results"]
            .as_array()
            .ok_or("Velas de opciones ausentes; no se sustituyen por cero")?;
        if rows.len() > 50000 {
            return Err("Página de velas excede el límite".into());
        }
        for row in rows {
            let t = row["t"].as_u64().ok_or("Vela de opción sin reloj")?;
            let o = row["o"].as_f64().ok_or("Vela de opción inválida")?;
            let h = row["h"].as_f64().ok_or("Vela de opción inválida")?;
            let l = row["l"].as_f64().ok_or("Vela de opción inválida")?;
            let c = row["c"].as_f64().ok_or("Vela de opción inválida")?;
            let v = row["v"].as_f64().ok_or("Vela de opción sin volumen")?;
            if ![o, h, l, c].iter().all(|n| n.is_finite() && *n > 0.)
                || !v.is_finite()
                || v < 0.
                || h < o.max(c)
                || l > o.min(c)
                || !t.is_multiple_of(60_000)
                || t < session.open_ms
                || t + 60_000 > through_ms
            {
                return Err("Calidad de velas de opciones no válida".into());
            }
            if let Some(old) = fingerprints.insert(t, row.clone()) {
                if old != *row {
                    return Err("Velas duplicadas en conflicto".into());
                }
            }
            bars.insert(
                t,
                Bar {
                    timestamp_ms: t,
                    open: o,
                    high: h,
                    low: l,
                    close: c,
                    volume: v,
                    vwap: row["vw"].as_f64().filter(|n| n.is_finite()),
                },
            );
        }
        match &page["next_url"] {
            Value::Null => {
                terminal = true;
                break;
            }
            Value::String(s) if s.is_empty() => {
                terminal = true;
                break;
            }
            Value::String(s) => url = checked_url(s, &path).map_err(str::to_owned)?,
            _ => return Err("Cursor de velas no válido".into()),
        }
    }
    if !terminal {
        return Err("Paginación de velas incompleta".into());
    }
    let bars: Vec<_> = bars.into_values().collect();
    let collected_ms = now_ms();
    let (samples, skipped) = aggregate_samples(
        request,
        &metadata,
        session,
        &bars,
        stocks,
        selected_after,
        collected_ms,
    );
    let evidence = serde_json::json!({"version":"option-aggregate-volume-evidence-v1","request":request,"contract":metadata,
        "session":session,"collectedMs":collected_ms,"measurement":"eligible-option-bars","adjusted":false,
        "pages":pages,"retrievalComplete":true,"bars":bars,"premiumReconstructed":false});
    Ok((samples, skipped, evidence))
}
fn enrich(
    report: &Report,
    bars: &[Bar],
    selected_after: u64,
) -> (Vec<Sample>, BTreeMap<String, usize>) {
    let prices: BTreeMap<_, _> = bars.iter().map(|b| (b.timestamp_ms, b)).collect();
    let mut samples = vec![];
    let mut skipped = BTreeMap::new();
    for window in &report.windows {
        let price = window
            .start_ms
            .checked_sub(60_000)
            .and_then(|t| prices.get(&t));
        if window.start_ms < selected_after {
            *skipped
                .entry("contract-selection-not-yet-available".into())
                .or_default() += 1;
            continue;
        }
        let bar = match price {
            Some(bar) if bar.close.is_finite() && bar.close > 0. => *bar,
            _ => {
                *skipped
                    .entry("missing-prior-closed-stock-minute".into())
                    .or_default() += 1;
                continue;
            }
        };
        samples.push(Sample {
            underlying: report.request.underlying.clone(),
            ticker: report.contract.ticker.clone(),
            kind: report.contract.kind.clone(),
            expiration: report.contract.expiration.clone(),
            strike: report.contract.strike,
            start_ms: window.start_ms,
            end_ms: window.end_ms,
            underlying_price: bar.close,
            price_available_ms: bar.timestamp_ms + 60_000,
            contracts: window.observed_contracts,
            premium: Some(window.observed_premium),
            measurement: "reported-trades".into(),
            source_collected_ms: report.collected_ms,
            quality_complete: report.aggregation_complete
                && window.contracts.is_some()
                && window.premium.is_some(),
        });
    }
    (samples, skipped)
}

pub(crate) async fn discover(
    key: &Credentials,
    symbol: &str,
    date: &str,
    price: f64,
    per_side: usize,
) -> Result<(Vec<String>, Value), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "REST no disponible")?;
    let path = "/v3/reference/options/contracts";
    let mut url =
        checked_url(&format!("https://api.massive.com{path}"), path).map_err(str::to_owned)?;
    url.query_pairs_mut()
        .append_pair("underlying_ticker", symbol)
        .append_pair(
            "expired",
            if calendar::date_at(now_ms()).is_some_and(|d| d.to_string() == date) {
                "false"
            } else {
                "true"
            },
        )
        .append_pair("expiration_date", date)
        .append_pair("strike_price.gte", &(price * 0.99).to_string())
        .append_pair("strike_price.lte", &(price * 1.01).to_string())
        .append_pair("limit", "250");
    let mut visited = BTreeSet::new();
    let mut rows = vec![];
    let mut complete = false;
    for _ in 0..4 {
        if !visited.insert(url.to_string()) {
            return Err("Catálogo histórico con cursor repetido".into());
        }
        let page = option_activity_history::get(&client, key, url).await?;
        let values = page["results"]
            .as_array()
            .ok_or("Catálogo histórico sin formato válido")?;
        if values.len() > 250 {
            return Err("Catálogo excede el tamaño solicitado".into());
        }
        rows.extend(values.iter().cloned());
        match &page["next_url"] {
            Value::Null => {
                complete = true;
                break;
            }
            Value::String(s) if s.is_empty() => {
                complete = true;
                break;
            }
            Value::String(s) => url = checked_url(s, path).map_err(str::to_owned)?,
            _ => return Err("Cursor de catálogo no válido".into()),
        }
    }
    if !complete {
        return Err("Catálogo parcial; selección ATM no certificada".into());
    }
    let selected = select_contracts_breadth(&rows, symbol, date, price, per_side)?;
    let evidence = serde_json::json!({"version":"option-selection-evidence-v1",
        "underlying":symbol,"date":date,"openingReferencePrice":price,
        "perSide":per_side,"selected":selected,"catalogRows":rows,
        "complete":complete,"pointInTimeAvailabilityVerified":false});
    Ok((selected, evidence))
}
#[cfg(test)]
fn select_contracts(
    rows: &[Value],
    symbol: &str,
    date: &str,
    price: f64,
) -> Result<Vec<String>, String> {
    select_contracts_breadth(rows, symbol, date, price, 1)
}
pub(crate) fn select_contracts_breadth(
    rows: &[Value],
    symbol: &str,
    date: &str,
    price: f64,
    per_side: usize,
) -> Result<Vec<String>, String> {
    if ![1, 3].contains(&per_side) || !price.is_finite() || price <= 0. {
        return Err("Selección fuera de límite".into());
    }
    let mut selected: Vec<String> = vec![];
    for kind in ["call", "put"] {
        let mut candidates: Vec<_> = rows
            .iter()
            .filter(|r| {
                r["underlying_ticker"] == symbol
                    && r["expiration_date"] == date
                    && r["contract_type"] == kind
                    && r["shares_per_contract"].as_u64() == Some(100)
                    && r["strike_price"]
                        .as_f64()
                        .is_some_and(|v| v.is_finite() && v >= price * 0.99 && v <= price * 1.01)
                    && r["ticker"]
                        .as_str()
                        .is_some_and(|t| snipers_core::options::underlying(t) == Some(symbol))
            })
            .collect();
        let anchors: &[f64] = if per_side == 1 {
            &[0.]
        } else {
            &[0., -0.0075, 0.0075]
        };
        for anchor in anchors {
            let target = price * (1. + anchor);
            candidates.sort_by(|a, b| {
                (a["strike_price"].as_f64().unwrap() - target)
                    .abs()
                    .total_cmp(&(b["strike_price"].as_f64().unwrap() - target).abs())
                    .then(a["ticker"].as_str().cmp(&b["ticker"].as_str()))
            });
            selected.push(
                candidates
                    .iter()
                    .find(|r| {
                        !selected
                            .iter()
                            .any(|t| r["ticker"].as_str() == Some(t.as_str()))
                    })
                    .and_then(|r| r["ticker"].as_str())
                    .ok_or("Falta CALL o PUT histórico cercano")?
                    .to_owned(),
            );
        }
    }
    Ok(selected)
}

pub fn save_json<T: Serialize>(directory: &Path, label: &str, data: &T) -> Result<PathBuf, String> {
    if label.is_empty()
        || !label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("Etiqueta de archivo no válida".into());
    }
    std::fs::create_dir_all(directory).map_err(|_| "No se pudo crear la carpeta de research")?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let file = directory.join(format!(
        "{label}-{}-{}-{}.json",
        now_ms(),
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)
        .map_err(|_| "No se pudo crear el archivo de research")?;
    let mut writer = std::io::BufWriter::new(out);
    serde_json::to_writer(&mut writer, data).map_err(|_| "No se pudo guardar research")?;
    writer.flush().map_err(|_| "Escritura incompleta")?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|_| "Escritura no confirmada")?;
    Ok(file)
}

pub async fn collect_session(
    key: &Credentials,
    symbol: &str,
    date: &str,
    directory: &Path,
    max_pages: usize,
) -> Result<Dataset, String> {
    collect_session_mode(key, symbol, date, directory, max_pages, true, false).await
}
pub async fn collect_session_mode(
    key: &Credentials,
    symbol: &str,
    date: &str,
    directory: &Path,
    max_pages: usize,
    trades: bool,
    aggregates: bool,
) -> Result<Dataset, String> {
    collect_session_breadth(
        key, symbol, date, directory, max_pages, trades, aggregates, 1,
    )
    .await
}
pub async fn collect_session_breadth(
    key: &Credentials,
    symbol: &str,
    date: &str,
    directory: &Path,
    max_pages: usize,
    trades: bool,
    aggregates: bool,
    per_side: usize,
) -> Result<Dataset, String> {
    let (start, _) = calendar::day_bounds(date).ok_or("Fecha no válida")?;
    let session = match calendar::schedule(calendar::date_at(start).ok_or("Fecha no válida")?) {
        Day::Open(s) => s,
        _ => return Err("Sesión no válida".into()),
    };
    if session.close_ms >= now_ms()
        || !(1..=100).contains(&max_pages)
        || snipers_core::universe::stock_asset(symbol).is_none()
        || (!trades && !aggregates)
        || ![1, 3].contains(&per_side)
    {
        return Err("Solicitud histórica no válida".into());
    }
    let mut dataset = Dataset {
        version: "option-activity-dataset-v1".into(),
        underlying: symbol.into(),
        date: date.into(),
        collected_ms: now_ms(),
        selection: format!(
            "0DTE-{per_side}-per-side; anchors-first-closed-RTH-minute; no-volume-ranking"
        ),
        opening_reference_price: None,
        opening_reference_available_ms: None,
        stock_minutes: 0,
        reports: vec![],
        aggregate_reports: vec![],
        context_reports: vec![],
        skipped_windows: BTreeMap::new(),
        samples: vec![],
        errors: vec![],
    };
    let bars = history::fetch_unadjusted(key, symbol, session.open_ms, session.close_ms - 1)
        .await
        .map_err(str::to_owned)?;
    dataset.stock_minutes = bars.len();
    let opening = bars
        .iter()
        .find(|b| b.timestamp_ms == session.open_ms)
        .ok_or("Falta primera vela histórica completa")?;
    dataset.opening_reference_price = Some(opening.close);
    dataset.opening_reference_available_ms = Some(opening.timestamp_ms + 60_000);
    let stock_evidence = serde_json::json!({"version":"stock-context-evidence-v1",
        "underlying":symbol,"date":date,"adjusted":false,"session":session,"bars":bars,
        "collectedMs":now_ms()});
    let stock_file = save_json(&directory.join("context"), "stock-bars", &stock_evidence)?;
    dataset
        .context_reports
        .push(stock_file.to_string_lossy().into_owned());
    let (selected, selection_evidence) =
        discover(key, symbol, date, opening.close, per_side).await?;
    let file = save_json(&directory.join("context"), "selection", &selection_evidence)?;
    dataset
        .context_reports
        .push(file.to_string_lossy().into_owned());
    for ticker in selected {
        let request = Request {
            underlying: symbol.into(),
            ticker,
            date: date.into(),
            window_minutes: 5,
            max_pages,
        };
        if aggregates {
            match collect_aggregate_volume(
                key,
                &request,
                &session,
                &bars,
                opening.timestamp_ms + 60_000,
            )
            .await
            {
                Ok((samples, skipped, evidence)) => {
                    let file = save_json(&directory.join("aggregates"), "option-bars", &evidence)?;
                    dataset
                        .aggregate_reports
                        .push(file.to_string_lossy().into_owned());
                    dataset.samples.extend(samples);
                    for (reason, n) in skipped {
                        *dataset.skipped_windows.entry(reason).or_default() += n;
                    }
                }
                Err(error) => dataset.errors.push(error),
            }
        }
        if trades {
            match option_activity_history::collect(key, request).await {
                Ok(report) => {
                    let file = option_activity_history::save(&directory.join("raw"), &report)?;
                    dataset.reports.push(file.to_string_lossy().into_owned());
                    let (samples, skipped) = enrich(&report, &bars, opening.timestamp_ms + 60_000);
                    for (reason, n) in skipped {
                        *dataset.skipped_windows.entry(reason).or_default() += n;
                    }
                    dataset.samples.extend(samples);
                    dataset.errors.extend(
                        report
                            .warnings
                            .into_iter()
                            .map(|w| format!("{}: {w}", report.contract.ticker)),
                    );
                }
                Err(error) => dataset.errors.push(error),
            }
        }
    }
    dataset.collected_ms = now_ms();
    Ok(dataset)
}
pub fn baseline(datasets: &[Dataset], cutoff_ms: u64, now: u64) -> Result<Baseline, &'static str> {
    let samples: Vec<_> = datasets.iter().flat_map(|d| d.samples.clone()).collect();
    option_activity_baseline::build(&samples, cutoff_ms, 20, now)
}
pub fn previous_sessions(cutoff_date: &str, count: usize) -> Result<Vec<String>, String> {
    if !(1..=60).contains(&count) {
        return Err("Cantidad de sesiones fuera de límite".into());
    }
    let (start, _) = calendar::day_bounds(cutoff_date).ok_or("Fecha de corte inválida")?;
    let mut day = calendar::date_at(start).ok_or("Fecha inválida")?;
    let mut dates = vec![];
    for _ in 0..100 {
        day = day.pred_opt().ok_or("Fecha no válida")?;
        match calendar::schedule(day) {
            Day::Open(s) => dates.push(s.date),
            Day::Unknown => return Err("Calendario fuera de cobertura".into()),
            _ => {}
        }
        if dates.len() == count {
            dates.reverse();
            return Ok(dates);
        }
    }
    Err("No se encontró suficiente historial de sesiones".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn broad_selection_preserves_atm_and_ignores_future_volume() {
        let mut rows = vec![];
        for kind in ["call", "put"] {
            for strike in [496, 500, 504] {
                let side = if kind == "call" { "C" } else { "P" };
                rows.push(
                    json!({"ticker":format!("O:QQQ261009{side}{:08}",strike*1000),
                    "underlying_ticker":"QQQ","expiration_date":"2026-10-09",
                    "contract_type":kind,"shares_per_contract":100,"strike_price":strike}),
                );
            }
        }
        let first = select_contracts_breadth(&rows, "QQQ", "2026-10-09", 500., 3).unwrap();
        assert_eq!(first.len(), 6);
        let original = select_contracts(&rows, "QQQ", "2026-10-09", 500.).unwrap();
        assert!(original.iter().all(|t| first.contains(t)));
        rows.reverse();
        for r in &mut rows {
            r["volume"] = json!(999999);
        }
        assert_eq!(
            first,
            select_contracts_breadth(&rows, "QQQ", "2026-10-09", 500., 3).unwrap()
        );
        assert_eq!(first.iter().collect::<BTreeSet<_>>().len(), 6);
    }
    #[test]
    fn insufficient_breadth_does_not_silently_replace_missing_strikes() {
        let rows = vec![json!({"ticker":"O:QQQ261009C00500000",
            "underlying_ticker":"QQQ","expiration_date":"2026-10-09",
            "contract_type":"call","shares_per_contract":100,"strike_price":500})];
        assert!(select_contracts_breadth(&rows, "QQQ", "2026-10-09", 500., 3).is_err());
        assert!(select_contracts_breadth(&rows, "QQQ", "2026-10-09", 500., 4).is_err());
    }
    #[test]
    fn selection_uses_opening_price_and_side_not_future_volume() {
        let rows = vec![
            json!({"ticker":"O:QQQ261009C00500000","underlying_ticker":"QQQ","expiration_date":"2026-10-09","contract_type":"call","shares_per_contract":100,"strike_price":500,"volume":1}),
            json!({"ticker":"O:QQQ261009C00510000","underlying_ticker":"QQQ","expiration_date":"2026-10-09","contract_type":"call","shares_per_contract":100,"strike_price":510,"volume":100000}),
            json!({"ticker":"O:QQQ261009P00500000","underlying_ticker":"QQQ","expiration_date":"2026-10-09","contract_type":"put","shares_per_contract":100,"strike_price":500}),
        ];
        let selected = select_contracts(&rows, "QQQ", "2026-10-09", 500.).unwrap();
        assert_eq!(
            selected,
            vec!["O:QQQ261009C00500000", "O:QQQ261009P00500000"]
        );
    }
    #[test]
    fn sample_schedule_excludes_cutoff_and_holidays() {
        assert_eq!(
            previous_sessions("2026-10-12", 2).unwrap(),
            vec!["2026-10-08", "2026-10-09"]
        );
        assert_eq!(
            previous_sessions("2026-09-08", 1).unwrap(),
            vec!["2026-09-04"]
        );
        assert_eq!(
            previous_sessions("2026-01-02", 2).unwrap(),
            vec!["2025-12-30", "2025-12-31"]
        );
        assert!(previous_sessions("2024-01-02", 2).is_err());
    }
    #[test]
    fn missing_stock_minutes_are_not_filled_and_current_minutes_are_not_used() {
        let open = calendar::legacy_eastern_ms("2026-10-09 09:30:00").unwrap();
        let report:Report=serde_json::from_value(json!({
            "version":"option-activity-history-v1","collectedMs":now_ms(),
            "request":{"underlying":"GOOGL","ticker":"O:GOOGL261016C00355000","date":"2026-10-09","windowMinutes":5,"maxPages":10},
            "contract":{"ticker":"O:GOOGL261016C00355000","kind":"call","expiration":"2026-10-16","strike":355,"premiumMultiplier":100},
            "session":{"date":"2026-10-09","openMs":open,"closeMs":open+23_400_000,"earlyClose":false},
            "windowClock":"sip_timestamp","scope":"reported-prints-single-requested-contract-equity-RTH",
            "pages":1,"rowsReceived":1,"duplicates":0,"rejected":0,"corrected":0,"conflicts":0,"unidentified":0,
            "retrievalComplete":true,"aggregationComplete":true,"warnings":[],"trades":[],
            "windows":[
                {"startMs":open,"endMs":open+300_000,"trades":0,"observedContracts":0,"observedPremium":0,"contracts":0,"premium":0,"participantClockTrades":0,"sipOnlyTrades":0},
                {"startMs":open+300_000,"endMs":open+600_000,"trades":1,"observedContracts":1,"observedPremium":100,"contracts":1,"premium":100,"participantClockTrades":0,"sipOnlyTrades":1}
            ]
        })).unwrap();
        let start = report.windows[1].start_ms;
        let bar = Bar {
            timestamp_ms: start - 60_000,
            open: 355.,
            high: 356.,
            low: 354.,
            close: 355.,
            volume: 10.,
            vwap: None,
        };
        let (samples, _) = enrich(&report, &[bar], report.session.open_ms + 60_000);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].price_available_ms, start);
        assert!(samples[0].quality_complete);
        assert!(enrich(&report, &[], report.session.open_ms + 60_000)
            .0
            .is_empty());
    }
    #[test]
    fn aggregate_windows_require_every_option_minute_and_never_invent_premium() {
        let open = calendar::legacy_eastern_ms("2026-10-09 09:30:00").unwrap();
        let day = calendar::date_at(open).unwrap();
        let session = match calendar::schedule(day) {
            Day::Open(s) => s,
            _ => panic!(),
        };
        let request = Request {
            underlying: "QQQ".into(),
            ticker: "O:QQQ261009C00500000".into(),
            date: session.date.clone(),
            window_minutes: 5,
            max_pages: 10,
        };
        let metadata = option_activity_history::Contract {
            ticker: request.ticker.clone(),
            kind: "call".into(),
            expiration: session.date.clone(),
            strike: 500.,
            premium_multiplier: Some(100.),
        };
        let bar = |t| Bar {
            timestamp_ms: t,
            open: 1.,
            high: 1.,
            low: 1.,
            close: 1.,
            volume: 2.,
            vwap: None,
        };
        let options: Vec<_> = (5..10).map(|m| bar(open + m * 60_000)).collect();
        let stocks = vec![Bar {
            timestamp_ms: open + 240_000,
            open: 500.,
            high: 500.,
            low: 500.,
            close: 500.,
            volume: 1.,
            vwap: None,
        }];
        let (samples, _) = aggregate_samples(
            &request,
            &metadata,
            &session,
            &options,
            &stocks,
            open + 60_000,
            now_ms(),
        );
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].contracts, 10.);
        assert!(samples[0].premium.is_none());
        assert_eq!(samples[0].measurement, "eligible-option-bars");
        assert!(aggregate_samples(
            &request,
            &metadata,
            &session,
            &options[..4],
            &stocks,
            open + 60_000,
            now_ms()
        )
        .0
        .is_empty());
    }
}
