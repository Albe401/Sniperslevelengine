//! Historical reported-print activity. SIP windows, explicit provenance and bounded REST only.
//! No quote-side inference, OI reconstruction, anomaly score or live-engine mutation.
use crate::{credentials::Credentials, history::checked_url, live::now_ms};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snipers_core::{
    calendar::{self, Day},
    options,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

const PAGE_SIZE: usize = 1000;
const PAGE_BYTES: usize = 4_000_000;
const MAX_TRADES: usize = 100_000;

/// Shared by parsing and read-only source diagnostics; never synthesize a verified ID.
pub(crate) fn reported_identity(row: &Value, received: usize) -> Option<(String, bool)> {
    if let Some(sequence) = row["sequence_number"].as_u64().filter(|n| *n > 0) {
        Some((format!("seq:{sequence}"), true))
    } else if let Some(id) = row["id"].as_str().filter(|s| !s.is_empty() && *s != "0" && s.len() <= 256) {
        Some((format!("id:{}:{id}", row["exchange"].as_u64()?), true))
    } else {
        Some((format!("unverified:{received}"), false))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub underlying: String,
    pub ticker: String,
    pub date: String,
    pub window_minutes: u64,
    pub max_pages: usize,
}
impl Request {
    fn session(&self, now: u64) -> Result<calendar::Session, String> {
        if !crate::option_print_scanner::valid_symbol(&self.underlying)
            || self.ticker.len() > 64
            || options::underlying(&self.ticker) != Some(self.underlying.as_str())
            || !(1..=100).contains(&self.max_pages)
            || ![1, 5, 15, 30].contains(&self.window_minutes)
        {
            return Err("Identidad, ventana o límite de páginas no válido".into());
        }
        let (start, _) = calendar::day_bounds(&self.date).ok_or("Fecha no válida")?;
        let date = calendar::date_at(start).ok_or("Fecha no válida")?;
        if Some(date) >= calendar::date_at(now) {
            return Err("Solo sesiones históricas cerradas".into());
        }
        match calendar::schedule(date) {
            Day::Open(session) if session.date == self.date => Ok(session),
            _ => Err("Sesión cerrada o fuera del calendario verificado".into()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    pub ticker: String,
    pub kind: String,
    pub expiration: String,
    pub strike: f64,
    pub premium_multiplier: Option<f64>,
}
pub(crate) fn contract(request: &Request, row: &Value) -> Result<Contract, String> {
    let raw = request
        .ticker
        .strip_prefix("O:")
        .ok_or("Ticker no válido")?;
    let tail = &raw[raw.len() - 15..]; // Request validates the OCC shape first.
    let expiration = format!("20{}-{}-{}", &tail[..2], &tail[2..4], &tail[4..6]);
    let kind = if &tail[6..7] == "C" { "call" } else { "put" };
    let strike = tail[7..].parse::<u64>().map_err(|_| "Strike no válido")? as f64 / 1000.;
    if row["ticker"] != request.ticker
        || row["underlying_ticker"] != request.underlying
        || row["expiration_date"] != expiration
        || row["contract_type"] != kind
        || row["strike_price"].as_f64() != Some(strike)
        || strike <= 0.
        || expiration < request.date
    {
        return Err("Referencia histórica no coincide con el contrato solicitado".into());
    }
    Ok(Contract {
        ticker: request.ticker.clone(),
        kind: kind.into(),
        expiration,
        strike,
        premium_multiplier: options::verified_multiplier(row, &request.underlying),
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    pub identity: String,
    pub identity_verified: bool,
    pub sip_ns: u64,
    pub participant_ns: Option<u64>,
    pub price: f64,
    pub contracts: f64,
    pub premium: Option<f64>,
    pub conditions: Vec<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub start_ms: u64,
    pub end_ms: u64,
    pub trades: usize,
    pub observed_contracts: f64,
    pub observed_premium: f64,
    /// Certified only against completed retrieval and validated reported prints.
    pub contracts: Option<f64>,
    pub premium: Option<f64>,
    pub participant_clock_trades: usize,
    pub sip_only_trades: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub date: String,
    pub open_ms: u64,
    pub close_ms: u64,
    pub early_close: bool,
}
impl From<calendar::Session> for Session {
    fn from(s: calendar::Session) -> Self {
        Self {
            date: s.date,
            open_ms: s.open_ms,
            close_ms: s.close_ms,
            early_close: s.early_close,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub version: String,
    pub collected_ms: u64,
    pub request: Request,
    pub contract: Contract,
    pub session: Session,
    pub window_clock: String,
    pub scope: String,
    pub pages: usize,
    pub rows_received: usize,
    pub duplicates: usize,
    pub rejected: usize,
    pub corrected: usize,
    pub conflicts: usize,
    pub unidentified: usize,
    pub retrieval_complete: bool,
    pub aggregation_complete: bool,
    pub warnings: Vec<String>,
    pub windows: Vec<Window>,
    pub trades: Vec<Trade>,
}

struct Accumulator {
    report: Report,
    identities: BTreeMap<String, Value>,
    range_start_ms: u64,
    range_end_ms: u64,
}
impl Accumulator {
    fn new(request: Request, contract: Contract, session: calendar::Session) -> Self {
        let step = request.window_minutes * 60_000;
        let windows = (session.open_ms..session.close_ms)
            .step_by(step as usize)
            .map(|start| Window {
                start_ms: start,
                end_ms: (start + step).min(session.close_ms),
                trades: 0,
                observed_contracts: 0.,
                observed_premium: 0.,
                contracts: None,
                premium: None,
                participant_clock_trades: 0,
                sip_only_trades: 0,
            })
            .collect();
        Self {
            range_start_ms: session.open_ms,
            range_end_ms: session.close_ms,
            report: Report {
                version: "option-activity-history-v1".into(),
                collected_ms: now_ms(),
                request,
                contract,
                session: session.into(),
                window_clock: "sip_timestamp".into(),
                scope: "reported-prints-single-requested-contract-equity-RTH".into(),
                pages: 0,
                rows_received: 0,
                duplicates: 0,
                rejected: 0,
                corrected: 0,
                conflicts: 0,
                unidentified: 0,
                retrieval_complete: false,
                aggregation_complete: false,
                warnings: vec![],
                windows,
                trades: vec![],
            },
            identities: BTreeMap::new(),
        }
    }
    fn push(&mut self, row: &Value) {
        self.report.rows_received += 1;
        if row["correction"].as_u64().is_some_and(|n| n != 0) {
            self.report.corrected += 1;
            return; // No speculative cancellation/replacement; mark the entire retrieval unusable.
        }
        match self.parse(row) {
            Some(trade) => {
                if !trade.identity_verified {
                    self.report.unidentified += 1;
                }
                if let Some(previous) = self.identities.get(&trade.identity) {
                    if previous == row {
                        self.report.duplicates += 1;
                    } else {
                        self.report.conflicts += 1;
                    }
                    return;
                }
                if self.report.trades.len() >= MAX_TRADES {
                    self.report.rejected += 1;
                    return;
                }
                self.identities.insert(trade.identity.clone(), row.clone());
                let index = ((trade.sip_ns / 1_000_000 - self.range_start_ms)
                    / (self.report.request.window_minutes * 60_000))
                    as usize;
                let window = &mut self.report.windows[index];
                window.trades += 1;
                window.observed_contracts += trade.contracts;
                window.observed_premium += trade.premium.unwrap_or(0.);
                if trade.participant_ns.is_some() {
                    window.participant_clock_trades += 1;
                } else {
                    window.sip_only_trades += 1;
                }
                self.report.trades.push(trade);
            }
            None => self.report.rejected += 1,
        }
    }
    fn parse(&self, row: &Value) -> Option<Trade> {
        let sip_ns = row["sip_timestamp"].as_u64()?;
        if sip_ns < self.range_start_ms * 1_000_000 || sip_ns >= self.range_end_ms * 1_000_000 {
            return None;
        }
        let participant_ns = if row["participant_timestamp"].is_null() {
            None
        } else {
            Some(row["participant_timestamp"].as_u64().filter(|n| *n > 0)?)
        };
        if !row["correction"].is_null() && row["correction"].as_u64().is_none() {
            return None;
        }
        let contracts = if row["decimal_size"].is_null() {
            row["size"].as_f64()?
        } else if let Some(n) = row["decimal_size"].as_f64() {
            n
        } else {
            row["decimal_size"]
                .as_str()
                .filter(|s| s.len() <= 64)?
                .parse::<f64>()
                .ok()?
        };
        let price = row["price"].as_f64()?;
        if !contracts.is_finite() || contracts <= 0. || !price.is_finite() || price <= 0. {
            return None;
        }
        let (identity, identity_verified) = reported_identity(row, self.report.rows_received)?;
        let conditions = if row["conditions"].is_null() {
            vec![]
        } else {
            row["conditions"]
                .as_array()?
                .iter()
                .map(Value::as_u64)
                .collect::<Option<Vec<_>>>()?
        };
        let premium = self
            .report
            .contract
            .premium_multiplier
            .map(|m| price * contracts * m);
        if premium.is_some_and(|p| !p.is_finite()) {
            return None;
        }
        Some(Trade {
            identity,
            identity_verified,
            sip_ns,
            participant_ns,
            price,
            contracts,
            premium,
            conditions,
        })
    }
    fn finish(mut self, terminal: bool) -> Report {
        let r = &mut self.report;
        r.collected_ms = now_ms();
        r.retrieval_complete = terminal;
        r.aggregation_complete = terminal
            && r.rejected == 0
            && r.corrected == 0
            && r.conflicts == 0
            && r.unidentified == 0
            && !r.trades.is_empty()
            && r.windows
                .iter()
                .all(|w| w.observed_contracts.is_finite() && w.observed_premium.is_finite());
        if !terminal {
            r.warnings
                .push("Paginación incompleta; totales no certificados".into());
        }
        if r.trades.is_empty() {
            r.warnings
                .push("Sin trades válidos; ausencia no certificada como volumen cero".into());
        }
        if r.rejected + r.corrected + r.conflicts > 0 {
            r.warnings
                .push("Filas rechazadas, corregidas o en conflicto; requiere revisión".into());
        }
        if r.unidentified > 0 {
            r.warnings.push(
                "Identidad de trades no verificable; deduplicación y totales pendientes".into(),
            );
        }
        if r.contract.premium_multiplier.is_none() {
            r.warnings
                .push("Multiplicador no verificado; primas pendientes".into());
        }
        for w in &mut r.windows {
            w.contracts = r.aggregation_complete.then_some(w.observed_contracts);
            w.premium = (r.aggregation_complete && r.contract.premium_multiplier.is_some())
                .then_some(w.observed_premium);
        }
        r.trades.sort_by_key(|t| t.sip_ns);
        self.report
    }
}

pub(crate) async fn get(
    client: &reqwest::Client,
    key: &Credentials,
    url: reqwest::Url,
) -> Result<Value, String> {
    let mut response = client
        .get(url)
        .bearer_auth(&key.0)
        .send()
        .await
        .map_err(|_| "REST: conexión interrumpida")?;
    if !response.status().is_success() {
        return Err(format!("REST: HTTP {}", response.status().as_u16()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "REST: página interrumpida")?
    {
        if bytes.len() + chunk.len() > PAGE_BYTES {
            return Err("REST: página demasiado grande".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let body: Value = serde_json::from_slice(&bytes).map_err(|_| "REST: JSON no válido")?;
    if body["status"] != "OK" {
        return Err("REST: estado no confirmado".into());
    }
    Ok(body)
}

fn next_page(
    value: &Value,
    path: &str,
    visited: &BTreeSet<String>,
) -> Result<Option<reqwest::Url>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let next = value.as_str().ok_or("Cursor de paginación no válido")?;
    if next.is_empty() {
        return Ok(None);
    }
    let url = checked_url(next, path).map_err(str::to_owned)?;
    if visited.contains(url.as_str()) {
        return Err("Cursor de paginación repetido".into());
    }
    Ok(Some(url))
}

pub async fn collect(key: &Credentials, request: Request) -> Result<Report, String> {
    let session = request.session(now_ms())?;
    let start = session.open_ms;
    let end = session.close_ms;
    collect_range(key, request, session, start, end, false).await
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowReport {
    pub version: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub capture_started_ms: u64,
    pub report: Report,
}
fn prospective_session(
    request: &Request,
    start: u64,
    end: u64,
    now: u64,
) -> Result<calendar::Session, String> {
    if request.window_minutes != 5
        || !(1..=20).contains(&request.max_pages)
        || !crate::option_print_scanner::valid_symbol(&request.underlying)
        || options::underlying(&request.ticker) != Some(request.underlying.as_str())
        || request.ticker.len() > 64
    {
        return Err("Solicitud de operaciones prospectivas fuera de límite".into());
    }
    let session = match calendar::date_at(now).map(calendar::schedule) {
        Some(Day::Open(s)) if s.date == request.date => s,
        _ => return Err("WAITING_SESSION: no hay sesión del día solicitado".into()),
    };
    if start < session.open_ms
        || end > session.close_ms
        || end.checked_sub(start) != Some(300_000)
        || !(start - session.open_ms).is_multiple_of(300_000)
        || now < end
        || now - end > 120_000
    {
        return Err("Ventana abierta, atrasada o fuera de sesión".into());
    }
    Ok(session)
}
pub async fn collect_window(
    key: &Credentials,
    request: Request,
    start: u64,
    end: u64,
) -> Result<WindowReport, String> {
    let started = now_ms();
    let session = prospective_session(&request, start, end, started)?;
    let report = collect_range(key, request, session, start, end, true).await?;
    Ok(WindowReport {
        version: "option-premium-window-evidence-v1".into(),
        start_ms: start,
        end_ms: end,
        capture_started_ms: started,
        report,
    })
}
async fn collect_range(
    key: &Credentials,
    request: Request,
    session: calendar::Session,
    start: u64,
    end: u64,
    window: bool,
) -> Result<Report, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "REST no disponible")?;
    let reference_path = format!("/v3/reference/options/contracts/{}", request.ticker);
    let mut reference_url = checked_url(
        &format!("https://api.massive.com{reference_path}"),
        &reference_path,
    )
    .map_err(str::to_owned)?;
    reference_url
        .query_pairs_mut()
        .append_pair("as_of", &request.date);
    let reference = get(&client, key, reference_url).await?;
    let metadata = contract(&request, &reference["results"])?;
    let mut acc = Accumulator::new(request, metadata, session);
    if window {
        acc.range_start_ms = start;
        acc.range_end_ms = end;
        acc.report.version = "option-activity-window-v1".into();
        acc.report.scope = "reported-prints-single-requested-contract-equity-RTH-window".into();
        acc.report.windows = vec![Window {
            start_ms: start,
            end_ms: end,
            trades: 0,
            observed_contracts: 0.,
            observed_premium: 0.,
            contracts: None,
            premium: None,
            participant_clock_trades: 0,
            sip_only_trades: 0,
        }];
    }
    let path = format!("/v3/trades/{}", acc.report.request.ticker);
    let mut url =
        checked_url(&format!("https://api.massive.com{path}"), &path).map_err(str::to_owned)?;
    url.query_pairs_mut()
        .append_pair("timestamp.gte", &(start * 1_000_000).to_string())
        .append_pair("timestamp.lt", &(end * 1_000_000).to_string())
        .append_pair("sort", "timestamp")
        .append_pair("order", "asc")
        .append_pair("limit", &PAGE_SIZE.to_string());
    let mut visited = BTreeSet::new();
    let mut terminal = false;
    for _ in 0..acc.report.request.max_pages {
        if window && now_ms() > end + 120_000 {
            acc.report
                .warnings
                .push("Captura fuera de plazo; no se solicitan más páginas".into());
            break;
        }
        if !visited.insert(url.to_string()) {
            acc.report.warnings.push("Cursor repetido".into());
            break;
        }
        let page = match get(&client, key, url).await {
            Ok(page) => page,
            Err(error) => {
                acc.report.warnings.push(error);
                break;
            }
        };
        let rows = match page["results"].as_array() {
            Some(rows) if rows.len() <= PAGE_SIZE => rows,
            _ => {
                acc.report
                    .warnings
                    .push("Formato o tamaño de filas no válido".into());
                break;
            }
        };
        acc.report.pages += 1;
        for row in rows {
            acc.push(row);
        }
        match next_page(&page["next_url"], &path, &visited) {
            Ok(Some(next)) => url = next,
            Ok(None) => {
                terminal = true;
                break;
            }
            Err(error) => {
                acc.report.warnings.push(error);
                break;
            }
        }
    }
    Ok(acc.finish(terminal))
}

/// Immutable local artifact; partial reports retain null certified totals for audit/retry.
pub fn save(directory: &Path, report: &Report) -> Result<PathBuf, String> {
    if report.version != "option-activity-history-v1"
        || report.trades.len() > MAX_TRADES
        || report.request.session(report.collected_ms).is_err()
    {
        return Err("Reporte no válido".into());
    }
    std::fs::create_dir_all(directory).map_err(|_| "No se pudo crear el historial de actividad")?;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let path = directory.join(format!(
        "activity-{}-{}-{}-{}-{}.json",
        report.request.underlying,
        report.request.date,
        now_ms(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| "No se pudo crear el reporte")?;
    let mut writer = std::io::BufWriter::new(file);
    serde_json::to_writer(&mut writer, report).map_err(|_| "No se pudo escribir el reporte")?;
    writer.flush().map_err(|_| "Escritura incompleta")?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|_| "No se pudo confirmar la escritura")?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn prospective_request_requires_current_closed_fresh_window() {
        let now = calendar::legacy_eastern_ms("2026-10-12 09:40:00").unwrap();
        let mut r = request("2026-10-12");
        r.ticker = "O:QQQ261012C00600000".into();
        assert!(prospective_session(&r, now - 300_000, now, now).is_ok());
        assert!(prospective_session(&r, now - 300_000, now, now - 1).is_err());
        assert!(prospective_session(&r, now - 300_000, now, now + 120_001).is_err());
        assert!(prospective_session(&r, now - 300_000, now, now + 86_400_000).is_err());
        r.max_pages = 21;
        assert!(prospective_session(&r, now - 300_000, now, now).is_err());
    }
    #[test]
    fn window_accumulator_uses_window_origin_and_rejects_other_minutes() {
        let mut a = acc();
        let start = a.report.session.open_ms + 300_000;
        let end = start + 300_000;
        a.range_start_ms = start;
        a.range_end_ms = end;
        a.report.windows = vec![a.report.windows[1].clone()];
        a.push(&row(start, 1));
        a.push(&row(start - 1, 2));
        a.push(&row(end, 3));
        assert_eq!(a.report.windows[0].trades, 1);
        let r = a.finish(true);
        assert_eq!(r.rejected, 2);
        assert!(!r.aggregation_complete);
        assert_eq!(r.windows[0].premium, None);
    }
    fn request(date: &str) -> Request {
        Request {
            underlying: "QQQ".into(),
            ticker: "O:QQQ260409C00569000".into(),
            date: date.into(),
            window_minutes: 5,
            max_pages: 10,
        }
    }
    fn acc() -> Accumulator {
        let req = request("2026-04-09");
        let session = req
            .session(calendar::rfc3339_ms("2026-10-10T12:00:00Z").unwrap())
            .unwrap();
        Accumulator::new(
            req,
            Contract {
                ticker: "O:QQQ260409C00569000".into(),
                kind: "call".into(),
                expiration: "2026-04-09".into(),
                strike: 569.,
                premium_multiplier: Some(100.),
            },
            session,
        )
    }
    fn row(ms: u64, seq: u64) -> Value {
        json!({"sip_timestamp":ms*1_000_000,"sequence_number":seq,"size":1,"price":2.,"conditions":[209]})
    }
    #[test]
    fn windows_include_small_trades_and_decimal_sizes_without_mixing_clocks() {
        let mut a = acc();
        let open = a.report.session.open_ms;
        let mut r = row(open, 1);
        r["participant_timestamp"] = json!((open - 1000) * 1_000_000);
        a.push(&r);
        let mut fractional = row(open + 300_000, 2);
        fractional["decimal_size"] = json!(0.5);
        a.push(&fractional);
        let r = a.finish(true);
        assert!(r.aggregation_complete);
        assert_eq!(r.windows.len(), 78);
        assert_eq!(r.windows[0].contracts, Some(1.));
        assert_eq!(r.windows[0].premium, Some(200.));
        assert_eq!(r.windows[1].contracts, Some(0.5));
        assert_eq!(r.windows[1].premium, Some(100.));
        assert_eq!(r.windows[2].contracts, Some(0.));
        assert_eq!(r.trades[0].conditions, vec![209]);
        assert_eq!(r.windows[0].participant_clock_trades, 1);
        assert_eq!(r.windows[1].sip_only_trades, 1);
    }
    #[test]
    fn deduplicates_across_pages_and_invalidates_conflicting_identity() {
        let mut a = acc();
        let r = row(a.report.session.open_ms, 1);
        a.push(&r);
        a.push(&r);
        assert_eq!(a.report.duplicates, 1);
        let mut conflict = r;
        conflict["price"] = json!(3.);
        a.push(&conflict);
        let r = a.finish(true);
        assert_eq!(r.conflicts, 1);
        assert!(!r.aggregation_complete);
        assert_eq!(r.windows[0].contracts, None);
    }
    #[test]
    fn partial_and_empty_retrieval_never_certify_zero() {
        let mut a = acc();
        a.push(&row(a.report.session.open_ms, 1));
        let r = a.finish(false);
        assert!(!r.retrieval_complete);
        assert_eq!(r.windows[0].contracts, None);
        assert_eq!(r.windows[0].observed_contracts, 1.);
        let empty = acc().finish(true);
        assert!(empty.retrieval_complete);
        assert!(!empty.aggregation_complete);
        assert_eq!(empty.windows[0].contracts, None);
    }
    #[test]
    fn rejects_invalid_corrected_unidentified_and_outside_session_rows() {
        let mut a = acc();
        let open = a.report.session.open_ms;
        let close = a.report.session.close_ms;
        a.push(&row(close, 1));
        a.push(&row(open - 1, 2));
        let mut bad = row(open, 3);
        bad["size"] = json!(-1);
        a.push(&bad);
        let mut correction = row(open, 4);
        correction["correction"] = json!(1);
        a.push(&correction);
        let mut no_id = row(open, 5);
        no_id.as_object_mut().unwrap().remove("sequence_number");
        a.push(&no_id);
        let r = a.finish(true);
        assert_eq!(r.rejected, 3);
        assert_eq!(r.corrected, 1);
        assert_eq!(r.unidentified, 1);
        assert!(!r.aggregation_complete);
    }
    #[test]
    fn missing_multiplier_preserves_volume_but_not_premium() {
        let mut a = acc();
        a.report.contract.premium_multiplier = None;
        a.push(&row(a.report.session.open_ms, 1));
        let r = a.finish(true);
        assert!(r.aggregation_complete);
        assert_eq!(r.windows[0].contracts, Some(1.));
        assert_eq!(r.windows[0].premium, None);
    }
    #[test]
    fn validates_contract_reference_and_calendar() {
        let req = request("2026-04-09");
        let mut reference = json!({"ticker":req.ticker,"underlying_ticker":"QQQ","expiration_date":"2026-04-09","contract_type":"call","strike_price":569.,"shares_per_contract":100});
        assert_eq!(
            contract(&req, &reference).unwrap().premium_multiplier,
            Some(100.)
        );
        reference["strike_price"] = json!(570.);
        assert!(contract(&req, &reference).is_err());
        let now = calendar::rfc3339_ms("2026-10-10T12:00:00Z").unwrap();
        assert!(request("2026-10-10").session(now).is_err());
        assert!(request("2026-04-03").session(now).is_err());
        assert_eq!(request("2025-04-09").session(now).unwrap().date, "2025-04-09");
        assert!(request("2023-04-09").session(now).is_err());
        let mut short = request("2026-11-27");
        short.ticker = "O:QQQ261127C00569000".into();
        let session = short
            .session(calendar::rfc3339_ms("2026-12-01T12:00:00Z").unwrap())
            .unwrap();
        assert_eq!((session.close_ms - session.open_ms) / 60_000, 210);
    }
    #[test]
    fn cursor_guard_rejects_external_repeated_and_malformed_urls() {
        let path = "/v3/trades/O:QQQ260409C00569000";
        let mut seen = BTreeSet::new();
        let url = format!("https://api.massive.com{path}?cursor=x&apiKey=secret");
        let next = next_page(&json!(url), path, &seen).unwrap().unwrap();
        assert!(!next.as_str().contains("secret"));
        seen.insert(next.to_string());
        assert!(next_page(&json!(url), path, &seen).is_err());
        assert!(next_page(&json!("https://other.example/v3/trades/x"), path, &seen).is_err());
        assert!(next_page(&json!(123), path, &seen).is_err());
    }
    #[test]
    fn immutable_storage_roundtrip() {
        let mut a = acc();
        a.push(&row(a.report.session.open_ms, 1));
        let r = a.finish(true);
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-option-activity-history");
        let p = save(&directory, &r).unwrap();
        let q = save(&directory, &r).unwrap();
        assert_ne!(p, q);
        let loaded: Report = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        assert_eq!(loaded.trades.len(), 1);
        assert_eq!(loaded.windows[0].premium, Some(200.));
        std::fs::remove_file(p).unwrap();
        std::fs::remove_file(q).unwrap();
    }
    #[test]
    fn decimal_size_strings_from_real_rest_rows_are_not_discarded() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../fixtures/option-trades-sip-real-2026-10-09.json"
        ))
        .unwrap();
        let req = Request {
            underlying: "GOOGL".into(),
            ticker: fixture["ticker"].as_str().unwrap().into(),
            date: "2026-10-09".into(),
            window_minutes: 5,
            max_pages: 10,
        };
        let session = req
            .session(calendar::rfc3339_ms("2026-10-10T12:00:00Z").unwrap())
            .unwrap();
        let mut a = Accumulator::new(
            req,
            Contract {
                ticker: fixture["ticker"].as_str().unwrap().into(),
                kind: "call".into(),
                expiration: "2026-10-16".into(),
                strike: 355.,
                premium_multiplier: Some(100.),
            },
            session,
        );
        for row in fixture["rows"].as_array().unwrap() {
            a.push(row);
        }
        let r = a.finish(true);
        assert!(r.aggregation_complete);
        assert_eq!(r.trades.len(), 2);
        assert_eq!(r.windows[0].contracts, Some(7.));
        assert_eq!(r.windows[0].premium, Some(3280.));
        let mut a = acc();
        let mut row = row(a.report.session.open_ms, 1);
        row["decimal_size"] = json!("NaN");
        a.push(&row);
        assert!(!a.finish(true).aggregation_complete);
    }
    #[test]
    fn historical_zero_sequences_do_not_collapse_distinct_trades() {
        let mut a = acc();
        let open = a.report.session.open_ms;
        let mut first = row(open, 0);
        first["id"] = json!("");
        a.push(&first);
        a.push(&row(open + 1000, 0));
        a.push(&first);
        let r = a.finish(true);
        assert_eq!(r.trades.len(), 3);
        assert_eq!(r.conflicts, 0);
        assert_eq!(r.unidentified, 3);
        assert_eq!(r.duplicates, 0);
        assert_eq!(r.windows[0].observed_contracts, 3.);
        assert_eq!(r.windows[0].contracts, None);
    }
}
