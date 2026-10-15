//! On-demand EOD research. Bounded REST requests; does not touch streaming or brokers.
use crate::{credentials::Credentials, live::now_ms};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use snipers_core::investment::{number, Company, Input};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub symbols: String,
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    pub min_cap: f64,
    pub min_drop: f64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            symbols: String::new(),
            limit: 30,
            offset: 0,
            min_cap: 1_000_000_000.,
            min_drop: 15.,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<Vec<String>, String> {
        if self.offset > 50_000
            || !(1..=100).contains(&self.limit)
            || !self.min_cap.is_finite()
            || self.min_cap < 100_000_000.
            || self.min_cap > 1e14
            || !self.min_drop.is_finite()
            || !(0. ..=90.).contains(&self.min_drop)
        {
            return Err("Límite 1–100, capitalización ≥100M y caída 0–90%".into());
        }
        let mut symbols = self
            .symbols
            .split([',', ' ', '\n', ';'])
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_uppercase())
            .collect::<Vec<_>>();
        if symbols.len() > 100 || symbols.iter().any(|s| !valid_symbol(s)) {
            return Err("Usa hasta 100 símbolos válidos separados por comas".into());
        }
        symbols.sort();
        symbols.dedup();
        Ok(symbols)
    }
}
fn valid_symbol(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 12
        && s.bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'.' || c == b'-')
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub version: u32,
    pub running: bool,
    pub canceled: bool,
    pub started_ms: u64,
    pub finished_ms: u64,
    pub total: usize,
    pub completed: usize,
    pub universe: usize,
    pub current: String,
    pub error: Option<String>,
    pub rows: Vec<Company>,
    pub config: Config,
    pub excluded: usize,
}
struct Client {
    http: reqwest::Client,
    key: Credentials,
}
impl Client {
    async fn pages(
        &self,
        path: &str,
        query: &[(&str, String)],
        maximum: usize,
    ) -> Result<(Vec<Value>, bool), String> {
        let mut data = self.get(path, query).await?;
        let mut rows = Vec::new();
        let mut cursors = std::collections::BTreeSet::new();
        for page in 0..maximum {
            rows.extend(data["results"].as_array().cloned().unwrap_or_default());
            let Some(next) = data["next_url"].as_str().filter(|s| !s.is_empty()) else {
                return Ok((rows, true));
            };
            if page + 1 == maximum {
                return Ok((rows, false));
            }
            if !cursors.insert(next.to_owned()) {
                return Err("Paginación repetida".into());
            }
            let url = crate::history::checked_url(next, path).map_err(str::to_owned)?;
            let pairs = url
                .query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect::<Vec<_>>();
            let q = pairs
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect::<Vec<_>>();
            data = self.get(path, &q).await?;
        }
        Ok((rows, false))
    }
    async fn daily_trade_pages(&self,path:&str,query:&[(&str,String)],minimum:f64)->Result<(Vec<Value>,usize,bool),String>{
        let mut data=self.get(path,query).await?;let mut rows=Vec::new();let mut examined=0;let mut cursors=std::collections::BTreeSet::new();
        for page in 0..100 {
            if let Some(records)=data["results"].as_array(){
                examined+=records.len();
                rows.extend(records.iter().filter(|v|{
                    let size=v["decimal_size"].as_str().and_then(|s|s.parse::<f64>().ok()).or_else(||v["size"].as_f64());
                    v["exchange"]==4&&(v["correction"].as_u64().is_some_and(|c|c!=0)||v["price"].as_f64().zip(size).is_some_and(|(p,n)|p.is_finite()&&n.is_finite()&&p*n>=minimum))
                }).cloned());
            }
            let Some(next)=data["next_url"].as_str().filter(|s|!s.is_empty()) else{return Ok((rows,examined,true));};
            if page==99{return Ok((rows,examined,false));}
            if !cursors.insert(next.to_owned()){return Err("Paginación repetida".into());}
            let url=crate::history::checked_url(next,path).map_err(str::to_owned)?;
            let pairs=url.query_pairs().map(|(k,v)|(k.into_owned(),v.into_owned())).collect::<Vec<_>>();
            let q=pairs.iter().map(|(k,v)|(k.as_str(),v.clone())).collect::<Vec<_>>();data=self.get(path,&q).await?;
        }
        Ok((rows,examined,false))
    }
    fn load() -> Result<Self, String> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(18))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| "No se pudo preparar Massive")?,
            key: Credentials::load().map_err(str::to_owned)?,
        })
    }
    async fn get(&self, path: &str, q: &[(&str, String)]) -> Result<Value, String> {
        let mut response = self
            .http
            .get(format!("https://api.massive.com{path}"))
            .bearer_auth(&self.key.0)
            .query(q)
            .send()
            .await
            .map_err(|_| "Fallo de red Massive".to_string())?;
        if !response.status().is_success() {
            return Err(format!(
                "Massive HTTP {} · verificar permiso o disponibilidad",
                response.status().as_u16()
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| "Respuesta incompleta")? {
            if bytes.len() + chunk.len() > 24_000_000 {
                return Err("Respuesta supera límite local".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| "JSON Massive no válido")?;
        if (path.ends_with("/ratios") || path.starts_with("/v2/aggs/"))
            && value["next_url"].as_str().is_some_and(|s| !s.is_empty())
        {
            return Err(
                "Respuesta paginada: cobertura incompleta; no se usa como universo completo".into(),
            );
        }
        Ok(value)
    }
    async fn company(
        &self,
        r: &Value,
        config: &Config,
        benchmark: &Value,
        forward_enabled: &mut bool,
    ) -> Company {
        let ticker = r["ticker"].as_str().unwrap_or("");
        let now = now_ms();
        let start = snipers_core::calendar::date_at(now.saturating_sub(1100 * 86_400_000))
            .unwrap()
            .to_string();
        let end = snipers_core::calendar::date_at(now).unwrap().to_string();
        let base = [
            ("tickers", ticker.to_string()),
            ("sort", "period_end.desc".into()),
            ("limit", "60".into()),
        ];
        // At most two independent requests in flight. No subscription per company.
        let profile_path = format!("/v3/reference/tickers/{ticker}");
        let bars_path = format!("/v2/aggs/ticker/{ticker}/range/1/day/{start}/{end}");
        let bars_query = [
            ("adjusted", "true".into()),
            ("sort", "asc".into()),
            ("limit", "50000".into()),
        ];
        let (profile, bars) = tokio::join!(
            self.get(&profile_path, &[]),
            self.get(&bars_path, &bars_query)
        );
        let (income, cash) = tokio::join!(
            self.get("/stocks/financials/v1/income-statements", &base),
            self.get("/stocks/financials/v1/cash-flow-statements", &base)
        );
        let balance = self
            .get(
                "/stocks/financials/v1/balance-sheets",
                &[
                    ("tickers", ticker.into()),
                    ("timeframe", "quarterly".into()),
                    ("sort", "period_end.desc".into()),
                    ("limit", "4".into()),
                ],
            )
            .await;
        let mut errors = Vec::new();
        let mut unpack = |name: &str, result: Result<Value, String>| match result {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{name}: {e}"));
                json!({})
            }
        };
        let p = unpack("Identidad", profile);
        let b = unpack("Precios", bars);
        let i = unpack("Resultados", income);
        let c = unpack("Caja", cash);
        let balance = unpack("Balance", balance);
        let mut company = snipers_core::investment::analyze(Input {
            ratio: r,
            profile: &p,
            bars: &b,
            income: &i,
            cash: &c,
            balance: &balance,
            now,
            min_drop: config.min_drop,
        });
        let beta = snipers_core::investment_risk::beta(&b, benchmark, ticker, now);
        company.metrics.push(snipers_core::investment::Metric {
            id: "beta".into(), label: "Beta 1a vs SPY".into(), group: "Precio".into(),
            value: beta.map(|v| v.0), unit: "number".into(),
            source: beta.map(|v| format!("Covarianza / varianza SPY; {} retornos diarios coincidentes, precios ajustados por splits sin dividendos",v.1)).unwrap_or_else(|| "Sin 200 retornos diarios coincidentes vigentes o benchmark no disponible".into()),
        });
        let forward = if *forward_enabled {
            self.get(
                "/benzinga/v1/earnings",
                &[
                    ("ticker", ticker.into()),
                    ("date.gt", end.clone()),
                    ("limit", "100".into()),
                    ("sort", "date.asc".into()),
                ],
            )
            .await
        } else {
            Err("Fuente de estimaciones sin permiso en esta exploracion".into())
        };
        if forward
            .as_ref()
            .err()
            .is_some_and(|e| e.contains("HTTP 403") || e.contains("HTTP 401"))
        {
            *forward_enabled = false;
        }
        let price = company
            .metrics
            .iter()
            .find(|m| m.id == "last_close")
            .and_then(|m| m.value)
            .unwrap_or(0.);
        let estimate = forward
            .as_ref()
            .ok()
            .and_then(|v| snipers_core::investment_risk::forward_pe(v, ticker, price, now));
        company.metrics.push(snipers_core::investment::Metric {
            id: "forward_pe".into(), label: "P/E forward (4 trimestres pendientes)".into(), group: "Valoración".into(),
            value: estimate.as_ref().map(|v|v.0), unit: "x".into(),
            source: estimate.map(|v|v.1).unwrap_or_else(||forward.err().unwrap_or_else(||"Sin consenso verificable de cuatro trimestres consecutivos con BPA total positivo".into())),
        });
        snipers_core::investment::apply_research_metrics(&mut company);
        company.source_errors = errors;
        let analysts=crate::investment_fmp::enrich(&mut company).await;
        let mut opportunity=snipers_core::opportunity::calculate(&company,now_ms());
        opportunity.analysts=analysts;
        company.opportunity=Some(opportunity);
        company
    }
}
/// Explicit, bounded enrichment of ONE company. No subscriptions or brokerage access.
pub async fn research(
    ticker: &str,
    price: f64,
    price_date: &str,
    date: &str,
    session: &str,
    minimum: f64,
) -> Result<snipers_core::investment_structure::Research, String> {
    research_mode(ticker, price, price_date, date, session, minimum, false).await
}
pub async fn research_mode(
    ticker: &str,
    price: f64,
    price_date: &str,
    date: &str,
    session: &str,
    minimum: f64,
    structure_only: bool,
) -> Result<snipers_core::investment_structure::Research, String> {
    use snipers_core::{
        calendar::{self, Day},
        investment_structure as model,
    };
    if !valid_symbol(ticker)
        || !price.is_finite()
        || price <= 0.
        || !minimum.is_finite()
        || !(100_000. ..=100_000_000.).contains(&minimum)
    {
        return Err("Empresa, precio o importe mínimo inválido".into());
    }
    let now = now_ms();
    let reference = calendar::day_bounds(price_date)
        .ok_or("Cierre sin fecha verificada")?
        .0;
    if reference > now || now - reference > 7 * 86_400_000 {
        return Err(
            "Actualiza primero el análisis financiero: cierre de referencia vencido".into(),
        );
    }
    let today = calendar::date_at(now).ok_or("Fecha no disponible")?;
    let mut day = today;
    if !date.is_empty() {
        let bounds = calendar::day_bounds(date).ok_or("Fecha inválida")?;
        if bounds.0 > now || now - bounds.0 > 31 * 86_400_000 {
            return Err("Elige una fecha de los últimos 31 días".into());
        }
        day = calendar::date_at(bounds.0).ok_or("Fecha inválida")?;
    } else {
        for _ in 0..10 {
            if let Day::Open(s) = calendar::schedule(day) {
                if s.open_ms <= now {
                    break;
                }
            }
            day = day.pred_opt().ok_or("Fecha inválida")?;
        }
    }
    let Day::Open(schedule) = calendar::schedule(day) else {
        return Err("Fecha sin sesión bursátil verificada".into());
    };
    let label = day.to_string();
    let pre = calendar::legacy_eastern_ms(&format!("{label} 04:00:00")).ok_or("Hora inválida")?;
    let post = calendar::legacy_eastern_ms(&format!("{label} 20:00:00")).ok_or("Hora inválida")?;
    let (from, to) = match session {
        "pre" => (pre, schedule.open_ms),
        "post" => (schedule.close_ms, post),
        "regular" => (schedule.open_ms, schedule.close_ms),
        "all" => (pre, post),
        _ => return Err("Sesión inválida".into()),
    };
    let to = to.min(now.saturating_add(1));
    if from >= to {
        return Err("Esa franja todavía no ha comenzado".into());
    }
    let expiration_from = today.to_string();
    let expiration_to = calendar::date_at(now + 365 * 86_400_000)
        .ok_or("Fecha inválida")?
        .to_string();
    let client = Client::load()?;
    let option_path = format!("/v3/snapshot/options/{ticker}");
    let trade_path = format!("/v3/trades/{ticker}");
    let oq = [
        ("expiration_date.gte", expiration_from.clone()),
        ("expiration_date.lte", expiration_to.clone()),
        ("limit", "250".into()),
        ("sort", "ticker".into()),
        ("order", "asc".into()),
    ];
    let tq = [
        ("timestamp.gte", (from * 1_000_000).to_string()),
        ("timestamp.lt", (to * 1_000_000).to_string()),
        ("limit", "10000".into()),
        ("sort", "timestamp".into()),
        ("order", "desc".into()),
    ];
    let (options, trades) = tokio::join!(client.pages(&option_path, &oq, 16), async {
        if structure_only {
            Ok((Vec::new(), false))
        } else {
            client.pages(&trade_path, &tq, 3).await
        }
    });
    let mut errors = Vec::new();
    let (options, chain_complete) = options.unwrap_or_else(|e| {
        errors.push(format!("Opciones: {e}"));
        (Vec::new(), false)
    });
    let (trades, trade_complete) = trades.unwrap_or_else(|e| {
        errors.push(format!("Fuera de bolsa: {e}"));
        (Vec::new(), false)
    });
    let mut blocks = model::blocks(&trades, from, to, minimum);
    let mut quote_errors = 0;
    for b in &mut blocks {
        let Some(t) = b.execution_ms else {
            continue;
        };
        match client
            .get(
                &format!("/v3/quotes/{ticker}"),
                &[
                    ("timestamp.lte", (t * 1_000_000).to_string()),
                    (
                        "timestamp.gte",
                        (t.saturating_sub(2000) * 1_000_000).to_string(),
                    ),
                    ("sort", "timestamp".into()),
                    ("order", "desc".into()),
                    ("limit", "1".into()),
                ],
            )
            .await
        {
            Ok(v) => model::attach_quote(b, &v["results"][0]),
            Err(_) => quote_errors += 1,
        }
    }
    if quote_errors > 0 {
        errors.push(format!(
            "{quote_errors} consultas NBBO no disponibles; no se asigna sesgo"
        ));
    }
    let structure = model::levels(&options, ticker, price, &expiration_from, &expiration_to);
    Ok(model::Research {
        ticker: ticker.into(),
        fetched_ms: now_ms(),
        reference_price: price,
        reference_date: price_date.into(),
        expiration_from,
        expiration_to,
        chain_complete,
        chain_rows: options.len(),
        structure,
        trade_date: label,
        session: session.into(),
        trade_complete,
        trades_requested: !structure_only,
        examined_trades: trades.len(),
        from_ms: from,
        through_ms: to,
        minimum_notional: minimum,
        blocks,
        errors,
    })
}
/// Callback snapshots persist the current run and progress. Cancellation stops after the current company.
pub async fn scan(
    config: Config,
    cancel: Arc<AtomicBool>,
    mut update: impl FnMut(Report),
) -> Result<Report, String> {
    let symbols = config.validate()?;
    let client = Client::load()?;
    let mut report = Report {
        version: 2,
        running: true,
        started_ms: now_ms(),
        config: config.clone(),
        current: "Consultando universo de ratios de cierre".into(),
        ..Default::default()
    };
    update(report.clone());
    let data = client
        .get(
            "/stocks/financials/v1/ratios",
            &[
                ("limit", "50000".into()),
                ("sort", "market_cap.desc".into()),
            ],
        )
        .await?;
    let rows = data["results"]
        .as_array()
        .ok_or("No llegaron ratios financieros")?;
    report.universe = rows.len();
    let mut candidates = rows
        .iter()
        .filter(|r| r["ticker"].as_str().is_some_and(valid_symbol))
        .filter(|r| {
            if !symbols.is_empty() {
                symbols.iter().any(|s| r["ticker"] == *s)
            } else {
                number(r, "market_cap").is_some_and(|v| v >= config.min_cap)
                    && number(r, "average_volume").is_some_and(|v| v >= 100_000.)
            }
        })
        .collect::<Vec<_>>();
    // Liquidity/capitalization bounded universe avoids ranking unverified cheap-looking ratios as quality.
    candidates.sort_by(|a, b| {
        number(b, "market_cap")
            .unwrap_or(0.)
            .total_cmp(&number(a, "market_cap").unwrap_or(0.))
    });
    candidates.dedup_by(|a, b| a["ticker"] == b["ticker"]);
    report.excluded = report.universe - candidates.len();
    if symbols.is_empty() {
        candidates = candidates.into_iter().skip(config.offset).collect();
    }
    report.total = candidates.len().min(config.limit);
    candidates.truncate(config.limit);
    if !symbols.is_empty() {
        let missing = symbols
            .iter()
            .filter(|s| !candidates.iter().any(|r| r["ticker"] == **s))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            report.error = Some(format!(
                "Sin ratios o fuera del límite elegido: {}",
                missing.join(", ")
            ));
        }
    }
    // One shared benchmark request per scan; never a subscription per company.
    let now = now_ms();
    let start = snipers_core::calendar::date_at(now.saturating_sub(400 * 86400000)).unwrap();
    let end = snipers_core::calendar::date_at(now).unwrap();
    let benchmark = client
        .get(
            &format!("/v2/aggs/ticker/SPY/range/1/day/{start}/{end}"),
            &[
                ("adjusted", "true".into()),
                ("sort", "asc".into()),
                ("limit", "50000".into()),
            ],
        )
        .await
        .unwrap_or_else(|_| json!({}));
    let mut forward_enabled = true;
    for r in candidates {
        if cancel.load(Ordering::SeqCst) {
            report.canceled = true;
            break;
        }
        report.current = format!(
            "Analizando {} · estados financieros e histórico",
            r["ticker"].as_str().unwrap_or("")
        );
        update(report.clone());
        report.rows.push(
            client
                .company(r, &config, &benchmark, &mut forward_enabled)
                .await,
        );
        report.completed += 1;
        snipers_core::opportunity::peers(&mut report.rows);
        update(report.clone());
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    report
        .rows
        .sort_by(|a, b| b.score.unwrap_or(-1.).total_cmp(&a.score.unwrap_or(-1.)));
    report.running = false;
    report.finished_ms = now_ms();
    report.current = if report.canceled {
        "Cancelado; resultados parciales"
    } else {
        "Análisis finalizado"
    }
    .into();
    update(report.clone());
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_rejects_path_injection_and_invalid_numbers() {
        let mut c = Config {
            symbols: "../../env".into(),
            ..Default::default()
        };
        assert!(c.validate().is_err());
        c.symbols = "aapl, MSFT,aapl".into();
        assert_eq!(c.validate().unwrap(), vec!["AAPL", "MSFT"]);
        c.min_drop = f64::NAN;
        assert!(c.validate().is_err());
    }
}

pub async fn analyze_individual(ticker: &str) -> Result<Company, String> {
    let ticker = normalize_individual(ticker)?;
    let client = Client::load()?;
    let ratios = client
        .get(
            "/stocks/financials/v1/ratios",
            &[("ticker", ticker.clone()), ("limit", "100".into())],
        )
        .await;
    let ratio = individual_ratio(&ticker, ratios.as_ref().ok());
    let now = now_ms();
    let start = snipers_core::calendar::date_at(now.saturating_sub(400 * 86_400_000))
        .ok_or("Fecha inválida")?;
    let end = snipers_core::calendar::date_at(now).ok_or("Fecha inválida")?;
    let benchmark = client
        .get(
            &format!("/v2/aggs/ticker/SPY/range/1/day/{start}/{end}"),
            &[
                ("adjusted", "true".into()),
                ("sort", "asc".into()),
                ("limit", "50000".into()),
            ],
        )
        .await
        .unwrap_or_else(|_| json!({}));
    let mut forward = true;
    let mut company = client
        .company(&ratio, &Config::default(), &benchmark, &mut forward)
        .await;
    if company.name.is_empty() {
        return Err(format!(
            "No se pudo verificar la compañía {ticker}. Revisa el símbolo y el acceso a Massive."
        ));
    }
    if ratio.get("date").is_none() {
        company.source_errors.push(match ratios { Err(e)=>format!("Ratios: {e}"), _=>"Sin ratios disponibles para esta compañía; se muestran las demás fuentes disponibles".into() });
    }
    Ok(company)
}
fn normalize_individual(ticker: &str) -> Result<String, String> {
    let ticker = ticker.trim().to_ascii_uppercase();
    if !valid_symbol(&ticker) || !ticker.bytes().any(|b| b.is_ascii_alphabetic()) {
        return Err("Escribe un solo símbolo, por ejemplo NIO o AAPL".into());
    }
    Ok(ticker)
}
fn individual_ratio(ticker: &str, data: Option<&Value>) -> Value {
    data.and_then(|d| d["results"].as_array())
        .and_then(|r| r.iter().find(|r| r["ticker"] == ticker))
        .cloned()
        .unwrap_or_else(|| json!({"ticker":ticker}))
}
#[cfg(test)]
mod individual_tests {
    use super::*;
    #[test]
    fn accepts_single_symbol_outside_local_universe() {
        assert_eq!(normalize_individual(" nio ").unwrap(), "NIO");
        assert_eq!(normalize_individual("BRK.B").unwrap(), "BRK.B");
        for s in ["", "NIO,AAPL", "../NIO", "123", "AAPL MSFT"] {
            assert!(normalize_individual(s).is_err());
        }
    }
    #[test]
    fn missing_ratios_do_not_block_individual_research_or_borrow_other_company() {
        assert_eq!(individual_ratio("NIO", None), json!({"ticker":"NIO"}));
        let data =
            json!({"results":[{"ticker":"AAPL","market_cap":999},{"ticker":"NIO","market_cap":1}]});
        assert_eq!(individual_ratio("NIO", Some(&data))["market_cap"], 1);
        assert!(individual_ratio("OUTSIDE", Some(&data))
            .get("market_cap")
            .is_none());
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct DailyPurchases { pub date:String, pub through_ms:u64, pub rows:Vec<DailyPurchase>, pub coverage:Vec<String> }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct DailyPurchase { pub ticker:String, pub block:snipers_core::investment_structure::Block, pub execution:Value }
/// Manual, bounded stock REST scan. Never opens a streaming connection.
pub async fn daily_purchases(symbols:&str,minimum:f64)->Result<DailyPurchases,String>{
    use snipers_core::{calendar,investment_structure as model};
    let tickers=Config{symbols:symbols.into(),limit:10,..Config::default()}.validate()?;
    if tickers.is_empty()||tickers.len()>10||!minimum.is_finite()||!(100_000. ..=1_000_000_000.).contains(&minimum){return Err("Indica entre 1 y 10 símbolos y un mínimo de 100.000 a 1.000 millones".into());}
    let now=now_ms();let date=calendar::date_at(now).ok_or("Fecha no disponible")?.to_string();
    let switch=calendar::legacy_eastern_ms(&format!("{date} 16:00:00")).ok_or("Hora inválida")?;
    let from=if now<switch{calendar::legacy_eastern_ms(&format!("{date} 04:00:00")).ok_or("Hora inválida")?}else{switch};
    let end=if now<switch{switch}else{calendar::legacy_eastern_ms(&format!("{date} 20:00:00")).ok_or("Hora inválida")?};
    let to=end.min(now);if to<=from{return Err("La búsqueda de bloques comienza a las 04:00 ET".into());}
    let client=Client::load()?;let mut out=DailyPurchases{date,through_ms:to,rows:vec![],coverage:vec![]};
    for ticker in tickers {
        let query=[("timestamp.gte",(from*1_000_000).to_string()),("timestamp.lt",(to*1_000_000).to_string()),("limit","10000".into()),("sort","timestamp".into()),("order","desc".into())];
        let (trades,examined,complete)=match client.daily_trade_pages(&format!("/v3/trades/{ticker}"),&query,minimum).await{Ok(v)=>v,Err(e)=>{out.coverage.push(format!("{ticker}: error {e}"));continue;}};
        let mut blocks=model::daily_blocks(&trades,from,to,minimum);
        blocks.retain(|b|b.execution_ms.is_none_or(|t|t>=from&&t<to));
        let count=blocks.len();let mut unknown=0;
        for (index, mut block) in blocks.into_iter().enumerate() {
            let execution_ns=trades.iter().find(|v|v["id"].as_str()==Some(block.id.as_str())&&v["exchange"]==4&&v["trf_id"].as_u64()==Some(block.trf_id)).and_then(|v|v["participant_timestamp"].as_u64());
            if index>=200 {block.nbbo="NBBO sin consultar · límite de 200 bloques".into();}
            else if let Some(ns)=execution_ns.filter(|ns|block.execution_ms==Some(ns/1_000_000)) {
                let from=ns.saturating_sub(2_000_000_000);
                let q=[("timestamp.lte",ns.to_string()),("timestamp.gte",from.to_string()),("sort","timestamp".into()),("order","desc".into()),("limit","1".into())];
                match client.get(&format!("/v3/quotes/{ticker}"),&q).await {
                    Ok(v)=>match crate::off_exchange::parse_block_quote(&v,from,ns) {
                        Ok(Some(_))=>model::attach_quote(&mut block,&v["results"][0]),
                        Ok(None)=>block.nbbo=if v["results"].as_array().is_some_and(|a|a.is_empty()){"Sin cotización previa en 2 s"}else{"Cotización NBBO inválida"}.into(),
                        Err(_)=>block.nbbo="Cotización NBBO inválida".into(),
                    },
                    Err(e)=>block.nbbo=format!("NBBO no disponible · {e}"),
                }
            } else {block.nbbo="Hora de ejecución no verificable".into();}
            let valid=block.bid.zip(block.ask).is_some_and(|(b,a)|(a-b)/((a+b)/2.)<=0.01);
            if !valid {block.nbbo="Sin NBBO verificable".into();}
            if !matches!(block.nbbo.as_str(),"En ask"|"Sobre ask"|"En bid"|"Bajo bid"){unknown+=1;}
            let mut execution=trades.iter().find(|v|v["id"].as_str()==Some(block.id.as_str())&&v["exchange"]==4&&v["trf_id"].as_u64()==Some(block.trf_id)).cloned().unwrap_or(Value::Null);
            if let Some(fields)=execution.as_object_mut(){for field in ["participant_timestamp","sip_timestamp","trf_timestamp"]{if let Some(v)=fields.get_mut(field){if let Some(ns)=v.as_u64(){*v=Value::String(ns.to_string());}}}}
            out.rows.push(DailyPurchase{ticker:ticker.clone(),block,execution});
        }
        out.coverage.push(format!("{ticker}: {} · {} operaciones examinadas · {count} bloques encontrados sobre el mínimo solicitado · {unknown} sin dirección verificable",if complete{"histórico recibido"}else{"histórico parcial; límite de 1.000.000 operaciones"},examined));
    }
    out.rows.sort_by(|a,b|b.block.notional.total_cmp(&a.block.notional));Ok(out)
}

/// Active US stock-market symbols, including ETFs; never inferred from financial eligibility.
pub async fn daily_purchases_universe()->Result<Vec<String>,String>{
    let client=Client::load()?;
    let (rows,complete)=client.pages("/v3/reference/tickers",&[("market","stocks".into()),("locale","us".into()),("active","true".into()),("limit","1000".into()),("sort","ticker".into()),("order","asc".into())],100).await?;
    if !complete{return Err("Universo parcial; no se presenta como mercado completo".into());}
    let mut symbols=rows.iter().filter_map(|r|r["ticker"].as_str()).filter(|s|valid_symbol(s)).map(str::to_owned).collect::<Vec<_>>();symbols.sort();symbols.dedup();Ok(symbols)
}
#[cfg(test)]
mod dark_pool_probe {
    use super::*;
    #[tokio::test]
    #[ignore = "Manual Massive entitlement and historical SMH print probe"]
    async fn smh_october_seventh_print() {
        let client=Client::load().unwrap();
        let from=snipers_core::calendar::legacy_eastern_ms("2026-10-07 14:43:00").unwrap()*1_000_000;
        let to=snipers_core::calendar::legacy_eastern_ms("2026-10-07 14:46:00").unwrap()*1_000_000;
        let trades=client.get("/v3/trades/SMH",&[("timestamp.gte",from.to_string()),("timestamp.lt",to.to_string()),("limit","50000".into()),("sort","timestamp".into()),("order","desc".into())]).await.unwrap();
        let rows=trades["results"].as_array().unwrap();
        let matches=rows.iter().filter(|r|r["price"].as_f64().is_some_and(|p|(p-623.38).abs()<0.001)&&r["size"].as_f64().is_some_and(|s|(s-160000.).abs()<1.)).collect::<Vec<_>>();
        println!("SMH 14:43–14:46: {} trades, {} matching 160000 @ 623.38",rows.len(),matches.len());
        for row in matches {
            let ns=row["participant_timestamp"].as_u64().unwrap();let lower=ns.saturating_sub(2_000_000_000);
            let quote=client.get("/v3/quotes/SMH",&[("timestamp.gte",lower.to_string()),("timestamp.lte",ns.to_string()),("limit","1".into()),("sort","timestamp".into()),("order","desc".into())]).await;
            match quote {Ok(value)=>{let parsed=crate::off_exchange::parse_block_quote(&value,lower,ns).unwrap();println!("TRF={} conditions={:?} quote_rows={} bid_ask={:?} quote_age_ms={:?}",row["trf_id"],row["conditions"],value["results"].as_array().map_or(0,Vec::len),parsed.map(|q|(q.bid,q.ask)),parsed.map(|q|ns/1_000_000-q.timestamp_ms));},Err(error)=>println!("quote_error={error}")}
        }
    }
}

/// Intraday Benzinga news: bounded pages, server-side credentials and explicit coverage.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsReport { pub items: Vec<Value>, pub fetched_ms:u64, pub truncated:bool }

pub async fn benzinga_intraday_news(ticker:&str)->Result<NewsReport,String>{
    let ticker=ticker.trim().to_ascii_uppercase();
    if !ticker.is_empty()&&!valid_symbol(&ticker){return Err("Escribe un solo símbolo válido".into());}
    let now=now_ms();
    let cutoff=now.saturating_sub(86_400_000);
    let client=Client::load()?;
    let mut query=vec![("published.gte",(cutoff/1000).to_string()),("sort","published.desc".into()),("limit","200".into())];
    if !ticker.is_empty(){query.push(("tickers",ticker.clone()));}
    let mut page=client.get("/benzinga/v2/news",&query).await?;
    let mut items=Vec::new();let mut ids=std::collections::BTreeSet::new();let mut truncated=false;
    for index in 0..3 {
        if page["status"].as_str()!=Some("OK"){return Err("Benzinga devolvió un estado no válido".into());}
        let rows=page["results"].as_array().ok_or("Respuesta Benzinga sin artículos")?;
        for row in rows {
            let Some(ms)=row["published"].as_str().and_then(snipers_core::calendar::rfc3339_ms) else{continue;};
            if ms<cutoff||ms>now {continue;}
            let tickers=row["tickers"].as_array().map(|a|a.iter().filter_map(Value::as_str).collect::<Vec<_>>()).unwrap_or_default();
            if !ticker.is_empty()&&!tickers.iter().any(|t|*t==ticker){continue;}
            let id=row["benzinga_id"].to_string();
            if id=="null"||!ids.insert(id.clone()){continue;}
            items.push(json!({"id":id,"ticker":if tickers.is_empty(){"Mercado".into()}else{tickers.join(" · ")},"title":row["title"].as_str().unwrap_or("Sin titular"),"summary":row["teaser"].as_str().unwrap_or("Solo titular; consulta la fuente original."),"tickerSymbols":tickers,"providerArticleId":row["benzinga_id"],"providerUpdatedAt":row["last_updated"],"author":row["author"],"channels":row["channels"],"tags":row["tags"],"source":"Benzinga","publishedAt":row["published"],"sentiment":"unknown","horizon":"intraday","url":row["url"]}));
        }
        let Some(next)=page["next_url"].as_str().filter(|s|!s.is_empty()) else{break;};
        if index==2 {truncated=true;break;}
        let url=reqwest::Url::parse(next).map_err(|_|"Paginación Benzinga no válida")?;
        if url.scheme()!="https"||url.host_str()!=Some("api.massive.com")||url.path()!="/benzinga/v2/news"||!url.username().is_empty()||url.password().is_some()||url.port_or_known_default()!=Some(443){return Err("Paginación Benzinga fuera del endpoint permitido".into());}
        let pairs=url.query_pairs().filter(|(k,_)|k!="apiKey").map(|(k,v)|(k.into_owned(),v.into_owned())).collect::<Vec<_>>();
        let q=pairs.iter().map(|(k,v)|(k.as_str(),v.clone())).collect::<Vec<_>>();
        page=client.get("/benzinga/v2/news",&q).await?;
    }
    for item in &mut items {crate::news_intelligence::decorate(item,now_ms(),&ticker);}
    items.sort_by(|a,b|b["publishedAt"].as_str().cmp(&a["publishedAt"].as_str()));
    Ok(NewsReport{items,fetched_ms:now_ms(),truncated})
}

