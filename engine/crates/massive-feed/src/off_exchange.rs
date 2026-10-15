//! A bounded independent analytics consumer; never opens a second Stocks socket.
use crate::{
    credentials::Credentials,
    history::checked_url,
    live::{now_ms, SharedState},
    Event,
};
use serde::Serialize;
use serde_json::Value;
use snipers_core::{
    calendar::{self, Day},
    off_exchange::Quote,
    off_exchange_tape::{Engine, Snapshot},
    off_exchange_trade::{normalize, Source, Trade},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tokio::sync::mpsc;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStatus {
    pub phase: String,
    pub detail: String,
    pub pages: usize,
    pub stock_trades: usize,
    pub off_exchange_trades: usize,
    pub through_ms: Option<u64>,
    pub checked_ms: Option<u64>,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub block_quote_status:String,
    #[serde(skip)] pub block_quote_requests:Vec<u64>,
    pub analytics: Snapshot,
    pub history: BTreeMap<String, HistoryStatus>,
    pub queue_drops: u64,
    pub error: Option<String>,
    pub recovery_since_ms: Option<u64>,
    #[serde(skip)]
    pub recovered_queue_drops: u64,
}
impl State {
    pub(crate) fn gap(&mut self, received_ms: u64) {
        self.recovery_since_ms = Some(self.recovery_since_ms.unwrap_or(0).max(received_ms));
    }
    fn history_recovered(&mut self, date: &str, status: &HistoryStatus, now: u64) {
        if calendar::date_at(now).is_some_and(|d| d.to_string() == date)
            && status.phase == "ready"
            && self
                .recovery_since_ms
                .is_some_and(|needed| status.through_ms.is_some_and(|t| t >= needed && t <= now))
        {
            self.recovery_since_ms = None;
            self.recovered_queue_drops = self.queue_drops;
            self.error = None;
        }
    }
}
pub enum Input {
    BlockQuotes {date:String,values:Vec<(u64,Quote)>,failed:usize},
    HistoryApplied(tokio::sync::oneshot::Sender<()>),
    Event(Event, u64),
    History {
        date: String,
        trades: Vec<Trade>,
        status: HistoryStatus,
    },
    HistoryError {
        date: String,
        detail: String,
    },
}
pub struct HistoryDay {
    pub date: String,
    pub trades: Vec<Trade>,
    pub pages: usize,
    pub stock_trades: usize,
    pub through_ms: u64,
}
// A successful intraday download is not a complete closed session after midnight.
fn reusable_closed_day(date: &str, now: u64, status: &HistoryStatus) -> bool {
    calendar::day_bounds(date).is_some_and(|(_, end)| {
        now >= end
            && status.phase == "ready"
            && status.through_ms.is_some_and(|through| through >= end - 1)
            && status.checked_ms.is_some_and(|checked| {
                checked <= now && calendar::date_at(checked) == calendar::date_at(now)
            })
    })
}
fn reusable_history(date: &str, now: u64, status: &HistoryStatus, recovery: Option<u64>) -> bool {
    if reusable_closed_day(date, now, status) {
        return true;
    }
    calendar::date_at(now).is_some_and(|d| d.to_string() == date)
        && status.phase == "ready"
        && status.through_ms.is_some_and(|t| {
            t <= now && now - t < 300_000 && recovery.is_none_or(|needed| t >= needed)
        })
}
pub fn dates(now: u64) -> Result<Vec<String>, &'static str> {
    let today = calendar::date_at(now).ok_or("Fecha inválida")?;
    let mut result = vec![today.to_string()];
    let mut day = today;
    for _ in 0..14 {
        day = day.pred_opt().ok_or("Fecha inválida")?;
        match calendar::schedule(day) {
            Day::Open(_) => result.push(day.to_string()),
            Day::Unknown => return Err("Calendario oficial fuera de cobertura"),
            Day::Closed => {}
        }
        if result.len() == 6 {
            return Ok(result);
        }
    }
    Err("No se encontraron cinco sesiones previas")
}
async fn page(
    client: &reqwest::Client,
    key: &Credentials,
    url: reqwest::Url,
) -> Result<Value, &'static str> {
    for attempt in 0..3 {
        match client.get(url.clone()).bearer_auth(&key.0).send().await {
            Ok(mut response) if response.status().is_success() => {
                let mut body = Vec::new();
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| "Respuesta TRF interrumpida")?
                {
                    if body.len() + chunk.len() > 32 * 1024 * 1024 {
                        return Err("Página TRF excede 32 MB");
                    }
                    body.extend_from_slice(&chunk);
                }
                return serde_json::from_slice(&body).map_err(|_| "Respuesta TRF inválida");
            }
            Ok(response) if [401, 403].contains(&response.status().as_u16()) => {
                return Err("Historial TRF rechazado (401/403)")
            }
            Ok(response)
                if response.status().as_u16() == 429 || response.status().is_server_error() =>
            {
                if attempt == 2 {
                    return Err("Historial TRF temporalmente limitado");
                }
                let delay = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1 << attempt)
                    .clamp(1, 60);
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }
            Ok(_) => return Err("Error HTTP consultando historial TRF"),
            Err(_) if attempt < 2 => tokio::time::sleep(Duration::from_secs(1 << attempt)).await,
            Err(_) => return Err("No se pudo recibir historial TRF"),
        }
    }
    Err("No se pudo recibir historial TRF")
}
pub async fn fetch(
    key: &Credentials,
    symbol: &str,
    date: &str,
    through: u64,
) -> Result<HistoryDay, &'static str> {
    fetch_report(key, symbol, date, through, |_, _, _| {}).await
}
pub async fn fetch_report(
    key: &Credentials,
    symbol: &str,
    date: &str,
    through: u64,
    mut progress: impl FnMut(usize, usize, usize),
) -> Result<HistoryDay, &'static str> {
    if snipers_core::universe::stock_asset(symbol).is_none()
        || !dates(through)?.iter().any(|d| d == date)
    {
        return Err("Activo/fecha no habilitados para historial TRF");
    }
    let path = format!("/v3/trades/{symbol}");
    let (start, end) = calendar::day_bounds(date).ok_or("Fecha TRF inválida")?;
    let from = start.checked_mul(1_000_000).ok_or("Fecha fuera de rango")?;
    let until = end
        .min(through.saturating_add(1))
        .checked_mul(1_000_000)
        .ok_or("Fecha fuera de rango")?;
    let mut url=checked_url(&format!("https://api.massive.com{path}?timestamp.gte={from}&timestamp.lt={until}&order=asc&sort=timestamp&limit=50000"),&path)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "No se pudo preparar REST TRF")?;
    let mut seen = BTreeSet::new();
    let mut trades = vec![];
    let mut count = 0;
    for pages in 1..=100 {
        if !seen.insert(url.to_string()) {
            return Err("Paginación TRF repetida");
        }
        let page = page(&client, key, url).await?;
        if page["status"] != "OK" {
            return Err("Historial TRF no confirmado por Massive");
        }
        if let Some(rows) = page.get("results").filter(|v| !v.is_null()) {
            let rows = rows
                .as_array()
                .filter(|r| r.len() <= 50_000)
                .ok_or("Página TRF inválida")?;
            count += rows.len();
            for row in rows {
                let sip = row["sip_timestamp"]
                    .as_u64()
                    .ok_or("Trade histórico sin SIP timestamp")?
                    / 1_000_000;
                if calendar::date_at(sip).map(|d| d.to_string()).as_deref() != Some(date) {
                    return Err("Paginación TRF de otra fecha");
                }
                // Freeze current-day coverage at request start; newer trades arrive via WS.
                if sip > through {
                    continue;
                }
                if let Some(trade) = normalize(row, symbol, Source::History)? {
                    if trades.len() >= snipers_core::off_exchange_tape::SESSION_LIMIT {
                        return Err("Historial TRF excede 5.000.000 prints por sesión");
                    }
                    trades.push(trade);
                }
            }
        }
        progress(pages, count, trades.len());
        match page.get("next_url").filter(|v| !v.is_null()) {
            None => {
                return Ok(HistoryDay {
                    date: date.into(),
                    trades,
                    pages,
                    stock_trades: count,
                    through_ms: through,
                })
            }
            Some(next) => url = checked_url(next.as_str().ok_or("Cursor TRF inválido")?, &path)?,
        }
    }
    Err("Historial TRF parcial: quedan páginas después del límite")
}
async fn history_worker(key: &Credentials, state: SharedState, tx: mpsc::Sender<Input>) {
    let (symbol, notify) = {
        let s = state.lock().unwrap();
        (s.symbol.clone(), s.off_exchange_notify.clone())
    };
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {_=interval.tick()=>{},_=notify.notified()=>{}}
        if !matches!(
            state.lock().unwrap().stocks.phase.as_str(),
            "waiting" | "streaming"
        ) {
            continue;
        }
        let now = now_ms();
        let Ok(days) = dates(now) else {
            state.lock().unwrap().off_exchange.error =
                Some("Calendario fuera de cobertura para cinco sesiones".into());
            continue;
        };
        state
            .lock()
            .unwrap()
            .off_exchange
            .history
            .retain(|date, _| days.contains(date));
        for date in days {
            let cached = {
                let s = state.lock().unwrap();
                !s.off_exchange
                    .analytics
                    .history
                    .iter()
                    .any(|h| h.date == date && !h.history_complete)
                    && s.off_exchange.history.get(&date).is_some_and(|h| {
                        reusable_history(&date, now, h, s.off_exchange.recovery_since_ms)
                    })
            };
            if cached {
                continue;
            }
            state.lock().unwrap().off_exchange.history.insert(
                date.clone(),
                HistoryStatus {
                    phase: "loading".into(),
                    detail: "Descargando páginas; no se incorpora historial parcial".into(),
                    ..Default::default()
                },
            );
            let input = match fetch_report(
                key,
                &symbol,
                &date,
                now,
                |pages, stock_trades, off_exchange_trades| {
                    if let Some(status) = state.lock().unwrap().off_exchange.history.get_mut(&date)
                    {
                        status.pages = pages;
                        status.stock_trades = stock_trades;
                        status.off_exchange_trades = off_exchange_trades;
                    }
                },
            )
            .await
            {
                Ok(day) => Input::History {
                    date: day.date,
                    status: HistoryStatus {
                        phase: "ready".into(),
                        detail: "Todas las páginas recibidas; pendiente fusión atómica".into(),
                        pages: day.pages,
                        stock_trades: day.stock_trades,
                        off_exchange_trades: day.trades.len(),
                        through_ms: Some(day.through_ms),
                        checked_ms: Some(now_ms()),
                    },
                    trades: day.trades,
                },
                Err(error) => Input::HistoryError {
                    date,
                    detail: error.into(),
                },
            };
            if commit_history(&tx, input).await.is_err() {
                return;
            }
        }
    }
}
async fn commit_history(tx: &mpsc::Sender<Input>, input: Input) -> Result<(), ()> {
    tx.send(input).await.map_err(|_| ())?;
    let (ack, received) = tokio::sync::oneshot::channel();
    tx.send(Input::HistoryApplied(ack)).await.map_err(|_| ())?;
    received.await.map_err(|_| ())
}

fn publish_history_state(engine: &mut Engine, state: &SharedState) {
    let now = now_ms();
    let mut snapshot = engine.snapshot(now);
    let mut s = state.lock().unwrap();
    let intact = s.off_exchange.queue_drops == s.off_exchange.recovered_queue_drops
        && s.off_exchange.error.is_none();
    let history_ready = s
        .off_exchange
        .history
        .get(&snapshot.session.date)
        .is_some_and(|h| {
            h.phase == "ready"
                && s.off_exchange
                    .recovery_since_ms
                    .is_none_or(|needed| h.through_ms.is_some_and(|t| t >= needed))
        });
    snapshot.session.complete &=
        intact && history_ready && matches!(s.stocks.phase.as_str(), "waiting" | "streaming");
    for window in &mut snapshot.windows {
        window.complete &= snapshot.session.complete;
    }
    for day in &mut snapshot.history {
        day.complete &= s
            .off_exchange
            .history
            .get(&day.date)
            .is_some_and(|h| reusable_closed_day(&day.date, now, h));
    }
    snapshot.blocks.premarket.complete &= snapshot.session.complete;
    snapshot.blocks.regular.complete &= snapshot.session.complete;
    if !snapshot.session.complete {
        snapshot.blocks.premarket.dominant="INSUFFICIENT".into();
        snapshot.blocks.regular.dominant="INSUFFICIENT".into();
    }
    s.off_exchange.block_quote_requests=snapshot.blocks.quote_requests.clone();
    s.off_exchange.analytics = snapshot;
}
async fn consume(state: SharedState, mut rx: mpsc::Receiver<Input>) {
    let symbol = state.lock().unwrap().symbol.clone();
    let mut engine = Engine::new(&symbol).expect("validated stock universe");
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _=interval.tick()=>{
                publish_history_state(&mut engine, &state);
            },
            input=rx.recv()=>match input {
                Some(Input::BlockQuotes{date,values,failed})=>{
                    if calendar::date_at(now_ms()).map(|d|d.to_string()).as_deref()==Some(&date){
                        for (at,q) in values {engine.block_quote(at,q);}
                        state.lock().unwrap().off_exchange.block_quote_status=if failed>0 {format!("Cotizaciones históricas: {failed} consultas fallaron; cobertura parcial")}else{"Cotizaciones históricas consultadas; dirección estimada solo con bid/ask válidos".into()};
                    }
                },
                None=>return,
                Some(Input::HistoryApplied(ack))=>{publish_history_state(&mut engine, &state);let _=ack.send(());},
                Some(Input::Event(Event::Quote{bp,ap,t,..},_))=>engine.quote(Quote{bid:bp,ask:ap,timestamp_ms:t}),
                Some(Input::Event(event @ Event::Trade{..},received))=>{
                    match serde_json::to_value(event).map_err(|_|"Evento TRF inválido").and_then(|v|normalize(&v,&symbol,Source::Live)) {
                        Ok(Some(trade))=>engine.trade(trade,received),Ok(None)=>{},Err(error)=>{let mut s=state.lock().unwrap();s.off_exchange.error=Some(error.into());s.off_exchange.gap(received);},
                    }
                },
                Some(Input::Event(_, _))=>{},
                Some(Input::History{date,trades,mut status})=>{
                    let result=tokio::task::spawn_blocking(move || {let result=engine.history(&date,trades,now_ms(),true);(engine,date,result)}).await;
                    let Ok((returned,date,result))=result else {state.lock().unwrap().off_exchange.error=Some("Falló la fusión del historial TRF".into());return;};engine=returned;
                    match result {Ok(())=>status.detail="Historial completo fusionado sin duplicados REST/WS".into(),Err(error)=>{status.phase="failed".into();status.detail=error.into();}}
                    let mut s=state.lock().unwrap();
                    s.off_exchange.history_recovered(&date,&status,now_ms());
                    s.off_exchange.history.insert(date,status);
                },
                Some(Input::HistoryError{date,detail})=>{state.lock().unwrap().off_exchange.history.insert(date,HistoryStatus{phase:"failed".into(),detail,..Default::default()});}
            }
        }
    }
}
pub async fn worker(
    key: &Credentials,
    state: SharedState,
    tx: mpsc::Sender<Input>,
    rx: mpsc::Receiver<Input>,
) {
    tokio::join!(history_worker(key, state.clone(), tx.clone()), block_quote_worker(key,state.clone(),tx), consume(state, rx));
}

async fn block_quote(key:&Credentials,symbol:&str,at:u64)->Result<Option<Quote>,&'static str>{
    let path=format!("/v3/quotes/{symbol}");
    let until=at.checked_mul(1_000_000).ok_or("Fecha inválida")?;
    let from=at.saturating_sub(snipers_core::off_exchange::QUOTE_MAX_AGE_MS)*1_000_000;
    let url=checked_url(&format!("https://api.massive.com{path}?timestamp.gte={from}&timestamp.lte={until}&order=desc&sort=timestamp&limit=1"),&path)?;
    let client=reqwest::Client::builder().timeout(Duration::from_secs(8)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_|"REST no disponible")?;
    let mut response=client.get(url).bearer_auth(&key.0).send().await.map_err(|_|"Cotización histórica no disponible")?;
    if !response.status().is_success(){return Err("Cotización histórica rechazada o limitada");}
    let mut bytes=Vec::new();
    while let Some(chunk)=response.chunk().await.map_err(|_|"Respuesta interrumpida")?{
        if bytes.len()+chunk.len()>1024*1024{return Err("Respuesta fuera de límite");}bytes.extend_from_slice(&chunk);
    }
    let value:Value=serde_json::from_slice(&bytes).map_err(|_|"Respuesta inválida")?;
    if value["status"]!="OK"{return Err("Cotización histórica no confirmada");}
    parse_block_quote(&value,from,until)
}
pub(crate) fn parse_block_quote(value:&Value,from:u64,until:u64)->Result<Option<Quote>,&'static str>{
    let Some(row)=value["results"].as_array().and_then(|r|r.first()) else{return Ok(None)};
    // conditions is OPTIONAL in the official API (and absent in verified NBBO rows).
    // Reject an explicitly abnormal condition, never reject absence alone.
    if row.get("conditions").filter(|v|!v.is_null()).is_some_and(|v|!v.as_array().is_some_and(|c|c.iter().all(|v|v.as_u64()==Some(1))))
        || !["bid_size","ask_size"].iter().all(|k|row[k].as_f64().is_some_and(|n|n.is_finite()&&n>0.)){return Ok(None);}
    let Some(ns)=row["sip_timestamp"].as_u64().filter(|ns|*ns<=until&&*ns>=from) else{return Ok(None)};
    let Some((bid,ask))=row["bid_price"].as_f64().zip(row["ask_price"].as_f64()) else{return Ok(None)};
    if !bid.is_finite()||!ask.is_finite()||bid<=0.||ask<=bid{return Ok(None);}
    Ok(Some(Quote{bid,ask,timestamp_ms:ns/1_000_000}))
}
async fn block_quote_worker(key:&Credentials,state:SharedState,tx:mpsc::Sender<Input>){
    let mut tried=BTreeMap::<u64,u64>::new();
    let mut interval=tokio::time::interval(Duration::from_secs(30));interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop{
        interval.tick().await;let now=now_ms();let Some(date)=calendar::date_at(now).map(|d|d.to_string()) else{continue};
        tried.retain(|at,_|calendar::date_at(*at).map(|d|d.to_string()).as_deref()==Some(&date));
        let (symbol,requests)={let s=state.lock().unwrap();(s.symbol.clone(),s.off_exchange.block_quote_requests.clone())};
        let requests:Vec<_>=requests.into_iter().filter(|at|tried.get(at).is_none_or(|retry|now>=*retry)).take(60).collect();
        if requests.is_empty(){continue;}
        state.lock().unwrap().off_exchange.block_quote_status="Recuperando bid/ask en la hora de cada bloque; porcentajes provisionales".into();
        let mut values=vec![];let mut failed=0;
        for batch in requests.chunks(4){
            let results=futures_util::future::join_all(batch.iter().map(|at|block_quote(key,&symbol,*at))).await;
            for (&at,result) in batch.iter().zip(results){
                tried.insert(at,now_ms()+300_000);
                match result{Ok(Some(q))=>values.push((at,q)),Ok(None)=>{},Err(_)=>failed+=1}
            }
        }
        if tx.send(Input::BlockQuotes{date,values,failed}).await.is_err(){return;}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "Explicit official API probe; uses local credentials and historical market data"]
    async fn official_premarket_block_probe(){
        let key=Credentials::load().unwrap();
        let date=std::env::var("SNIPERS_BLOCK_PROBE_DATE").unwrap_or_else(|_|"2026-09-30".into());
        let through=calendar::legacy_eastern_ms(&format!("{date} 09:30:00")).unwrap();
        let day=fetch(&key,"QQQ",&date,through).await.unwrap();
        let start=calendar::legacy_eastern_ms(&format!("{date} 04:00:00")).unwrap();
        let selected:Vec<_>=day.trades.iter().rev().filter(|t|t.price*t.shares>=snipers_core::off_exchange::MIN_NOTIONAL&&snipers_core::block_pressure::conditions_eligible(t))
            .filter_map(snipers_core::block_pressure::execution).filter(|at|*at>=start&&*at<through).take(12).collect();
        let mut engine=Engine::new("QQQ").unwrap();engine.history(&day.date,day.trades,through,true).unwrap();
        let mut recovered=0;
        for batch in selected.chunks(4){
            for (&at,result) in batch.iter().zip(futures_util::future::join_all(batch.iter().map(|at|block_quote(&key,"QQQ",*at))).await){
                if let Some(q)=result.unwrap(){engine.block_quote(at,q);recovered+=1;}
            }
        }
        let report=engine.snapshot(through).blocks;
        assert!(report.premarket.count>0);assert!(recovered>0);
        assert!(report.premarket.buy+report.premarket.sell>0.);
        assert!((report.premarket.total-report.premarket.buy-report.premarket.sell-report.premarket.unknown).abs()<0.01);
        println!("Official probe: {} premarket blocks, {} historical quotes, {:.2}% classified coverage (12-query validation sample)",report.premarket.count,recovered,report.premarket.coverage_pct.unwrap());
    }
    #[test]
    fn historical_nbbo_accepts_optional_conditions_but_rejects_future_one_sided_and_abnormal(){
        let until=1790774992357000000u64;let from=until-2_000_000_000;
        let mut v=serde_json::json!({"results":[{"ask_price":740.16,"bid_price":740.15,"ask_size":80,"bid_size":1800,"sip_timestamp":1790774992346973587u64}]});
        assert!(parse_block_quote(&v,from,until).unwrap().is_some());
        for (field,value) in [("sip_timestamp",serde_json::json!(until+1)),("ask_size",serde_json::json!(0)),("ask_price",serde_json::json!(740.15)),("conditions",serde_json::json!([2])),("conditions",serde_json::json!([0]))]{
            let mut changed=v.clone();changed["results"][0][field]=value;assert!(parse_block_quote(&changed,from,until).unwrap().is_none());
        }
        v["results"][0]["conditions"]=serde_json::json!([1]);assert!(parse_block_quote(&v,from,until).unwrap().is_some());
    }
    #[tokio::test]
    async fn download_cannot_start_next_cycle_before_merge_is_acknowledged() {
        let (tx, mut rx) = mpsc::channel(4);
        let task = tokio::spawn(async move {
            commit_history(
                &tx,
                Input::HistoryError {
                    date: "2026-09-25".into(),
                    detail: "test".into(),
                },
            )
            .await
        });
        assert!(matches!(rx.recv().await, Some(Input::HistoryError { .. })));
        let Some(Input::HistoryApplied(ack)) = rx.recv().await else {
            panic!("missing merge acknowledgement");
        };
        tokio::task::yield_now().await;
        assert!(
            !task.is_finished(),
            "queue admission must not be treated as a completed merge"
        );
        ack.send(()).unwrap();
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn merge_ack_publishes_current_history_before_cache_can_be_reused() {
        let state = crate::live::new_state();
        state.lock().unwrap().stocks.phase = "waiting".into();
        let now = now_ms();
        let date = calendar::date_at(now)
            .unwrap()
            .pred_opt()
            .unwrap()
            .to_string();
        let (start, end) = calendar::day_bounds(&date).unwrap();
        let sip = start + 14 * 3_600_000;
        let row = normalize(&serde_json::json!({"price":500,"size":300,"exchange":4,"trf_id":12,"id":"commit-test","sip_timestamp":sip*1_000_000}), "QQQ", Source::History).unwrap().unwrap();
        let (tx, rx) = mpsc::channel(4);
        let consumer = tokio::spawn(consume(state.clone(), rx));
        commit_history(
            &tx,
            Input::History {
                date: date.clone(),
                trades: vec![row],
                status: HistoryStatus {
                    phase: "ready".into(),
                    through_ms: Some(end),
                    checked_ms: Some(now),
                    ..Default::default()
                },
            },
        )
        .await
        .unwrap();
        {
            let s = state.lock().unwrap();
            let h = s
                .off_exchange
                .analytics
                .history
                .iter()
                .find(|h| h.date == date)
                .unwrap();
            assert!(h.history_complete);
            assert_eq!(h.print_count, 1);
            assert!(reusable_closed_day(
                &date,
                now,
                &s.off_exchange.history[&date]
            ));
        }
        drop(tx);
        consumer.await.unwrap();
    }

    #[tokio::test]
    async fn previous_day_prefix_is_not_certified_as_a_complete_closed_session() {
        let state = crate::live::new_state();
        state.lock().unwrap().stocks.phase = "waiting".into();
        let (tx, rx) = mpsc::channel(4);
        let task = tokio::spawn(consume(state.clone(), rx));
        let now = now_ms();
        let date = calendar::date_at(now)
            .unwrap()
            .pred_opt()
            .unwrap()
            .to_string();
        let (start, end) = calendar::day_bounds(&date).unwrap();
        let sip = start + 14 * 3_600_000;
        let row=normalize(&serde_json::json!({"price":500,"size":300,"exchange":4,"trf_id":12,"id":"one","sip_timestamp":sip*1_000_000}),"QQQ",Source::History).unwrap().unwrap();
        tx.send(Input::History {
            date: date.clone(),
            trades: vec![row.clone()],
            status: HistoryStatus {
                phase: "ready".into(),
                through_ms: Some(sip + 1000),
                checked_ms: Some(now),
                ..Default::default()
            },
        })
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        {
            let s = state.lock().unwrap();
            let day = s
                .off_exchange
                .analytics
                .history
                .iter()
                .find(|h| h.date == date)
                .unwrap();
            assert_eq!(day.print_count, 1);
            assert!(
                !day.complete,
                "a download completed before midnight is only a session prefix"
            );
        }
        tx.send(Input::History {
            date: date.clone(),
            trades: vec![row],
            status: HistoryStatus {
                phase: "ready".into(),
                through_ms: Some(end),
                checked_ms: Some(now),
                ..Default::default()
            },
        })
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(
            state
                .lock()
                .unwrap()
                .off_exchange
                .analytics
                .history
                .iter()
                .find(|h| h.date == date)
                .unwrap()
                .complete
        );
        task.abort();
        let _ = task.await;
    }
    #[test]
    fn historical_recovery_requires_current_day_and_coverage_after_the_latest_gap() {
        let now = now_ms();
        let date = calendar::date_at(now).unwrap().to_string();
        let mut state = State {
            queue_drops: 3,
            error: Some("parse error".into()),
            ..Default::default()
        };
        state.gap(now - 10);
        let mut status = HistoryStatus {
            phase: "ready".into(),
            through_ms: Some(now - 11),
            ..Default::default()
        };
        state.history_recovered(&date, &status, now);
        assert!(state.error.is_some());
        assert_eq!(state.recovered_queue_drops, 0);
        status.through_ms = Some(now);
        state.history_recovered("2000-01-01", &status, now);
        assert!(state.error.is_some());
        status.phase = "failed".into();
        state.history_recovered(&date, &status, now);
        assert!(state.error.is_some());
        status.phase = "ready".into();
        state.history_recovered(&date, &status, now);
        assert!(state.error.is_none());
        assert_eq!(state.recovery_since_ms, None);
        assert_eq!(state.queue_drops, 3);
        assert_eq!(state.recovered_queue_drops, 3);
        state.queue_drops += 1;
        state.gap(now + 10);
        state.gap(now + 5);
        state.history_recovered(&date, &status, now + 20);
        assert_eq!(state.recovery_since_ms, Some(now + 10));
        assert_ne!(state.queue_drops, state.recovered_queue_drops);
    }
    #[test]
    fn retries_failed_history_and_refreshes_today_without_reconnecting() {
        let now = now_ms();
        let date = calendar::date_at(now).unwrap().to_string();
        let mut status = HistoryStatus {
            phase: "ready".into(),
            through_ms: Some(now),
            ..Default::default()
        };
        assert!(reusable_history(&date, now, &status, None));
        assert!(!reusable_history(&date, now + 300_000, &status, None));
        assert!(!reusable_history(&date, now + 1, &status, Some(now + 1)));
        status.phase = "failed".into();
        assert!(!reusable_history(&date, now, &status, None));
    }
    #[test]
    fn midnight_requires_reloading_an_intraday_history_even_across_dst() {
        for date in ["2026-09-23", "2026-03-08", "2026-11-01"] {
            let (start, end) = calendar::day_bounds(date).unwrap();
            let mut status = HistoryStatus {
                phase: "ready".into(),
                through_ms: Some(start + 3_600_000),
                ..Default::default()
            };
            assert!(!reusable_closed_day(date, end, &status));
            status.through_ms = Some(end - 1);
            status.checked_ms = Some(end);
            assert!(!reusable_closed_day(date, end - 1, &status));
            assert!(reusable_closed_day(date, end, &status));
            assert!(!reusable_closed_day(date, end + 86_400_000, &status));
            status.phase = "failed".into();
            assert!(!reusable_closed_day(date, end, &status));
            status.phase = "ready".into();
            status.through_ms = None;
            assert!(!reusable_closed_day(date, end, &status));
        }
    }
    #[test]
    fn five_sessions_skip_weekends_holidays_and_reject_foreign_pagination() {
        let now = calendar::day_bounds("2026-09-08").unwrap().0 + 12 * 3_600_000;
        assert_eq!(
            dates(now).unwrap(),
            [
                "2026-09-08",
                "2026-09-04",
                "2026-09-03",
                "2026-09-02",
                "2026-09-01",
                "2026-08-31"
            ]
        );
        assert!(checked_url("https://other.test/v3/trades/QQQ", "/v3/trades/QQQ").is_err());
        assert!(checked_url("https://api.massive.com/v3/trades/SPY", "/v3/trades/QQQ").is_err());
        let (a, b) = calendar::day_bounds("2026-03-08").unwrap();
        assert_eq!(b - a, 23 * 3_600_000);
        let (a, b) = calendar::day_bounds("2026-11-01").unwrap();
        assert_eq!(b - a, 25 * 3_600_000);
    }
    #[tokio::test]
    async fn consumer_merges_history_and_live_without_an_extra_socket() {
        let state = crate::live::new_state();
        state.lock().unwrap().stocks.phase = "waiting".into();
        let (tx, rx) = mpsc::channel(10);
        let task = tokio::spawn(consume(state.clone(), rx));
        let now = now_ms();
        let date = calendar::date_at(now).unwrap().to_string();
        let event:Event=serde_json::from_value(serde_json::json!({"ev":"T","sym":"QQQ","x":4,"trfi":12,"p":500.0,"s":300,"t":now,"i":"one","q":1})).unwrap();
        let trade = normalize(&serde_json::to_value(&event).unwrap(), "QQQ", Source::Live)
            .unwrap()
            .unwrap();
        tx.send(Input::Event(event, now)).await.unwrap();
        let historical = Trade {
            source: Source::History,
            ..trade
        };
        tx.send(Input::History {
            date: date.clone(),
            trades: vec![historical],
            status: HistoryStatus {
                phase: "ready".into(),
                through_ms: Some(now),
                ..Default::default()
            },
        })
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        {
            let s = state.lock().unwrap();
            assert_eq!(s.off_exchange.analytics.session.print_count, 1);
            assert_eq!(s.off_exchange.analytics.diagnostics.duplicates, 1);
            assert!(s.off_exchange.analytics.session.complete);
        }
        state.lock().unwrap().off_exchange.recovery_since_ms = Some(now + 1);
        tokio::time::sleep(Duration::from_millis(1000)).await;
        assert!(
            !state
                .lock()
                .unwrap()
                .off_exchange
                .analytics
                .session
                .complete,
            "pre-reconnect history must not certify recovery"
        );
        tx.send(Input::History {
            date: date.clone(),
            trades: vec![],
            status: HistoryStatus {
                phase: "ready".into(),
                through_ms: Some(now_ms()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(1000)).await;
        assert!(
            state
                .lock()
                .unwrap()
                .off_exchange
                .analytics
                .session
                .complete
        );
        {
            let mut s = state.lock().unwrap();
            s.off_exchange.queue_drops = 1;
            s.off_exchange.gap(now_ms());
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
        assert!(
            !state
                .lock()
                .unwrap()
                .off_exchange
                .analytics
                .session
                .complete
        );
        tx.send(Input::History {
            date,
            trades: vec![],
            status: HistoryStatus {
                phase: "ready".into(),
                through_ms: Some(now_ms()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        {
            let s = state.lock().unwrap();
            assert!(s.off_exchange.analytics.session.complete);
            assert_eq!(s.off_exchange.queue_drops, 1, "keep the diagnostic counter");
            assert_eq!(s.off_exchange.recovered_queue_drops, 1);
        }
        task.abort();
        let _ = task.await;
    }
}
