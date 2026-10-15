//! Explicit one-shot prospective REST observer. No scheduler, live state, scores or orders.
use crate::{
    credentials::Credentials, history, live::now_ms, option_activity_dataset as dataset,
    option_activity_history, option_activity_history::Request, option_premium_dataset,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snipers_core::{
    calendar::{self, Day, Session},
    option_activity_baseline::{Baseline, Sample},
    option_activity_detector, option_premium_detector,
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

const STEP: u64 = 300_000;
const MAX_LAG: u64 = 120_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrozenSession {
    pub version: String,
    pub underlying: String,
    pub date: String,
    #[serde(deserialize_with = "read_session")]
    pub session: Session,
    pub frozen_ms: u64,
    pub eligible_from_ms: u64,
    pub opening_reference_price: f64,
    pub selection_evidence: Value,
    pub stock_evidence: Value,
    pub baseline: Baseline,
    pub execution_enabled: bool,
    #[serde(default)]
    pub premium_mode: bool,
}
fn read_session<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Session, D::Error> {
    let value = Value::deserialize(d)?;
    let open = value["openMs"]
        .as_u64()
        .ok_or_else(|| serde::de::Error::custom("Sesión sin apertura"))?;
    let session = match calendar::date_at(open).map(calendar::schedule) {
        Some(Day::Open(s)) => s,
        _ => return Err(serde::de::Error::custom("Calendario de sesión inválido")),
    };
    if serde_json::to_value(&session).map_err(serde::de::Error::custom)? != value {
        return Err(serde::de::Error::custom(
            "Sesión no coincide con calendario",
        ));
    }
    Ok(session)
}
fn session_at(now: u64) -> Result<Session, String> {
    match calendar::date_at(now).map(calendar::schedule) {
        Some(Day::Open(s)) if now >= s.open_ms && now < s.close_ms => Ok(s),
        _ => Err("WAITING_SESSION: no hay sesión regular abierta".into()),
    }
}
fn eligible_from(session: &Session, frozen_ms: u64) -> Result<u64, String> {
    if frozen_ms < session.open_ms + 60_000 || frozen_ms >= session.close_ms {
        return Err("WAITING_SELECTION: falta primer minuto cerrado o terminó la sesión".into());
    }
    let start = session.open_ms + ((frozen_ms - session.open_ms) / STEP + 1) * STEP;
    if start + STEP > session.close_ms {
        return Err("WAITING_NEXT_SESSION: no queda ventana completa".into());
    }
    Ok(start)
}
pub(crate) fn validate_reference(
    baseline: &Baseline,
    session: &Session,
    now: u64,
) -> Result<(), String> {
    if baseline.version != "option-activity-baseline-v1"
        || baseline.execution_enabled
        || !baseline.available_before_cutoff
        || baseline.built_ms >= baseline.cutoff_ms
        || baseline.cutoff_ms != session.open_ms
        || baseline.built_ms >= now
        || !(20..=252).contains(&baseline.minimum_sessions)
        || baseline.cohorts.is_empty()
        || baseline.cohorts.iter().any(|c| {
            c.dates
                .iter()
                .any(|d| d >= &session.date || calendar::day_bounds(d).is_none())
        })
    {
        return Err(
            "PENDING_REFERENCE: se requiere referencia previa con corte en apertura de esta sesión"
                .into(),
        );
    }
    Ok(())
}
pub(crate) fn closed_window(frozen: &FrozenSession, now: u64) -> Result<(u64, u64), String> {
    if frozen.version != "option-activity-frozen-session-v1"
        || frozen.execution_enabled
        || frozen.date != frozen.session.date
        || frozen.frozen_ms >= frozen.eligible_from_ms
        || eligible_from(&frozen.session, frozen.frozen_ms)? != frozen.eligible_from_ms
        || calendar::date_at(now).map(|d| d.to_string()).as_deref() != Some(&frozen.date)
        || now < frozen.session.open_ms
        || now > frozen.session.close_ms + MAX_LAG
    {
        return Err("INVALID_FROZEN_SESSION: sesión incompatible con reloj o configuración".into());
    }
    let end = frozen.session.open_ms + ((now - frozen.session.open_ms) / STEP) * STEP;
    let end = end.min(frozen.session.close_ms);
    let start = end.checked_sub(STEP).ok_or("WAITING_CLOSED_WINDOW")?;
    if start < frozen.eligible_from_ms || now - end > MAX_LAG {
        return Err("WAITING_CLOSED_WINDOW: falta ventana elegible recién cerrada".into());
    }
    Ok((start, end))
}
fn write_new<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "Archivo existente o destino no disponible".to_owned())?;
    serde_json::to_writer(&mut file, value).map_err(|_| "No se pudo serializar observación")?;
    file.write_all(b"\n").map_err(|_| "Escritura incompleta")?;
    file.sync_all()
        .map_err(|_| "Escritura no confirmada".into())
}
fn arm_start(frozen: &FrozenSession, now: u64) -> Result<u64, String> {
    let session = session_at(now)?;
    if session.date != frozen.date {
        return Err("Sesión de contexto distinta".into());
    }
    let start = eligible_from(&session, now)?;
    if start < frozen.eligible_from_ms || start - now > 60_000 {
        return Err("WAITING_CONTEXT: armar en el último minuto antes de la ventana".into());
    }
    Ok(start)
}
pub(crate) fn context_price(
    context: &Value,
    start: u64,
    date: &str,
    symbol: &str,
) -> Result<(f64, u64), String> {
    let known = context["knownMs"]
        .as_u64()
        .ok_or("PENDING_CONTEXT: falta captura previa")?;
    let available = context["priceAvailableMs"]
        .as_u64()
        .ok_or("Contexto sin reloj")?;
    let price = context["price"].as_f64().ok_or("Contexto sin precio")?;
    let bar = context["bars"]
        .as_array()
        .filter(|b| b.len() == 1)
        .and_then(|b| b.first())
        .ok_or("PENDING_CONTEXT: falta evidencia del minuto recibido")?;
    if bar["timestampMs"]
        .as_u64()
        .and_then(|t| t.checked_add(60_000))
        != Some(available)
        || bar["close"].as_f64() != Some(price)
    {
        return Err("PENDING_CONTEXT: precio no coincide con evidencia".into());
    }
    if context["date"] != date
        || context["underlying"] != symbol
        || context["startMs"] != start
        || context["adjusted"] != false
        || known >= start
        || available > known
        || available > start
        || start - available > 60_000
        || !price.is_finite()
        || price <= 0.
    {
        return Err("PENDING_CONTEXT: contexto futuro, atrasado o incompatible".into());
    }
    Ok((price, available))
}
/// Persist a received, closed stock bar during the last minute before a window.
pub async fn arm(directory: &Path) -> Result<Value, String> {
    let frozen: FrozenSession = serde_json::from_slice(
        &fs::read(directory.join("frozen-session.json")).map_err(|_| "Falta sesión congelada")?,
    )
    .map_err(|_| "Sesión congelada inválida")?;
    let started = now_ms();
    let start = arm_start(&frozen, started)?;
    let path = directory.join(format!("context-{start}.json"));
    if path.exists() {
        return Err("Contexto ya congelado".into());
    }
    let through = (started / 60_000) * 60_000;
    let key = Credentials::load()?;
    let bars = history::fetch_unadjusted(&key, &frozen.underlying, through - 60_000, through - 1)
        .await
        .map_err(str::to_owned)?;
    let bar = bars
        .iter()
        .find(|b| b.timestamp_ms + 60_000 == through)
        .ok_or("PENDING_CONTEXT: minuto aún no recibido")?;
    let known = now_ms();
    let context = serde_json::json!({"version":"option-activity-prior-context-v1",
        "date":frozen.date,"underlying":frozen.underlying,"startMs":start,"knownMs":known,
        "price":bar.close,"priceAvailableMs":through,"adjusted":false,"bars":bars});
    context_price(&context, start, &frozen.date, &frozen.underlying)?;
    write_new(&path, &context)?;
    Ok(
        serde_json::json!({"status":"CONTEXT_FROZEN","startMs":start,"knownMs":known,
        "executionEnabled":false,"ordersSent":0}),
    )
}
/// Requires a current open session; never reconstructs a selection for elapsed windows.
pub async fn prepare(
    baseline_path: &Path,
    symbol: &str,
    directory: &Path,
) -> Result<Value, String> {
    prepare_mode(baseline_path, symbol, directory, false).await
}
pub async fn prepare_premium(
    baseline_path: &Path,
    symbol: &str,
    directory: &Path,
) -> Result<Value, String> {
    prepare_mode(baseline_path, symbol, directory, true).await
}
async fn prepare_mode(
    baseline_path: &Path,
    symbol: &str,
    directory: &Path,
    premium_mode: bool,
) -> Result<Value, String> {
    let started = now_ms();
    let session = session_at(started)?;
    eligible_from(&session, started)?;
    let baseline: Baseline =
        serde_json::from_slice(&fs::read(baseline_path).map_err(|_| "Referencia no disponible")?)
            .map_err(|_| "Referencia no válida")?;
    validate_reference(&baseline, &session, started)?;
    if premium_mode
        && !baseline.cohorts.iter().any(|c| {
            c.key.underlying == symbol
                && c.key.measurement == "reported-trades"
                && c.premium.is_some()
        })
    {
        return Err(
            "PENDING_REFERENCE: falta referencia de primas de operaciones reportadas".into(),
        );
    }
    if snipers_core::universe::stock_asset(symbol).is_none()
        || !baseline.cohorts.iter().any(|c| c.key.underlying == symbol)
    {
        return Err("Símbolo sin referencia".into());
    }
    if directory.join("frozen-session.json").exists() {
        return Err("Sesión ya congelada; no se reemplaza".into());
    }
    let key = Credentials::load()?;
    let bars = history::fetch_unadjusted(&key, symbol, session.open_ms, session.open_ms + 59_999)
        .await
        .map_err(str::to_owned)?;
    let opening = bars
        .iter()
        .find(|b| b.timestamp_ms == session.open_ms)
        .ok_or("Falta primer minuto cerrado")?;
    let (_, selection_evidence) =
        dataset::discover(&key, symbol, &session.date, opening.close, 3).await?;
    let frozen_ms = now_ms();
    let eligible_from_ms = eligible_from(&session, frozen_ms)?;
    if session_at(frozen_ms)?.date != session.date {
        return Err("Cambió la sesión durante selección".into());
    }
    let frozen = FrozenSession {
        version: "option-activity-frozen-session-v1".into(),
        underlying: symbol.into(),
        date: session.date.clone(),
        session,
        frozen_ms,
        eligible_from_ms,
        opening_reference_price: opening.close,
        selection_evidence,
        stock_evidence: serde_json::json!({"adjusted":false,"bars":bars,"retrievedMs":frozen_ms}),
        baseline,
        execution_enabled: false,
        premium_mode,
    };
    fs::create_dir_all(directory).map_err(|_| "Destino no disponible")?;
    write_new(&directory.join("frozen-session.json"), &frozen)?;
    Ok(
        serde_json::json!({"status":"FROZEN_OBSERVATION_SESSION","date":frozen.date,
        "eligibleFromMs":eligible_from_ms,"contracts":6,"premiumMode":premium_mode,"executionEnabled":false,"ordersSent":0}),
    )
}
/// Capture only the latest closed five-minute window within two minutes of closure.
/// No catch-up pretending that old windows were observed prospectively.
pub async fn capture(directory: &Path) -> Result<Value, String> {
    let frozen: FrozenSession = serde_json::from_slice(
        &fs::read(directory.join("frozen-session.json")).map_err(|_| "Falta sesión congelada")?,
    )
    .map_err(|_| "Sesión congelada no válida")?;
    if frozen.premium_mode {
        return Err("Usar capture-premium para sesión de primas".into());
    }
    let started = now_ms();
    let (start, end) = closed_window(&frozen, started)?;
    validate_reference(&frozen.baseline, &frozen.session, started)?;
    let path = directory.join(format!("window-{start}.json"));
    if path.exists() {
        return Err("ALREADY_CAPTURED: ventana ya registrada".into());
    }
    let selected = frozen.selection_evidence["selected"]
        .as_array()
        .ok_or("Selección ausente")?;
    let catalog = frozen.selection_evidence["catalogRows"]
        .as_array()
        .ok_or("Catálogo ausente")?;
    let expected = dataset::select_contracts_breadth(
        catalog,
        &frozen.underlying,
        &frozen.date,
        frozen.opening_reference_price,
        3,
    )?;
    let actual: Vec<_> = selected.iter().filter_map(Value::as_str).collect();
    if selected.len() != 6
        || frozen.selection_evidence["complete"] != true
        || actual != expected.iter().map(String::as_str).collect::<Vec<_>>()
    {
        return Err("Selección incompleta".into());
    }
    let context = fs::read(directory.join(format!("context-{start}.json")))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .unwrap_or(Value::Null);
    let price_context = context_price(&context, start, &frozen.date, &frozen.underlying);
    let key = if price_context.is_ok() {
        Some(Credentials::load()?)
    } else {
        None
    };
    let mut observations = vec![];
    for ticker in selected {
        let ticker = ticker.as_str().ok_or("Ticker no válido")?;
        if let Err(error) = &price_context {
            observations.push(
                serde_json::json!({"ticker":ticker,"status":"PENDING_CONTEXT",
                "reason":error,"completedMs":now_ms(),"alertEligible":false}),
            );
            continue;
        }
        let request = Request {
            underlying: frozen.underlying.clone(),
            ticker: ticker.into(),
            date: frozen.date.clone(),
            window_minutes: 5,
            max_pages: 4,
        };
        match dataset::collect_aggregate_volume_until(
            key.as_ref().ok_or("Falta credencial")?,
            &request,
            &frozen.session,
            &[],
            frozen.eligible_from_ms,
            end,
        )
        .await
        {
            Ok((_, _, evidence)) => {
                let bars = evidence["bars"]
                    .as_array()
                    .ok_or("Evidencia de opciones ausente")?;
                let volumes: Option<Vec<_>> = (start..end)
                    .step_by(60_000)
                    .map(|t| {
                        bars.iter()
                            .find(|b| b["timestampMs"].as_u64() == Some(t))
                            .and_then(|b| b["volume"].as_f64())
                    })
                    .collect();
                let (price, available) = *price_context.as_ref().map_err(|e| e.clone())?;
                let sample = volumes.map(|v| Sample {
                    underlying: frozen.underlying.clone(),
                    ticker: ticker.into(),
                    kind: evidence["contract"]["kind"].as_str().unwrap_or("").into(),
                    expiration: evidence["contract"]["expiration"]
                        .as_str()
                        .unwrap_or("")
                        .into(),
                    strike: evidence["contract"]["strike"].as_f64().unwrap_or(f64::NAN),
                    start_ms: start,
                    end_ms: end,
                    underlying_price: price,
                    price_available_ms: available,
                    contracts: v.iter().sum(),
                    premium: None,
                    measurement: "eligible-option-bars".into(),
                    source_collected_ms: evidence["collectedMs"].as_u64().unwrap_or(0),
                    quality_complete: true,
                });
                let completed = now_ms();
                let reading = sample
                    .as_ref()
                    .map(|s| option_activity_detector::evaluate(&frozen.baseline, s, completed));
                let timely = completed <= end + MAX_LAG;
                observations.push(serde_json::json!({"ticker":ticker,"sample":sample,"evidence":evidence,
                    "reading":reading,"status":if !timely {"PENDING_CAPTURE_LATE"} else {
                        reading.as_ref().map(|r|r.status.as_str()).unwrap_or("PENDING_MISSING_MINUTE")},
                    "completedMs":completed,"timelyCapture":timely,
                    "alertEligible":timely && reading.as_ref().is_some_and(|r|r.status=="UNUSUAL_VOLUME_EXPERIMENTAL")}));
            }
            Err(error) => observations.push(
                serde_json::json!({"ticker":ticker,"status":"PENDING_SOURCE",
                "reason":error,"completedMs":now_ms(),"alertEligible":false}),
            ),
        }
    }
    let output = serde_json::json!({"version":"option-activity-prospective-window-v1",
        "date":frozen.date,"underlying":frozen.underlying,"startMs":start,"endMs":end,
        "captureStartedMs":started,"captureCompletedMs":now_ms(),"frozenMs":frozen.frozen_ms,
        "referenceBuiltMs":frozen.baseline.built_ms,"stockEvidence":context,
        "observations":observations,"calibrated":false,"executionEnabled":false,"ordersSent":0});
    write_new(&path, &output)?;
    Ok(serde_json::json!({"status":"OBSERVATION_SAVED","file":path,
        "contracts":6,"startMs":start,"endMs":end,"executionEnabled":false,"ordersSent":0}))
}

pub(crate) fn selected_tickers(frozen: &FrozenSession) -> Result<Vec<String>, String> {
    let rows = frozen.selection_evidence["catalogRows"]
        .as_array()
        .ok_or("Catálogo ausente")?;
    let expected = dataset::select_contracts_breadth(
        rows,
        &frozen.underlying,
        &frozen.date,
        frozen.opening_reference_price,
        3,
    )?;
    if frozen.selection_evidence["complete"] != true
        || frozen.selection_evidence["selected"] != serde_json::json!(expected)
    {
        return Err("Selección congelada inconsistente".into());
    }
    Ok(expected)
}
pub(crate) fn premium_observation(
    frozen: &FrozenSession,
    context: &Value,
    evidence: &option_activity_history::WindowReport,
    completed: u64,
) -> Result<Value, String> {
    let start = evidence.start_ms;
    let end = evidence.end_ms;
    let (underlying_price, price_available_ms) =
        context_price(context, start, &frozen.date, &frozen.underlying)?;
    if !frozen.premium_mode
        || frozen.frozen_ms >= start
        || start < frozen.eligible_from_ms
        || evidence.report.request.underlying != frozen.underlying
        || evidence.report.request.date != frozen.date
    {
        return Err("Evidencia ajena a la sesión congelada".into());
    }
    let quality = option_premium_dataset::certify_window(evidence);
    let sample = quality.certified_premium.map(|premium| Sample {
        underlying: frozen.underlying.clone(),
        ticker: evidence.report.request.ticker.clone(),
        kind: evidence.report.contract.kind.clone(),
        expiration: evidence.report.contract.expiration.clone(),
        strike: evidence.report.contract.strike,
        start_ms: start,
        end_ms: end,
        underlying_price,
        price_available_ms,
        contracts: evidence.report.windows[0].contracts.unwrap_or(0.),
        premium: Some(premium),
        measurement: "reported-trades".into(),
        source_collected_ms: evidence.report.collected_ms,
        quality_complete: true,
    });
    let timely = completed <= end + MAX_LAG && completed >= evidence.report.collected_ms;
    let reading = sample
        .as_ref()
        .map(|s| option_premium_detector::evaluate(&frozen.baseline, s, completed));
    let status = if !timely {
        "PENDING_CAPTURE_LATE"
    } else {
        reading
            .as_ref()
            .map(|r| r.status.as_str())
            .unwrap_or("PENDING_PREMIUM_DATA")
    };
    Ok(
        serde_json::json!({"ticker":evidence.report.request.ticker,"status":status,
        "quality":quality,"evidence":evidence,"sample":sample,"reading":reading,
        "completedMs":completed,"timelyCapture":timely,
        "alertEligible":timely && reading.as_ref().is_some_and(|r|r.status=="UNUSUAL_PREMIUM_EXPERIMENTAL")}),
    )
}

/// One bounded page chain per contract, raw trades and quality saved beside the reading.
pub async fn capture_premium(directory: &Path) -> Result<Value, String> {
    let frozen: FrozenSession = serde_json::from_slice(
        &fs::read(directory.join("frozen-session.json")).map_err(|_| "Falta sesión congelada")?,
    )
    .map_err(|_| "Sesión congelada inválida")?;
    if !frozen.premium_mode {
        return Err("Usar prepare-premium antes de capturar primas".into());
    }
    let started = now_ms();
    let (start, end) = closed_window(&frozen, started)?;
    validate_reference(&frozen.baseline, &frozen.session, started)?;
    let tickers = selected_tickers(&frozen)?;
    let path = directory.join(format!("premium-window-{start}.json"));
    if path.exists() {
        return Err("ALREADY_CAPTURED: primas ya registradas".into());
    }
    let context = fs::read(directory.join(format!("context-{start}.json")))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .unwrap_or(Value::Null);
    let price = context_price(&context, start, &frozen.date, &frozen.underlying);
    let key = if price.is_ok() {
        Some(Credentials::load()?)
    } else {
        None
    };
    let mut observations = vec![];
    for ticker in tickers {
        if let Err(reason) = &price {
            observations.push(
                serde_json::json!({"ticker":ticker,"status":"PENDING_CONTEXT",
                "reason":reason,"alertEligible":false}),
            );
            continue;
        }
        // Do not dispatch additional requests after the freshness deadline.
        if now_ms() > end + MAX_LAG {
            observations.push(
                serde_json::json!({"ticker":ticker,"status":"PENDING_CAPTURE_LATE",
                "alertEligible":false}),
            );
            continue;
        }
        let request = Request {
            underlying: frozen.underlying.clone(),
            ticker: ticker.clone(),
            date: frozen.date.clone(),
            window_minutes: 5,
            max_pages: 20,
        };
        match option_activity_history::collect_window(
            key.as_ref().ok_or("Falta credencial")?,
            request,
            start,
            end,
        )
        .await
        {
            Ok(evidence) => {
                let completed = now_ms();
                observations.push(premium_observation(
                    &frozen, &context, &evidence, completed,
                )?);
            }
            Err(reason) => observations.push(
                serde_json::json!({"ticker":ticker,"status":"PENDING_SOURCE",
                "reason":reason,"completedMs":now_ms(),"alertEligible":false}),
            ),
        }
    }
    let output = serde_json::json!({"version":"option-premium-prospective-window-v1","date":frozen.date,
        "underlying":frozen.underlying,"startMs":start,"endMs":end,"captureStartedMs":started,
        "captureCompletedMs":now_ms(),"frozenMs":frozen.frozen_ms,"referenceBuiltMs":frozen.baseline.built_ms,
        "stockEvidence":context,"observations":observations,"calibrated":false,"executionEnabled":false,"ordersSent":0});
    write_new(&path, &output)?;
    Ok(
        serde_json::json!({"status":"PREMIUM_OBSERVATION_SAVED","file":path,"contracts":6,
        "startMs":start,"endMs":end,"executionEnabled":false,"ordersSent":0}),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn premium_fixture() -> (FrozenSession, Value, option_activity_history::WindowReport)
    {
        let mut f = frozen();
        f.premium_mode = true;
        let start = f.eligible_from_ms;
        let end = start + STEP;
        let context = serde_json::json!({"date":f.date,"underlying":"QQQ","startMs":start,
            "knownMs":start-20_000,"priceAvailableMs":start-60_000,"price":600.,"adjusted":false,
            "bars":[{"timestampMs":start-120_000,"close":600.}]});
        let evidence=serde_json::from_value(serde_json::json!({
            "version":"option-premium-window-evidence-v1","startMs":start,"endMs":end,"captureStartedMs":end+1000,
            "report":{"version":"option-activity-window-v1","collectedMs":end+2000,
                "request":{"underlying":"QQQ","ticker":"O:QQQ261012C00600000","date":f.date,"windowMinutes":5,"maxPages":20},
                "contract":{"ticker":"O:QQQ261012C00600000","kind":"call","expiration":f.date,"strike":600,"premiumMultiplier":100},
                "session":f.session,"windowClock":"sip_timestamp",
                "scope":"reported-prints-single-requested-contract-equity-RTH-window",
                "pages":1,"rowsReceived":1,"duplicates":0,"rejected":0,"corrected":0,"conflicts":0,"unidentified":0,
                "retrievalComplete":true,"aggregationComplete":true,"warnings":[],
                "windows":[{"startMs":start,"endMs":end,"trades":1,"observedContracts":1,"observedPremium":5000,
                    "contracts":1,"premium":5000,"participantClockTrades":0,"sipOnlyTrades":1}],
                "trades":[{"identity":"seq:1","identityVerified":true,"sipNs":start*1000000,
                    "participantNs":null,"price":50,"contracts":1,"premium":5000,"conditions":[209]}]}
        })).unwrap();
        (f, context, evidence)
    }
    #[test]
    fn certified_window_can_be_saved_while_reference_remains_pending() {
        let (f, c, e) = premium_fixture();
        let v = premium_observation(&f, &c, &e, e.end_ms + 3000).unwrap();
        assert_eq!(v["quality"]["status"], "CERTIFIED_REPORTED_PREMIUM");
        assert_eq!(v["sample"]["premium"], 5000.);
        assert_eq!(v["status"], "PENDING");
        assert_eq!(v["alertEligible"], false);
    }
    #[test]
    fn partial_and_late_capture_never_enable_premium_alerts() {
        let (f, c, mut e) = premium_fixture();
        e.report.retrieval_complete = false;
        let v = premium_observation(&f, &c, &e, e.end_ms + 3000).unwrap();
        assert!(v["sample"].is_null());
        assert_eq!(v["status"], "PENDING_PREMIUM_DATA");
        let (f, c, e) = premium_fixture();
        let v = premium_observation(&f, &c, &e, e.end_ms + MAX_LAG + 1).unwrap();
        assert_eq!(v["status"], "PENDING_CAPTURE_LATE");
        assert_eq!(v["alertEligible"], false);
    }
    fn session() -> Session {
        session_at(calendar::legacy_eastern_ms("2026-10-12 09:31:00").unwrap()).unwrap()
    }
    fn frozen() -> FrozenSession {
        let s = session();
        let now = s.open_ms + 60_000;
        FrozenSession {
            version: "option-activity-frozen-session-v1".into(),
            underlying: "QQQ".into(),
            date: s.date.clone(),
            session: s.clone(),
            frozen_ms: now,
            eligible_from_ms: s.open_ms + STEP,
            opening_reference_price: 600.,
            selection_evidence: Value::Null,
            stock_evidence: Value::Null,
            baseline: serde_json::from_value(
                serde_json::json!({"version":"option-activity-baseline-v1",
                "builtMs":now-86_400_000,"cutoffMs":s.open_ms,"availableBeforeCutoff":true,
                "minimumSessions":20,"policy":"experimental","status":"EXPERIMENTAL_REFERENCE",
                "calibrated":false,"executionEnabled":false,"acceptedSamples":0,"duplicates":0,
                "conflictingSamples":0,"excluded":{},"cohorts":[]}),
            )
            .unwrap(),
            execution_enabled: false,
            premium_mode: false,
        }
    }
    #[test]
    fn serialized_calendar_cannot_be_tampered() {
        let f = frozen();
        let mut value = serde_json::to_value(&f).unwrap();
        assert!(serde_json::from_value::<FrozenSession>(value.clone()).is_ok());
        value["session"]["closeMs"] = serde_json::json!(f.session.close_ms + STEP);
        assert!(serde_json::from_value::<FrozenSession>(value).is_err());
    }
    #[test]
    fn context_must_be_received_before_window_without_stale_price() {
        let f = frozen();
        let start = f.eligible_from_ms;
        let mut c = serde_json::json!({"date":f.date,"underlying":"QQQ","startMs":start,
            "knownMs":start-30_000,"priceAvailableMs":start-60_000,"price":600.,"adjusted":false,
            "bars":[{"timestampMs":start-120_000,"close":600.}]});
        assert!(context_price(&c, start, &f.date, "QQQ").is_ok());
        c["knownMs"] = serde_json::json!(start + 1);
        assert!(context_price(&c, start, &f.date, "QQQ").is_err());
        c["knownMs"] = serde_json::json!(start - 30_000);
        c["priceAvailableMs"] = serde_json::json!(start - 120_000);
        assert!(context_price(&c, start, &f.date, "QQQ").is_err());
        assert!(arm_start(&f, start - 61_000).is_err());
        assert_eq!(arm_start(&f, start - 60_000).unwrap(), start);
    }
    #[test]
    fn reference_must_be_built_before_current_session_cutoff() {
        let f = frozen();
        let mut baseline = f.baseline.clone();
        baseline.cohorts.push(
            serde_json::from_value(serde_json::json!({
                "key":{"underlying":"QQQ","kind":"call","daysToExpiration":0,"minuteOffset":5,
                    "windowMinutes":5,"sessionMinutes":390,"moneynessBand":"[-0.25,0.25)",
                    "measurement":"eligible-option-bars"},
                "dates":["2026-10-09"],"sessions":1,"contractWindows":1,"status":"PENDING_SAMPLES",
                "volumeReferenceReady":false,"premiumReferenceReady":false,"premium":null,
                "volume":{"p25":100,"median":100,"p75":100,"p90":100,"p95":100,"mad":0}
            }))
            .unwrap(),
        );
        assert!(validate_reference(&baseline, &f.session, f.frozen_ms).is_ok());
        baseline.cutoff_ms += STEP;
        assert!(validate_reference(&baseline, &f.session, f.frozen_ms).is_err());
        baseline.cutoff_ms = f.session.open_ms;
        baseline.built_ms = f.session.open_ms;
        assert!(validate_reference(&baseline, &f.session, f.frozen_ms).is_err());
    }
    #[test]
    fn closed_market_and_first_open_minute_cannot_prepare() {
        assert!(session_at(calendar::legacy_eastern_ms("2026-10-10 10:00:00").unwrap()).is_err());
        let s = session();
        assert!(eligible_from(&s, s.open_ms + 59_999).is_err());
        assert_eq!(
            eligible_from(&s, s.open_ms + 60_000).unwrap(),
            s.open_ms + STEP
        );
        assert_eq!(
            eligible_from(&s, s.open_ms + STEP).unwrap(),
            s.open_ms + 2 * STEP
        );
    }
    #[test]
    fn windows_must_be_closed_eligible_and_fresh() {
        let f = frozen();
        let end = f.session.open_ms + 2 * STEP;
        assert!(closed_window(&f, end - 1).is_err());
        assert_eq!(closed_window(&f, end).unwrap(), (end - STEP, end));
        assert!(closed_window(&f, end + MAX_LAG + 1).is_err());
    }
    #[test]
    fn frozen_clock_and_different_day_cannot_be_replayed() {
        let mut f = frozen();
        let now = f.session.open_ms + 2 * STEP;
        f.frozen_ms = f.eligible_from_ms;
        assert!(closed_window(&f, now).is_err());
        f = frozen();
        assert!(closed_window(&f, now + 86_400_000).is_err());
    }
    #[test]
    fn cannot_replace_an_existing_observation() {
        let dir = std::env::temp_dir().join(format!(
            "option-observer-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("window.json");
        write_new(&path, &serde_json::json!({"value":1})).unwrap();
        assert!(write_new(&path, &serde_json::json!({"value":2})).is_err());
        assert!(fs::read_to_string(&path).unwrap().contains(":1"));
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&dir).unwrap();
    }
}
