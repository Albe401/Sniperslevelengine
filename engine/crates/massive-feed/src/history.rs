//! Minute history is keyed by timestamp, never added to streamed volume.
use crate::{
    chart_history::{self, ChartHistory},
    credentials::Credentials,
    live::{now_ms, Bar, SharedState},
};
use snipers_core::calendar::{self, Day};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryState {
    pub phase: String,
    pub detail: String,
    pub from_ms: Option<u64>,
    pub through_ms: Option<u64>,
    pub updated_ms: Option<u64>,
    pub recovered_bars: usize,
}

#[derive(Deserialize)]
struct RestBar {
    t: u64,
    o: f64,
    h: f64,
    l: f64,
    c: f64,
    v: f64,
    vw: Option<f64>,
}
#[derive(Deserialize)]
struct Page {
    status: String,
    ticker: Option<String>,
    #[serde(default)]
    results: Vec<RestBar>,
    next_url: Option<String>,
}

fn convert(row: RestBar, from: u64, to: u64) -> Result<Option<Bar>, &'static str> {
    if ![row.o, row.h, row.l, row.c]
        .iter()
        .all(|v| v.is_finite() && *v > 0.0)
        || !row.v.is_finite()
        || row.v < 0.0
        || row.h < row.o.max(row.c)
        || row.l > row.o.min(row.c)
        || !row.t.is_multiple_of(60_000)
    {
        return Err("Massive devolvió una vela histórica no válida.");
    }
    if row.t < from || row.t.saturating_add(59_999) > to {
        return Ok(None);
    }
    Ok(Some(Bar {
        timestamp_ms: row.t,
        open: row.o,
        high: row.h,
        low: row.l,
        close: row.c,
        volume: row.v,
        vwap: row.vw.filter(|v| v.is_finite()),
    }))
}

pub(crate) fn checked_url(value: &str, path: &str) -> Result<reqwest::Url, &'static str> {
    let mut url = reqwest::Url::parse(value).map_err(|_| "URL histórica inválida.")?;
    if url.scheme() != "https"
        || url.host_str() != Some("api.massive.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != path
        || url.fragment().is_some()
    {
        return Err("Paginación histórica fuera del endpoint permitido.");
    }
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| k != "apiKey")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    url.set_query(None);
    url.query_pairs_mut().extend_pairs(pairs);
    Ok(url)
}

pub async fn fetch(
    key: &Credentials,
    symbol: &str,
    from: u64,
    to: u64,
) -> Result<Vec<Bar>, &'static str> {
    fetch_adjusted(key, symbol, from, to, true).await
}

/// Option strikes are historical nominal prices; do not compare them with split-adjusted stocks.
pub async fn fetch_unadjusted(
    key: &Credentials,
    symbol: &str,
    from: u64,
    to: u64,
) -> Result<Vec<Bar>, &'static str> {
    fetch_adjusted(key, symbol, from, to, false).await
}

async fn fetch_adjusted(
    key: &Credentials,
    symbol: &str,
    from: u64,
    to: u64,
    adjusted: bool,
) -> Result<Vec<Bar>, &'static str> {
    if snipers_core::universe::stock_asset(symbol).is_none() {
        return Err("Activo no habilitado para historicos Stocks.");
    }
    if from > to || to - from > 31 * 86_400_000 {
        return Err("Rango histórico fuera del límite de 31 días.");
    }
    let path = format!("/v2/aggs/ticker/{symbol}/range/1/minute/{from}/{to}");
    let mut url = checked_url(
        &format!("https://api.massive.com{path}?adjusted={adjusted}&sort=asc&limit=50000"),
        &path,
    )?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "No se pudo preparar REST.")?;
    let mut seen = BTreeSet::new();
    let mut bars = BTreeMap::new();
    for _ in 0..20 {
        if !seen.insert(url.to_string()) {
            return Err("Paginación histórica repetida.");
        }
        let mut page = None;
        for attempt in 0..3 {
            let result = client.get(url.clone()).bearer_auth(&key.0).send().await;
            match result {
                Ok(mut response) if response.status().is_success() => {
                    let mut bytes = Vec::new();
                    while let Some(chunk) = response
                        .chunk()
                        .await
                        .map_err(|_| "Respuesta histórica interrumpida.")?
                    {
                        if bytes.len() + chunk.len() > 16_000_000 {
                            return Err("Página histórica excede el límite de tamaño.");
                        }
                        bytes.extend_from_slice(&chunk);
                    }
                    page = Some(
                        serde_json::from_slice::<Page>(&bytes)
                            .map_err(|_| "Respuesta histórica inválida.")?,
                    );
                    break;
                }
                Ok(response) if [401, 403].contains(&response.status().as_u16()) => {
                    return Err("Acceso al historial rechazado (401/403).")
                }
                Ok(response)
                    if response.status().as_u16() == 429 || response.status().is_server_error() =>
                {
                    if attempt == 2 {
                        return Err("Historial temporalmente limitado por Massive.");
                    }
                    let delay = response
                        .headers()
                        .get("retry-after")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(1 << attempt)
                        .min(60);
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
                Ok(_) => return Err("Solicitud histórica rechazada por Massive."),
                Err(_) => {
                    if attempt == 2 {
                        return Err(
                            "No se pudo recuperar el historial: conexión REST interrumpida.",
                        );
                    }
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                }
            }
        }
        let page = page.ok_or("No se recibió historial.")?;
        if !["OK", "DELAYED"].contains(&page.status.as_str()) {
            return Err("Massive no confirmó una respuesta histórica válida.");
        }
        if page.status == "DELAYED" {
            return Err("El historial fue marcado como delayed; no se aplicó al monitor realtime.");
        }
        if page
            .ticker
            .as_deref()
            .is_some_and(|ticker| ticker != symbol)
        {
            return Err("Símbolo histórico inesperado.");
        }
        for row in page.results {
            if let Some(bar) = convert(row, from, to)? {
                bars.insert(bar.timestamp_ms, bar);
            }
        }
        if bars.len() > 44_640 {
            return Err("Historial excede el rango permitido.");
        }
        match page.next_url {
            Some(next) => url = checked_url(&next, &path)?,
            None => return Ok(bars.into_values().collect()),
        }
    }
    Err("Se alcanzó el límite de páginas; historial no aplicado.")
}

/// One-off chart request. Historical minutes never enter the per-second live IPC stream.
pub async fn fetch_chart_history(
    key: &Credentials,
    symbol: &str,
    now: u64,
) -> Result<ChartHistory, &'static str> {
    let mut day = calendar::date_at(now).ok_or("Fecha del gráfico no disponible.")?;
    let mut oldest = None;
    let mut count = 0;
    for _ in 0..75 {
        match calendar::schedule(day) {
            Day::Open(session) if session.open_ms <= now => {
                oldest = Some(session.open_ms);
                count += 1;
                if count == 30 { break; }
            }
            Day::Unknown => return Err("Calendario de negociación no disponible."),
            _ => {}
        }
        day = day.pred_opt().ok_or("Fecha del gráfico no disponible.")?;
    }
    if count < 30 { return Err("No se pudieron identificar 30 sesiones."); }
    let from = oldest.unwrap();
    let to = now / 60_000 * 60_000 - 1;
    let mut minutes = BTreeMap::new();
    let mut start = from;
    while start <= to {
        let end = to.min(start.saturating_add(30 * 86_400_000 - 1));
        for bar in fetch(key, symbol, start, end).await? {
            minutes.insert(bar.timestamp_ms, bar);
        }
        start = end.saturating_add(1);
    }
    let mut chart = chart_history::collect_with_limit(&minutes, now, 31);
    chart.sessions.retain(|session| chart.minutes.iter().any(|bar| bar.timestamp_ms >= session.open_ms && bar.timestamp_ms < session.close_ms));
    if chart.sessions.len() > 30 {
        chart.sessions.drain(..chart.sessions.len() - 30);
    }
    if let Some(first) = chart.sessions.first() {
        chart.minutes.retain(|bar| bar.timestamp_ms >= first.open_ms);
    }
    Ok(chart)
}

/// Request all pages before merging; a partial HTTP result never marks recovery complete.
pub async fn recover(key: &Credentials, state: &SharedState) {
    let to = now_ms() / 60_000 * 60_000 - 1;
    let (from, revision, symbol) = {
        let mut s = state.lock().unwrap();
        let from = s
            .history
            .through_ms
            .map(|t| t.saturating_sub(120_000) / 60_000 * 60_000)
            .unwrap_or_else(|| to.saturating_sub(21 * 86_400_000) / 60_000 * 60_000)
            .max(to.saturating_sub(21 * 86_400_000) / 60_000 * 60_000);
        s.history.phase = "loading".into();
        s.history.detail = "Recuperando minutos completos; la recepción WebSocket continúa".into();
        (from, s.minute_revision, s.symbol.clone())
    };
    let result = fetch(key, &symbol, from, to).await;
    let mut s = state.lock().unwrap();
    match result {
        Ok(bars) => {
            let count = bars.len();
            merge(&mut s, bars, revision);
            s.history=HistoryState { phase:"ready".into(), detail:"Consulta histórica completa. Los intervalos sin operaciones elegibles no generan velas.".into(), from_ms:Some(from),through_ms:Some(to),updated_ms:Some(now_ms()),recovered_bars:count };
        }
        Err(detail) => {
            s.history.phase = "failed".into();
            s.history.detail = detail.into();
        }
    }
}

pub async fn worker(key: &Credentials, state: SharedState) {
    let notify = state.lock().unwrap().history_notify.clone();
    recover(key, &state).await;
    loop {
        notify.notified().await;
        recover(key, &state).await;
    }
}

pub(crate) fn trim(s: &mut crate::live::LiveState) {
    while s.minutes.len() > 30_000 {
        if let Some((t, _)) = s.minutes.pop_first() {
            s.minute_versions.remove(&t);
        }
    }
}
fn merge(s: &mut crate::live::LiveState, bars: Vec<Bar>, revision: u64) {
    s.minute_revision += 1;
    for bar in bars {
        // A WS update received while REST was in flight wins over its older response.
        if s.minute_versions
            .get(&bar.timestamp_ms)
            .is_none_or(|v| *v <= revision)
        {
            s.minutes.insert(bar.timestamp_ms, bar);
        }
    }
    trim(s);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bar(t: u64, v: f64) -> Bar {
        Bar {
            timestamp_ms: t,
            open: 10.0,
            high: 12.0,
            low: 9.0,
            close: 11.0,
            volume: v,
            vwap: Some(10.5),
        }
    }
    #[test]
    fn merge_replaces_duplicates_but_preserves_concurrent_live_correction() {
        let mut s = crate::live::LiveState::default();
        merge(&mut s, vec![bar(60_000, 10.0), bar(60_000, 20.0)], 0);
        assert_eq!(s.minutes[&60_000].volume, 20.0);
        s.minute_versions.insert(60_000, 2);
        s.minutes.insert(60_000, bar(60_000, 30.0));
        merge(&mut s, vec![bar(60_000, 15.0), bar(120_000, 5.0)], 1);
        assert_eq!(s.minutes[&60_000].volume, 30.0);
        assert_eq!(s.minutes.len(), 2);
    }
    #[test]
    fn pagination_cannot_forward_credentials_to_another_host_or_endpoint() {
        let path = "/v2/aggs/ticker/QQQ/range/1/minute/0/1";
        for bad in [
            "https://evil.example",
            "http://api.massive.com",
            "https://api.massive.com:444",
            "https://user@api.massive.com",
        ] {
            assert!(checked_url(&format!("{bad}{path}"), path).is_err());
        }
        assert!(checked_url("https://api.massive.com/other", path).is_err());
        let clean = checked_url(
            &format!("https://api.massive.com{path}?cursor=abc&apiKey=secret"),
            path,
        )
        .unwrap();
        assert!(!clean.as_str().contains("secret"));
        assert!(clean.as_str().contains("cursor=abc"));
    }
    #[test]
    fn unfinished_and_out_of_range_bars_are_not_committed() {
        let row = || RestBar {
            t: 60_000,
            o: 10.0,
            h: 12.0,
            l: 9.0,
            c: 11.0,
            v: 20.0,
            vw: Some(10.5),
        };
        assert!(convert(row(), 0, 119_998).unwrap().is_none());
        assert!(convert(row(), 0, 119_999).unwrap().is_some());
        assert!(convert(row(), 120_000, 180_000).unwrap().is_none());
    }
}
