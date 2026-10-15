//! Production option quotes for Paper execution. Massive is the data source, not Alpaca indicative.
use crate::{credentials::Credentials, live::now_ms};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub bid: f64,
    pub ask: f64,
    pub bid_size: u64,
    pub ask_size: u64,
    pub timestamp_ms: u64,
    pub received_ms: u64,
}
impl Quote {
    pub fn validate(&self, now: u64, entry: bool, budget: f64) -> Result<(), String> {
        if self.timestamp_ms > now {
            return Err(format!(
                "Cotización Massive con reloj adelantado {} ms",
                self.timestamp_ms - now
            ));
        }
        if now.saturating_sub(self.timestamp_ms) > 5000 {
            return Err(format!(
                "Cotización Massive caducada: {} ms (máximo 5000 ms)",
                now.saturating_sub(self.timestamp_ms)
            ));
        }
        if !self.bid.is_finite()
            || !self.ask.is_finite()
            || self.bid <= 0.0
            || self.ask < self.bid
            || self.bid_size == 0
            || self.ask_size == 0
        {
            return Err("Cotización Massive ausente, caducada o no operable".into());
        }
        // The submitted limit rounds the ask UP to cents. Budget that actual
        // limit, not the raw quote (e.g. 3.005 becomes 3.01 = 301 USD).
        let limit_premium = self.limit(true).parse::<f64>().unwrap_or(f64::INFINITY) * 100.0;
        if entry
            && (!budget.is_finite()
                || budget <= 0.0
                || (self.ask - self.bid) / self.ask > 0.10
                || limit_premium > budget)
        {
            return Err(
                "Entrada bloqueada por spread superior al 10% o prima sobre el presupuesto".into(),
            );
        }
        Ok(())
    }
    pub fn limit(&self, buy: bool) -> String {
        format!(
            "{:.2}",
            if buy {
                (self.ask * 100.0 - 1e-9).ceil() / 100.0
            } else {
                (self.bid * 100.0 + 1e-9).floor() / 100.0
            }
        )
    }
}
pub struct Quotes {
    client: reqwest::Client,
    key: Credentials,
}
pub fn structural_calendar_days(spot:f64,target:f64,daily_range:f64,side:&str)->Result<i64,String>{
    if ![spot,target,daily_range].iter().all(|n|n.is_finite()&&*n>0.)||!matches!(side,"CALL"|"PUT")||(target-spot)*if side=="CALL"{1.}else{-1.}<=0.{return Err("Recorrido estructural o rango diario inválido".into());}
    Ok((((target-spot).abs()/daily_range*2.).ceil().clamp(1.,6.)*7.) as i64)
}
fn aggressive_expiry_priority(expiry:&str,minimum:&str,maximum:&str)->Option<String>{
    if expiry<minimum||expiry>maximum{return None;}
    let (start, end) = snipers_core::calendar::day_bounds(expiry)?;
    let date = snipers_core::calendar::date_at(start + (end - start) / 2)?;
    if !matches!(snipers_core::calendar::schedule(date), snipers_core::calendar::Day::Open(_)) { return None; }
    Some(expiry.to_owned())
}
pub fn expiry_window(session:&str,mode:&str)->Result<(String,String,i64),String>{
    let (ms,_) = snipers_core::calendar::day_bounds(session).ok_or("Fecha inválida")?;
    let date=snipers_core::calendar::date_at(ms).ok_or("Fecha inválida")?;
    let days=match mode {"zero_dte"=>0,"next"=>1,"three_days"=>3,"week"=>7,_=>return Err("Modo de vencimiento inválido".into())};
    let add=|n:i64| (0..n).try_fold(date,|d,_|d.succ_opt()).map(|d|d.to_string()).ok_or_else(||"Fecha fuera de rango".to_owned());
    Ok((add(days)?,add(if days==0 {0}else{45})?,days))
}
impl Quotes {
    /// Earliest liquid expiration within the explicitly selected expiry window.
    pub async fn select_auto(&self,root:&str,side:&str,spot:f64,target:f64,daily_range:f64,budget:f64,expiry_mode:&str)->Result<(Quote,Value),String>{
        if snipers_core::universe::stock_asset(root).is_none()||!matches!(side,"CALL"|"PUT")||![spot,target,daily_range,budget].iter().all(|n|n.is_finite()&&*n>0.){return Err("Faltan estructura, rango diario o presupuesto para elegir vencimiento".into());}
        let direction=if side=="CALL"{1.}else{-1.};
        if (target-spot)*direction<=0.{return Err("Objetivo sin recorrido".into());}
        
        let now=now_ms();let today=snipers_core::calendar::date_at(now).ok_or("Fecha inválida")?;
        let (minimum,maximum,desired_days)=expiry_window(&today.to_string(),expiry_mode)?;
        let path=format!("/v3/snapshot/options/{root}");
        let mut query=vec![("expiration_date.gte".to_owned(),minimum.clone()),("expiration_date.lte".into(),maximum.clone()),("contract_type".into(),side.to_lowercase()),("strike_price.gte".into(),(spot-(spot*0.02).max(5.)).to_string()),("strike_price.lte".into(),(spot+(spot*0.02).max(5.)).to_string()),("limit".into(),"250".into())];
        let mut rows=vec![];let mut complete=false;
        for _ in 0..8 {
            let params:Vec<_>=query.iter().map(|(k,v)|(k.as_str(),v.clone())).collect();
            let page=self.get(&path,&params).await?;
            rows.extend(page["results"].as_array().ok_or("Cadena no disponible")?.clone());
            let Some(next)=page["next_url"].as_str() else {complete=true;break};
            let url=crate::history::checked_url(next,&path).map_err(str::to_owned)?;
            query=url.query_pairs().map(|(k,v)|(k.into_owned(),v.into_owned())).collect();
        }
        if !complete{return Err("Cadena paginada incompleta; esperar nueva evaluación".into());}

        let mut candidates:Vec<_>=rows.iter().filter_map(|r|{
            let d=&r["details"];let ticker=d["ticker"].as_str()?;
            let expiry=d["expiration_date"].as_str()?;let strike=d["strike_price"].as_f64()?;
            let bid=r["last_quote"]["bid"].as_f64()?;let ask=r["last_quote"]["ask"].as_f64()?;
            if snipers_core::options::underlying(ticker)!=Some(root)||d["contract_type"]!=side.to_lowercase()||d["shares_per_contract"]!=100||r["last_quote"]["timeframe"]!="REAL-TIME"||expiry<minimum.clone().as_str()||expiry>maximum.clone().as_str()||!strike.is_finite()||!bid.is_finite()||!ask.is_finite()||bid<=0.||ask<bid||(ask-bid)/ask>0.03||ask*100.>budget{return None;}
            let priority=aggressive_expiry_priority(expiry,&minimum.clone(),&maximum.clone())?;
            Some((priority,(strike-spot).abs(),ticker.trim_start_matches("O:").to_owned(),expiry.to_owned()))
        }).collect();
        candidates.sort_by(|a,b|a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
        let (_,_,ticker,expiry)=candidates.first().ok_or("Sin contrato líquido en el vencimiento elegido dentro del presupuesto")?;
        let quote=self.quote(ticker).await?;quote.validate(now_ms(),true,budget)?;
        Ok((quote,serde_json::json!({"expiry":expiry,"target_days":desired_days,"expiry_mode":expiry_mode,"target":target,"daily_range":daily_range,"reason":"Vencimiento según modo seleccionado; 0DTE solo hoy, otros modos desde su fecha mínima. Contrato líquido dentro del presupuesto","management":"structure-swing-v1"})))
    }
    /// Independent read-only snapshot; never blocks the execution/exit loop.
    pub async fn sensitivities(
        &self,
        symbol: &str,
    ) -> Result<snipers_core::theta_vega::Read, String> {
        let ticker = format!("O:{}", symbol.strip_prefix("O:").unwrap_or(symbol));
        let root = snipers_core::options::underlying(&ticker)
            .filter(|r| *r == "QQQ")
            .ok_or("Contrato QQQ inválido")?;
        let value = self
            .get(&format!("/v3/snapshot/options/{root}/{ticker}"), &[])
            .await?;
        if value["results"]["details"]["ticker"] != ticker {
            return Err("Snapshot de otro contrato".into());
        }
        let mut c =
            snipers_core::options::normalize(&value["results"], root).map_err(str::to_owned)?;
        let observed = now_ms();
        // Reference failures leave dollars unknown, not an assumed 100 multiplier.
        if let Ok(reference) = self
            .get(&format!("/v3/reference/options/contracts/{ticker}"), &[])
            .await
        {
            c.premium_multiplier =
                snipers_core::options::verified_multiplier(&reference["results"], root);
        }
        Ok(snipers_core::theta_vega::read(&c, observed, now_ms()))
    }
    pub async fn spot(&self) -> Result<f64, String> {
        let v = self
            .get("/v2/snapshot/locale/us/markets/stocks/tickers/QQQ", &[])
            .await?;
        v["ticker"]["lastTrade"]["p"]
            .as_f64()
            .filter(|p| p.is_finite() && *p > 0.)
            .ok_or("Precio QQQ no disponible".into())
    }
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| "Cliente Massive no disponible")?,
            key: Credentials::load().map_err(str::to_owned)?,
        })
    }
    async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, String> {
        let r = self
            .client
            .get(format!("https://api.massive.com{path}"))
            .bearer_auth(&self.key.0)
            .query(query)
            .send()
            .await
            .map_err(|_| "Massive no respondió")?;
        if !r.status().is_success() {
            return Err(format!(
                "Massive rechazó la consulta (HTTP {})",
                r.status().as_u16()
            ));
        }
        r.json()
            .await
            .map_err(|_| "Respuesta de Massive inválida".into())
    }
    pub async fn quote(&self, symbol: &str) -> Result<Quote, String> {
        crate::alpaca::OrderIntent {
            symbol: symbol.into(),
            qty: 1,
            limit_price: "1".into(),
            side: crate::alpaca::OrderSide::Buy,
            client_order_id: "snipers-quote".into(),
        }
        .validate()?;
        let data = self
            .get(
                &format!("/v3/quotes/O:{symbol}"),
                &[
                    ("sort", "timestamp".into()),
                    ("order", "desc".into()),
                    ("limit", "1".into()),
                ],
            )
            .await?;
        let q = &data["results"][0];
        let quote = Quote {
            symbol: symbol.into(),
            bid: q["bid_price"].as_f64().ok_or("Falta bid")?,
            ask: q["ask_price"].as_f64().ok_or("Falta ask")?,
            bid_size: q["bid_size"].as_u64().ok_or("Falta tamaño bid")?,
            ask_size: q["ask_size"].as_u64().ok_or("Falta tamaño ask")?,
            timestamp_ms: q["sip_timestamp"]
                .as_u64()
                .ok_or("Falta hora de cotización")?
                / 1_000_000,
            received_ms: now_ms(),
        };
        // Retain the original provider timestamp and wait only for bounded skew.
        // Validation still rejects every future/stale quote after this wait.
        let lead = quote.timestamp_ms.saturating_sub(now_ms());
        if (1..=1000).contains(&lead) {
            tokio::time::sleep(std::time::Duration::from_millis(lead)).await;
        }
        quote.validate(now_ms(), false, 0.)?;
        Ok(quote)
    }
    pub async fn select(&self,side:&str,expiry:&str,spot:f64)->Result<Quote,String>{self.select_asset("QQQ",side,expiry,spot).await}
    pub async fn select_asset(&self, root:&str, side: &str, expiry: &str, spot: f64) -> Result<Quote, String> {
        if snipers_core::universe::stock_asset(root).is_none(){return Err("Activo no admitido".into());}
        if !matches!(side, "CALL" | "PUT")
            || !spot.is_finite()
            || spot <= 0.0
            || snipers_core::calendar::day_bounds(expiry).is_none()
        {
            return Err("Selección de contrato inválida".into());
        }
        let data = self
            .get(
                &format!("/v3/snapshot/options/{root}"),
                &[
                    ("expiration_date", expiry.into()),
                    ("contract_type", side.to_lowercase()),
                    ("strike_price.gte", (spot - 5.).to_string()),
                    ("strike_price.lte", (spot + 5.).to_string()),
                    ("limit", "250".into()),
                ],
            )
            .await?;
        if data["next_url"].is_string() {
            return Err("Cadena incompleta: no se selecciona sobre una página parcial".into());
        }
        let rows = data["results"].as_array().ok_or("Cadena no disponible")?;
        let symbol = select_symbol(rows, side, expiry, spot)?;
        if snipers_core::options::underlying(&format!("O:{symbol}"))!=Some(root){return Err("Contrato de otro activo".into());}
        self.quote(&symbol).await
    }
}
fn select_symbol(rows: &[Value], side: &str, expiry: &str, spot: f64) -> Result<String, String> {
    let mut eligible: Vec<_> = rows
        .iter()
        .filter_map(|r| {
            let d = &r["details"];
            let ticker = d["ticker"].as_str()?.strip_prefix("O:")?;
            if d["contract_type"] != side.to_lowercase()
                || d["expiration_date"] != expiry
                || d["shares_per_contract"].as_u64() != Some(100)
                || r["last_quote"]["timeframe"] != "REAL-TIME"
            {
                return None;
            }
            let strike = d["strike_price"].as_f64()?;
            if !strike.is_finite() {
                return None;
            }
            Some(((strike - spot).abs(), ticker.to_owned()))
        })
        .collect();
    eligible.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    eligible
        .first()
        .map(|r| r.1.clone())
        .ok_or("No hay contrato estándar con cotizaciones REAL-TIME".into())
}
#[cfg(test)]
mod tests {
    #[test] fn selected_expiry_windows_are_strict(){
        assert_eq!(super::expiry_window("2026-10-09","zero_dte").unwrap(),("2026-10-09".into(),"2026-10-09".into(),0));
        assert_eq!(super::expiry_window("2026-10-09","next").unwrap().0,"2026-10-10");
        assert_eq!(super::expiry_window("2026-10-09","three_days").unwrap().0,"2026-10-12");
        assert_eq!(super::expiry_window("2026-12-28","week").unwrap().0,"2027-01-04");
        assert!(super::expiry_window("2026-10-09","bad").is_err());
        assert!(super::aggressive_expiry_priority("2026-10-12","2026-10-09","2026-10-09").is_none());
    }
    use super::*;
    #[test] fn aggressive_expiry_skips_early_weekend_and_missing_dates(){
        let minimum="2026-10-17";let maximum="2026-11-28";
        let mut dates=["2026-10-16","2026-10-17","2026-10-19","2026-10-23","2026-11-26","2026-12-04"].into_iter().filter_map(|d|aggressive_expiry_priority(d,minimum,maximum)).collect::<Vec<_>>();dates.sort();
        assert_eq!(dates,vec!["2026-10-19","2026-10-23"]);
        assert_eq!(aggressive_expiry_priority("2026-10-12","2026-10-12",maximum),Some("2026-10-12".into()));
        assert!(aggressive_expiry_priority("not-a-date",minimum,maximum).is_none());
    }
    #[test] fn calendar_buffer_is_structural_symmetric_and_never_zero_dte(){
        for (distance,days) in [(2.,7),(7.,14),(12.,21),(60.,42)]{
            assert_eq!(structural_calendar_days(100.,100.+distance,10.,"CALL").unwrap(),days);
            assert_eq!(structural_calendar_days(100.,100.-distance,10.,"PUT").unwrap(),days);
        }
        assert!(structural_calendar_days(100.,90.,10.,"CALL").is_err());assert!(structural_calendar_days(100.,110.,0.,"CALL").is_err());
        assert!(structural_calendar_days(100.,110.,f64::NAN,"CALL").is_err());
    }
    fn q() -> Quote {
        Quote {
            symbol: "QQQ260925C00745000".into(),
            bid: 1.,
            ask: 1.02,
            bid_size: 10,
            ask_size: 10,
            timestamp_ms: 1000,
            received_ms: 1100,
        }
    }
    #[test]
    fn stale_future_crossed_and_empty_sizes_block() {
        for n in [999, 6001] {
            assert!(q().validate(n, true, 300.).is_err());
        }
        let mut q = q();
        q.bid_size = 0;
        assert!(q.validate(1100, true, 300.).is_err());
        q.bid_size = 1;
        q.bid = 2.;
        assert!(q.validate(1100, true, 300.).is_err());
    }
    #[test]
    fn budget_and_spread_apply_to_entry_not_exit() {
        let mut q = q();
        assert!(q.validate(1100, true, 50.).is_err());
        q.ask = 2.;
        assert!(q.validate(1100, true, 300.).is_err());
        assert!(q.validate(1100, false, 0.).is_ok());
    }
    #[test]
    fn entry_budget_covers_rounded_limit_and_rejects_invalid_budget() {
        let mut quote = q();
        quote.bid = 2.99;
        quote.ask = 3.005;
        assert_eq!(quote.limit(true), "3.01");
        assert!(quote.validate(1100, true, 300.50).is_err());
        assert!(quote.validate(1100, true, 301.0).is_ok());
        for budget in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
            assert!(quote.validate(1100, true, budget).is_err());
        }
        assert!(quote.validate(1100, false, 0.).is_ok());
    }
    #[test]
    fn explicit_quote_limits() {
        assert_eq!(q().limit(true), "1.02");
        assert_eq!(q().limit(false), "1.00");
        let mut p = q();
        p.ask = 1.11;
        p.bid = 1.13;
        assert_eq!(p.limit(true), "1.11");
        assert_eq!(p.limit(false), "1.13");
    }
    #[test]
    fn contract_selection_rejects_delayed_and_adjusted() {
        let row = serde_json::json!({"details":{"ticker":"O:QQQ260925C00745000","contract_type":"call","expiration_date":"2026-09-25","shares_per_contract":100,"strike_price":745},"last_quote":{"timeframe":"REAL-TIME"}});
        assert!(select_symbol(std::slice::from_ref(&row), "CALL", "2026-09-25", 745.).is_ok());
        let mut delayed = row.clone();
        delayed["last_quote"]["timeframe"] = "DELAYED".into();
        assert!(select_symbol(&[delayed], "CALL", "2026-09-25", 745.).is_err());
        let mut adjusted = row;
        adjusted["details"]["shares_per_contract"] = 10.into();
        assert!(select_symbol(&[adjusted], "CALL", "2026-09-25", 745.).is_err());
    }
}
