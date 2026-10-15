//! REST snapshots are distinct from continuous trades/quotes; OI has daily semantics.
use crate::{credentials::Credentials, history::checked_url, live::now_ms};
use serde::Serialize;
use serde_json::Value;
use snipers_core::options::{normalize, Observation};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

/// Reference and snapshot evidence from the same inventory REST pages.
#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeatmapData {
    pub reference: Vec<Value>, pub snapshots: Vec<Value>,
    pub reference_complete: bool, pub scope_start: String, pub scope_end: String,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotState {
    pub structural_enrichment:Option<crate::etf_scanner::Report>,
    pub phase: String,
    pub detail: String,
    pub observed_ms: Option<u64>,
    pub observed_date: Option<String>,
    pub previous_observed_date: Option<String>,
    pub contracts: usize,
    pub oi_missing: usize,
    pub greeks_missing: usize,
    pub expirations: Vec<String>,
    pub changes: Vec<snipers_core::options::OiChange>,
    pub storage_error: Option<String>,
    pub horizons: Vec<snipers_core::horizons::Horizon>,
    pub exposure_error: Option<String>,
    pub dex_changes: BTreeMap<String, snipers_core::dex_change::Change>,
    pub daily_range: Option<snipers_core::daily_range::Range>,
    pub daily_range_error: Option<String>,
    pub daily_range_source: String,
    pub structural_alignment: Option<snipers_core::reading::Alignment>,
}

fn refresh_due(snapshot: &SnapshotState, now: u64) -> bool {
    snapshot.phase != "ready"
        || snapshot.observed_ms.is_none_or(|t| {
            t > now
                || now - t >= 300_000
                || snipers_core::calendar::date_at(t) != snipers_core::calendar::date_at(now)
        })
}

pub async fn worker(key: &Credentials, state: crate::live::SharedState) {
    let notify = state.lock().unwrap().snapshot_notify.clone();
    let (restore_symbol, dex_directory) = {
        let s = state.lock().unwrap();
        (
            s.symbol.clone(),
            s.oi_directory.as_ref().map(|p| p.join("dex-baselines")),
        )
    };
    let restore_directory = dex_directory.clone();
    let restored = tokio::task::spawn_blocking(move || match restore_directory {
        Some(p) => crate::dex_store::restore(&p, &restore_symbol, now_ms()),
        None => Ok(snipers_core::dex_change::Tracker::default()),
    })
    .await;
    let (mut dex_tracker, dex_restore_error) = match restored {
        Ok(Ok(tracker)) => (tracker, None),
        Ok(Err(e)) => (
            snipers_core::dex_change::Tracker::default(),
            Some(e.to_owned()),
        ),
        Err(_) => (
            snipers_core::dex_change::Tracker::default(),
            Some("No se pudo recuperar la base DEX".into()),
        ),
    };
    let mut refresh = tokio::time::interval(Duration::from_secs(60));
    refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = notify.notified() => {},
            _ = refresh.tick() => {
                let s = state.lock().unwrap();
                if s.selection.spot.is_none() || !refresh_due(&s.option_snapshot_status, now_ms()) { continue; }
            }
        }
        let (symbol, spot, source, directory, price_at) = {
            let mut s = state.lock().unwrap();
            // A refresh must not hide a still-current successful observation.
            if s.option_snapshot_status.phase != "ready" {
                s.option_snapshot_status.phase = "loading".into();
            }
            s.option_snapshot_status.detail =
                "Consultando cadena y OI; el streaming continúa".into();
            let reference=s.ticks.get(&s.symbol).filter(|t|t.symbol==s.symbol&&t.price.is_some_and(|p|p.is_finite()&&p>0.)&&t.trade_ms.is_some_and(|t|t<=now_ms()&&now_ms()-t<=120_000));
            (
                s.symbol.clone(),
                reference.and_then(|t|t.price).or(s.selection.spot),
                if reference.is_some(){"Precio del activo · streaming".into()}else{s.selection.source.clone()},
                s.oi_directory.clone(),
                reference.and_then(|t|t.trade_ms),
            )
        };
        let result = match spot {
            Some(spot) => fetch(key, &symbol, spot, 4).await,
            None => Err("Referencia del subyacente aún no disponible"),
        };
        match result {
            Ok(observation) => {
                let disk_observation = observation.clone();
                let stored = tokio::task::spawn_blocking(move || {
                    let Some(directory) = directory else {
                        return (None, Some("Persistencia local no configurada".to_owned()));
                    };
                    let previous = crate::oi_store::previous(
                        &directory,
                        &disk_observation.symbol,
                        &disk_observation.observed_date,
                    );
                    let saved = crate::oi_store::save(&directory, &disk_observation);
                    match (previous, saved) {
                        (Ok(previous), Ok(_)) => (previous, None),
                        (Ok(previous), Err(error)) => (previous, Some(error.to_owned())),
                        (Err(error), _) => (None, Some(error.to_owned())),
                    }
                })
                .await
                .unwrap_or((
                    None,
                    Some("No se pudo completar el almacenamiento OI".into()),
                ));
                let history: Vec<_> = stored.0.into_iter().collect();
                let analytical_observation = observation.clone();
                let previous = history.first().cloned();
                let daily_range_source = source.clone();
                let analysis = tokio::task::spawn_blocking(move || {
                    let horizons = snipers_core::horizons::calculate_with_history(
                        &analytical_observation,
                        spot.unwrap(),
                        &source,
                        previous.as_ref(),
                    );
                    let range = snipers_core::daily_range::from_observation(
                        &analytical_observation,
                        spot.unwrap(),
                    );
                    let enrichment=crate::etf_scanner::structural_profile(&analytical_observation,spot.unwrap(),price_at).ok().map(|mut r|{
                        r.enrichment=Some(crate::etf_scanner::enrich(&analytical_observation,previous.as_ref(),&r,None));r
                    });
                    (horizons, range, enrichment)
                })
                .await
                .unwrap_or((
                    Err("No se pudo calcular exposición y estructura"),
                    Err("No se pudo calcular el rango diario"),
                    None,
                ));
                let (daily_range, daily_range_error) = match analysis.1 {
                    Ok(range) => (Some(range), None),
                    Err(error) => (None, Some(error.to_owned())),
                };
                let (horizons, exposure_error) = match analysis.0 {
                    Ok(h) => (h, None),
                    Err(e) => {
                        dex_tracker.clear();
                        (vec![], Some(e.to_owned()))
                    }
                };
                let analysis_now = now_ms();
                let structural_alignment = Some(snipers_core::reading::align([
                    structural_side(&horizons, &["DAY", "NEXT_SESSION"]),
                    structural_side(&horizons, &["WEEK"]),
                    structural_side(&horizons, &["NEXT4"]),
                ]));
                let dex_changes = horizons
                    .iter()
                    .map(|h| {
                        (
                            h.mode.clone(),
                            dex_tracker.observe(&observation, h, analysis_now),
                        )
                    })
                    .collect();
                let mut changes = snipers_core::options::changes(&observation, &history);
                changes.truncate(60);
                let dex_error = persist_dex(
                    &dex_tracker,
                    dex_directory.as_deref(),
                    &symbol,
                    analysis_now,
                )
                .await;
                let errors: Vec<_> = [stored.1, dex_restore_error.clone(), dex_error]
                    .into_iter()
                    .flatten()
                    .collect();
                let storage_error = (!errors.is_empty()).then(|| errors.join(" · "));
                let summary=SnapshotState {
                    structural_enrichment:analysis.2.map(|mut r|{if let Some(e)=&mut r.enrichment{e.storage_error=storage_error.clone();}r}),
                    phase:"ready".into(),detail:"Snapshot REST completo. OI del último día negociado; fecha específica no suministrada por el proveedor.".into(),
                    observed_ms:Some(observation.observed_ms),observed_date:Some(observation.observed_date.clone()),previous_observed_date:history.first().map(|h|h.observed_date.clone()),
                    contracts:observation.contracts.len(),oi_missing:observation.contracts.values().filter(|c|c.open_interest.is_none()).count(),greeks_missing:observation.contracts.values().filter(|c|c.delta.is_none()||c.gamma.is_none()).count(),
                    expirations:observation.contracts.values().map(|c|c.expiration.clone()).collect::<BTreeSet<_>>().into_iter().collect(),changes,storage_error,horizons,exposure_error,dex_changes,daily_range,daily_range_error,daily_range_source,structural_alignment,
                };
                let mut s = state.lock().unwrap();
                s.option_snapshot_status = summary;
                s.option_snapshot = Some(observation);
            }
            Err(detail) => {
                dex_tracker.clear();
                // Persist an empty checkpoint so a restart after a failed refresh
                // cannot silently resurrect the previously invalidated baseline.
                let dex_error =
                    persist_dex(&dex_tracker, dex_directory.as_deref(), &symbol, now_ms()).await;
                let mut s = state.lock().unwrap();
                if let Some(error) = dex_error {
                    s.option_snapshot_status.storage_error = Some(error);
                }
                s.option_snapshot_status.dex_changes.clear();
                s.option_snapshot_status.daily_range = None;
                s.option_snapshot_status.structural_alignment = None;
                s.option_snapshot_status.daily_range_error = Some(detail.into());
                s.option_snapshot_status.phase = "failed".into();
                s.option_snapshot_status.structural_enrichment = None;
                s.option_snapshot_status.detail = detail.into();
            }
        }
    }
}

async fn persist_dex(
    tracker: &snipers_core::dex_change::Tracker,
    directory: Option<&std::path::Path>,
    symbol: &str,
    now: u64,
) -> Option<String> {
    let directory = directory?.to_path_buf();
    let bytes = match tracker.checkpoint(symbol, now) {
        Ok(bytes) => bytes,
        Err(e) => return Some(e.into()),
    };
    let symbol = symbol.to_owned();
    match tokio::task::spawn_blocking(move || {
        crate::dex_store::save(&directory, &symbol, &bytes, now)
    })
    .await
    {
        Ok(Ok(_)) => None,
        Ok(Err(e)) => Some(e.into()),
        Err(_) => Some("No se pudo guardar la base DEX".into()),
    }
}

fn structural_side<'a>(horizons: &'a [snipers_core::horizons::Horizon], modes: &[&str]) -> &'a str {
    let mut found = horizons.iter().filter(|h| modes.contains(&h.mode.as_str()));
    let Some(h) = found.next() else {
        return "N/A";
    };
    if found.next().is_some() || !h.structure.reading.available {
        "N/A"
    } else {
        &h.structure.reading.side
    }
}

pub(crate) async fn page(
    client: &reqwest::Client,
    key: &Credentials,
    url: &reqwest::Url,
) -> Result<Value, &'static str> {
    for attempt in 0..3 {
        match client.get(url.clone()).bearer_auth(&key.0).send().await {
            Ok(response) if response.status().is_success() => {
                let page: Value = response
                    .json()
                    .await
                    .map_err(|_| "Snapshot REST inválido")?;
                if page["status"] != "OK" {
                    return Err("Massive no confirmó un snapshot válido");
                }
                return Ok(page);
            }
            Ok(response) if [401, 403].contains(&response.status().as_u16()) => {
                return Err("Acceso a snapshots rechazado (401/403)")
            }
            Ok(response)
                if response.status().as_u16() == 429 || response.status().is_server_error() =>
            {
                let delay = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1 << attempt)
                    .min(60);
                if attempt < 2 {
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
            }
            Ok(_) => return Err("Solicitud de snapshots rechazada"),
            Err(_) if attempt < 2 => tokio::time::sleep(Duration::from_secs(1 << attempt)).await,
            Err(_) => {}
        }
    }
    Err("No se pudo completar la consulta de snapshots")
}
pub(crate) fn url(path: &str, query: &[(&str, String)]) -> Result<reqwest::Url, &'static str> {
    let mut url = checked_url(&format!("https://api.massive.com{path}"), path)?;
    url.query_pairs_mut()
        .extend_pairs(query.iter().map(|(k, v)| (*k, v)));
    Ok(url)
}
/// Full pagination for nearest expirations plus this week's dates for horizon parity.
/// A failed page never publishes a partially assembled observation.
pub async fn fetch(
    key: &Credentials,
    symbol: &str,
    spot: f64,
    count: usize,
) -> Result<Observation, &'static str> {
    fetch_inner(key,symbol,spot,count,None).await
}
/// ETF scanner: all provider strikes and expirations within an explicit calendar window.
pub async fn fetch_etf_window(key:&Credentials,symbol:&str,spot:f64,days:u32)->Result<Observation,&'static str>{
    if !matches!(symbol,"SPY"|"QQQ") || !matches!(days,0|7|28){return Err("Universo ETF inválido");}
    fetch_inner(key,symbol,spot,4,Some(days)).await
}
/// Native scanner snapshots for configured stocks; preserve the ETF-only entry point.
pub async fn fetch_stock_window(key:&Credentials,symbol:&str,spot:f64,days:u32)->Result<Observation,&'static str>{
    if snipers_core::universe::stock_asset(symbol).is_none() || !matches!(days,0|7|28|30|183){return Err("Universo de opciones inválido");}
    fetch_inner(key,symbol,spot,4,Some(days)).await
}
/// 183 is the six-calendar-month inventory window, clamped at month end.
pub fn window_end(now:u64,days:u32)->Option<String>{
 let mut date=snipers_core::calendar::date_at(now)?;
 if days!=183{return (0..days).try_fold(date,|d,_|d.succ_opt()).map(|d|d.to_string());}
 let first=date.to_string();let year:i32=first[0..4].parse().ok()?;let month:u32=first[5..7].parse().ok()?;let day:u32=first[8..10].parse().ok()?;
 let total=year*12+month as i32-1+6;let target=format!("{:04}-{:02}",total/12,total%12+1);
 for _ in 0..190 {let next=date.succ_opt()?;let label=next.to_string();if label[..7]>target[..] {return Some(date.to_string());}date=next;if label[..7]==target[..]&&label[8..10].parse::<u32>().ok()?>=day{return Some(label);}}
 None
}
async fn fetch_inner(key:&Credentials,symbol:&str,spot:f64,count:usize,days:Option<u32>)->Result<Observation,&'static str>{
    fetch_inner_capture(key,symbol,spot,count,days,false).await.map(|(o,_)|o)
}
pub async fn fetch_stock_window_heatmap(key:&Credentials,symbol:&str,spot:f64)->Result<(Observation,Option<HeatmapData>),&'static str>{
    if snipers_core::universe::stock_asset(symbol).is_none(){return Err("Universo de opciones inválido");}
    fetch_inner_capture(key,symbol,spot,4,Some(183),true).await
}
async fn fetch_inner_capture(key:&Credentials,symbol:&str,spot:f64,count:usize,days:Option<u32>,capture:bool)->Result<(Observation,Option<HeatmapData>),&'static str>{
    let asset = snipers_core::universe::stock_asset(symbol)
        .or_else(|| {
            (symbol == "SPX")
                .then(|| snipers_core::universe::assets().get("SPX"))
                .flatten()
        })
        .ok_or("Activo no admitido para snapshot")?;
    if !spot.is_finite() || spot <= 0.0 || !(1..=4).contains(&count) {
        return Err("Rango de snapshot inválido");
    }
    let today = snipers_core::calendar::date_at(now_ms())
        .ok_or("Fecha ET no disponible")?
        .to_string();
    let friday = snipers_core::horizons::friday_at(now_ms()).ok_or("Fecha ET inválida")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Cliente REST no disponible")?;
    let reference = "/v3/reference/options/contracts";
    let range = asset.level_range;
    let end=days.map(|days|window_end(now_ms(),days).ok_or("Fecha fuera de rango")).transpose()?;
    let reference_pages=if days==Some(183){100}else{20};
    let max_contracts=if days==Some(183){100_000}else{20_000};
    let mut reference_params=vec![
        ("underlying_ticker",symbol.into()),("expiration_date.gte",today.clone()),
        ("sort","expiration_date".into()),("order","asc".into()),("limit","1000".into())
    ];
    if let Some(end)=&end{reference_params.push(("expiration_date.lte",end.clone()));}
    else{reference_params.extend([("strike_price.gte",(spot-range).to_string()),("strike_price.lte",(spot+range).to_string())]);}
    let mut next = url(
        reference,
        &reference_params,
    )?;
    let mut seen = BTreeSet::new();
    let mut expirations = BTreeSet::new();
    let mut multipliers = BTreeMap::new();
    let mut heatmap_reference = Vec::new();
    let mut heatmap_snapshots = BTreeMap::new();
    for index in 0..reference_pages {
        if !seen.insert(next.to_string()) {
            return Err("Cursor repetido en contratos");
        }
        let data = page(&client, key, &next).await?;
        let rows = data["results"]
            .as_array()
            .ok_or("Lista de contratos ausente")?;
        for row in rows {
            if capture {
                heatmap_reference.push(heatmap_reference_row(row));
                if heatmap_reference.len()>max_contracts{return Err("Referencia heatmap excede el límite de contratos");}
            }
            if symbol == "SPX"
                && row["ticker"]
                    .as_str()
                    .and_then(snipers_core::options::underlying)
                    != Some("SPXW")
            {
                continue;
            }
            let expiry = row["expiration_date"]
                .as_str()
                .ok_or("Contrato sin vencimiento")?;
            if expiry < today.as_str() || end.as_ref().is_some_and(|end|expiry>end.as_str()) {
                return Err("Contrato vencido inesperado");
            }
            expirations.insert(expiry.to_owned());
            if let (Some(ticker), Some(multiplier)) = (
                row["ticker"].as_str(),
                snipers_core::options::verified_multiplier(row, symbol),
            ) {
                multipliers.insert(ticker.to_owned(), multiplier);
            }
        }
        // Read beyond the last selected date so metadata for it is complete too.
        if days.is_none() && expirations.len() > count && expirations.last().is_some_and(|e| e > &friday) {
            break;
        }
        match data["next_url"].as_str() {
            Some(value) if index+1 < reference_pages => next = checked_url(value, reference)?,
            Some(_) => return Err("Límite de páginas de contratos alcanzado"),
            None => break,
        }
    }
    if expirations.is_empty() && days.is_none() {
        return Err("No se encontraron vencimientos en el rango");
    }
    let path = format!("/v3/snapshot/options/{symbol}");
    let mut contracts = BTreeMap::new();
    let selected: BTreeSet<_> = if days.is_some(){expirations.clone()}else{expirations
        .iter()
        .take(count)
        .chain(expirations.iter().filter(|e| *e <= &friday))
        .cloned()
        .collect()};
    for expiration in selected {
        let mut params=vec![("expiration_date",expiration.clone()),("limit","250".into()),("sort","strike_price".into()),("order","asc".into())];
        if days.is_none(){params.extend([("strike_price.gte",(spot-range).to_string()),("strike_price.lte",(spot+range).to_string())]);}
        let mut next = url(
            &path,
            &params,
        )?;
        let mut seen = BTreeSet::new();
        for index in 0..40 {
            if !seen.insert(next.to_string()) {
                return Err("Cursor repetido en snapshots");
            }
            let data = page(&client, key, &next).await?;
            for row in data["results"]
                .as_array()
                .ok_or("Lista de snapshots ausente")?
            {
                let mut contract = normalize(row, symbol)?;
                if capture {
                    let raw=heatmap_snapshot_row(row);
                    if heatmap_snapshots.get(&contract.ticker).is_some_and(|old|old!=&raw){return Err("Identidad heatmap cambió entre páginas; repetir snapshot");}
                    heatmap_snapshots.insert(contract.ticker.clone(),raw);
                }
                contract.premium_multiplier = multipliers.get(&contract.ticker).copied();
                if contract.expiration != expiration
                    || days.is_none() && (contract.strike < spot - range || contract.strike > spot + range)
                {
                    return Err("Snapshot fuera del rango solicitado");
                }
                if let Some(old) = contracts.insert(contract.ticker.clone(), contract.clone()) {
                    if serde_json::to_value(old).ok() != serde_json::to_value(contract).ok() {
                        return Err("Contrato cambió entre páginas; repetir snapshot");
                    }
                }
                if contracts.len() > max_contracts {
                    return Err("Snapshot excede el límite de contratos");
                }
            }
            match data["next_url"].as_str() {
                Some(value) if index < 39 => next = checked_url(value, &path)?,
                Some(_) => return Err("Límite de páginas de snapshots alcanzado"),
                None => break,
            }
        }
    }
    if contracts.is_empty() && days.is_none() {
        return Err("La cadena de opciones está vacía");
    }
    let observed_ms = now_ms();
    let observed_date = snipers_core::calendar::date_at(observed_ms)
        .ok_or("Fecha ET no disponible")?
        .to_string();
    if observed_date != today {
        return Err("La fecha ET cambió durante la consulta; repetir snapshot");
    }
    let evidence=if capture {Some(HeatmapData{reference:heatmap_reference,snapshots:heatmap_snapshots.into_values().collect(),reference_complete:true,scope_start:today,scope_end:end.ok_or("Rango heatmap ausente")?})}else{None};
    Ok((Observation {
        schema: 1,
        symbol: symbol.into(),
        observed_ms,
        observed_date,
        oi_reference_date: None,
        contracts,
    },evidence))
}

fn heatmap_reference_row(row:&Value)->Value {
    let mut out=serde_json::Map::new();
    for key in ["ticker","underlying_ticker","expiration_date","strike_price","contract_type","shares_per_contract","additional_underlyings"] {
        if let Some(value)=row.get(key){out.insert(key.into(),value.clone());}
    }
    Value::Object(out)
}
fn heatmap_snapshot_row(row:&Value)->Value {
    let mut out=serde_json::Map::new();
    if let Some(details)=row.get("details"){out.insert("details".into(),heatmap_reference_row(details));}
    if let Some(value)=row.get("open_interest"){out.insert("open_interest".into(),value.clone());}
    if let Some(value)=row.get("underlying_asset"){
        let mut asset=value.clone();
        // Preserve integer nanoseconds across JSON -> JavaScript float64 boundary.
        if let Some(ns)=asset["last_updated"].as_u64(){asset["last_updated"]=Value::String(ns.to_string());}
        out.insert("underlying_asset".into(),asset);
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn heatmap_projection_preserves_deliverable_unknowns_and_excludes_unrelated_fields(){
        let raw=serde_json::json!({"ticker":"O:QQQ261016C00750000","shares_per_contract":100,"additional_underlyings":null,"sensitive_unrelated":"excluded"});
        let r=heatmap_reference_row(&raw);
        assert!(r.get("additional_underlyings").is_some_and(Value::is_null));
        assert!(r.get("sensitive_unrelated").is_none());
        let missing=heatmap_reference_row(&serde_json::json!({"ticker":"x"}));
        assert!(missing.get("shares_per_contract").is_none());
        assert!(missing.get("additional_underlyings").is_none());
    }
    #[test]
    fn heatmap_snapshot_projection_retains_exact_daily_zero_and_underlying_identity(){
        let raw=serde_json::json!({"details":{"ticker":"x","additional_underlyings":[{"underlying":"OTHER"}]},"open_interest":0,"underlying_asset":{"ticker":"QQQ","last_updated":1791904450050584243u64},"greeks":{"delta":0.5}});
        let r=heatmap_snapshot_row(&raw);
        assert_eq!(r["open_interest"],0);
        assert_eq!(r["details"]["additional_underlyings"].as_array().unwrap().len(),1);
        assert_eq!(r["underlying_asset"]["last_updated"].as_str(),Some("1791904450050584243"));
        assert!(r.get("greeks").is_none());
    }
    #[test]
    fn refreshes_before_expiry_and_retries_failures_without_expiration_change() {
        let now = now_ms();
        let mut s = SnapshotState {
            phase: "ready".into(),
            observed_ms: Some(now),
            ..Default::default()
        };
        assert!(!refresh_due(&s, now + 59_000));
        assert!(refresh_due(&s, now + 300_000));
        s.phase = "failed".into();
        assert!(refresh_due(&s, now + 60_000));
        s.phase = "ready".into();
        s.observed_ms = Some(now + 1);
        assert!(refresh_due(&s, now));
    }
    #[test]
    fn snapshot_pagination_is_confined_to_same_symbol_and_host() {
        let path = "/v3/snapshot/options/QQQ";
        assert!(checked_url(
            "https://api.massive.com/v3/snapshot/options/SPY?cursor=x",
            path
        )
        .is_err());
        assert!(checked_url("https://example.com/v3/snapshot/options/QQQ", path).is_err());
        let next = checked_url(
            "https://api.massive.com/v3/snapshot/options/QQQ?cursor=x&apiKey=secret",
            path,
        )
        .unwrap();
        assert!(!next.as_str().contains("secret"));
    }
    #[test]
    fn six_month_inventory_window_is_calendar_based_and_clamps_month_end(){
      for (from,to) in [("2026-10-08","2027-04-08"),("2026-08-31","2027-02-28"),("2027-08-31","2028-02-29"),("2026-12-31","2027-06-30")] {
        let now=snipers_core::calendar::legacy_eastern_ms(&format!("{from} 10:00:00")).unwrap();
        assert_eq!(window_end(now,183).as_deref(),Some(to));
      }
    }

}
