//! Independent passive calculations, fed by the existing sockets. No order lane.
use crate::{credentials::Credentials, live::{self, SharedState}, Event, Market};
use serde_json::{json, Value};
pub const SYMBOLS: [&str; 10] = ["AAPL","MSFT","AMZN","GOOGL","META","NVDA","TSLA","QQQ","SPY","IWM"];

pub fn enabled() -> bool {
    std::env::var("SNIPERS_SNAPSHOT_ENABLED").ok().as_deref()==Some("1") || crate::snapshot_bridge::root().ok().is_some_and(|r|r.join("data/snapshots/connection.secret").is_file()) ||
    std::env::current_dir().ok().is_some_and(|p| p.ancestors().any(|a|a.join("data/snapshots/connection.secret").is_file() || a.join("snipers-rust/data/snapshots/connection.secret").is_file()))
}
pub fn initialize(parent: &SharedState) {
    if !enabled() { return; }
    let mut s=parent.lock().unwrap();
    for symbol in SYMBOLS {
        s.snapshot_lanes.entry(symbol.into()).or_insert_with(||live::new_state_for(symbol).expect("snapshot asset catalog"));
    }
    for lane in s.snapshot_lanes.values(){lane.lock().unwrap().oi_directory=s.oi_directory.clone();}
}
pub fn fanout(parent: &SharedState, market: Market, event: &Event, now: u64) {
    let root=match event {Event::Trade{sym,..}|Event::Quote{sym,..}|Event::SecondBar{sym,..}|Event::MinuteBar{sym,..}=>{
        if matches!(market,Market::Options){snipers_core::options::underlying(sym)}else{Some(sym.as_str())}
    },_=>None};
    let Some(root)=root else{return};
    let lane=parent.lock().unwrap().snapshot_lanes.get(root).cloned();
    if let Some(lane)=lane {
        let mut s=lane.lock().unwrap();
        if let Event::Trade{p,t,..}=event {if matches!(market,Market::Stocks)&&*t<=now&&now-*t<=15000 {
            let initial=s.selection.spot.is_none();s.selection.spot=Some(*p);s.selection.source="Shared Stocks feed".into();
            if initial{s.snapshot_notify.notify_one();}
        }}
        if live::apply(&mut s,market,event,now) {
            let feed=if matches!(market,Market::Stocks){&mut s.stocks}else{&mut s.options};
            feed.phase="streaming".into();
        }
    }
}
pub fn phase(parent: &SharedState,market: Market,phase: &str,detail: &str) {
    let lanes=parent.lock().unwrap().snapshot_lanes.values().cloned().collect::<Vec<_>>();
    for lane in lanes {
        let mut s=lane.lock().unwrap();
        if phase=="connecting" {if matches!(market,Market::Stocks){s.cvd_engine.gap();s.auction_engine.gap();}}
        let feed=if matches!(market,Market::Stocks){&mut s.stocks}else{&mut s.options};
        // A global streaming event is not evidence that this particular asset received data.
        if phase!="streaming" {feed.phase=phase.into();feed.detail=detail.into();}
        if matches!(market,Market::Stocks)&&phase=="waiting" {s.history_notify.notify_one();s.off_exchange_notify.notify_one();}
    }
}
pub async fn workers(key: &Credentials,parent: SharedState) {
    let lanes=parent.lock().unwrap().snapshot_lanes.values().cloned().collect::<Vec<_>>();
    let futures=lanes.into_iter().enumerate().map(|(index,lane)|async move {
        tokio::time::sleep(std::time::Duration::from_secs(index as u64*3)).await;
        let (tx,rx)=tokio::sync::mpsc::channel(10000);
        lane.lock().unwrap().off_exchange_tx=Some(tx.clone());
        tokio::join!(reference_bootstrap(lane.clone()),crate::history::worker(key,lane.clone()),crate::auction_session::worker(key,lane.clone()),
            crate::daily_atr::worker(key,lane.clone()),crate::gap_engine::worker(key,lane.clone()),
            crate::option_snapshot::worker(key,lane.clone()),crate::off_exchange::worker(key,lane,tx,rx));
    });
    futures_util::future::join_all(futures).await;
    std::future::pending::<()>().await;
}
/// A verified historical close can bootstrap REST options outside streaming hours.
/// It is a reference only: never create a tick or change the feed phase.
async fn reference_bootstrap(lane:SharedState) {
    let mut interval=tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        interval.tick().await;
        bootstrap_reference(&lane,live::now_ms());
    }
}
fn bootstrap_reference(lane:&SharedState,now:u64) {
        let mut s=lane.lock().unwrap();
        if s.selection.spot.is_some(){return}
        let reference=s.minutes.values().rev().find(|b|b.timestamp_ms.saturating_add(60_000)<=now&&b.close.is_finite()&&b.close>0.).map(|b|(b.close,b.timestamp_ms));
        if let Some((price,at))=reference {
            s.selection.spot=Some(price);
            s.selection.source=format!("Referencia histórica REST · vela {at}; no cotización en vivo");
            s.snapshot_notify.notify_one();
        }
}
/// Immutable local handoff also works when the OS blocks loopback between processes.
pub async fn file_capture(parent: SharedState) {
    let Ok(root)=crate::snapshot_bridge::root() else {std::future::pending::<()>().await;return};
    let folder=root.join("data/snapshots/inbox");
    if !enabled(){std::future::pending::<()>().await;return}
    let _=std::fs::create_dir_all(&folder);
    let mut interval=tokio::time::interval(std::time::Duration::from_secs(1));let mut last_slot=0;
    loop {
        interval.tick().await;let now=live::now_ms();let slot=now/300_000;
        if slot<=last_slot{continue}last_slot=slot;
        let state=parent.clone();let folder=folder.clone();
        let result=tokio::task::spawn_blocking(move||{
            let value=bundle(&state,live::now_ms());
            let name=format!("capture-{}-{}",value["capturedMs"],std::process::id());
            let temporary=folder.join(format!("{name}.tmp"));let target=folder.join(format!("{name}.json"));
            std::fs::write(&temporary,serde_json::to_vec(&value).map_err(std::io::Error::other)?)?;
            std::fs::rename(temporary,target)
        }).await;
        if !matches!(result,Ok(Ok(()))){last_slot=0;tokio::time::sleep(std::time::Duration::from_secs(5)).await;}
    }
}
pub fn bundle(parent: &SharedState, now: u64) -> Value {
    let lanes=parent.lock().unwrap().snapshot_lanes.clone();
    let main=live::snapshot_for_audit(parent,now);
    let mut frames=serde_json::Map::new();
    for symbol in SYMBOLS {
        let mut frame=if let Some(lane)=lanes.get(symbol){
            let mut f=live::snapshot_for_audit(lane,now);
            f.treasury=main.treasury.clone();
            serde_json::to_value(f).unwrap_or(Value::Null)
        }else if main.symbol==symbol {serde_json::to_value(&main).unwrap_or(Value::Null)}else{live::bot_frame(parent,symbol,now)};
        if let Some(lane)=lanes.get(symbol){frame["rawOptions"]=serde_json::to_value(&lane.lock().unwrap().option_snapshot).unwrap_or(Value::Null);}
        else if main.symbol==symbol{frame["rawOptions"]=serde_json::to_value(&parent.lock().unwrap().option_snapshot).unwrap_or(Value::Null);}
        frames.insert(symbol.into(),frame);
    }
    json!({"version":"snipers-snapshot-bundle-v1","capturedMs":live::now_ms().max(now),"captureStartedMs":now,"symbols":SYMBOLS,"frames":frames,
        "coverage":{"independentLanes":lanes.len(),"allTicksCertified":false,"ordersEnabled":false}})
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn historical_reference_bootstraps_without_inventing_live_tick() {
        let lane=live::new_state_for("QQQ").unwrap();
        lane.lock().unwrap().minutes.insert(60_000,live::Bar{timestamp_ms:60_000,open:100.,high:102.,low:99.,close:101.,volume:10.,vwap:None});
        bootstrap_reference(&lane,119_999);assert!(lane.lock().unwrap().selection.spot.is_none());
        bootstrap_reference(&lane,120_000);
        let s=lane.lock().unwrap();assert_eq!(s.selection.spot,Some(101.));assert!(s.selection.source.contains("histórica"));assert!(s.ticks.get("QQQ").is_none_or(|t|t.price.is_none()));
    }
    #[test] fn bundle_never_borrows_selected_price_for_another_asset(){
        let s=live::new_state_for("QQQ").unwrap();
        let b=bundle(&s,live::now_ms());
        assert_eq!(b["frames"].as_object().unwrap().len(),10);
        assert_eq!(b["frames"]["AAPL"]["symbol"],"AAPL");
        assert!(b["frames"]["AAPL"]["ticks"]["AAPL"].is_null());
        assert_eq!(b["coverage"]["ordersEnabled"],false);
    }
    #[test] fn shared_event_isolated_by_asset_and_reconnect_preserves_gap(){
        let parent=live::new_state_for("QQQ").unwrap();
        let aapl=live::new_state_for("AAPL").unwrap();let iwm=live::new_state_for("IWM").unwrap();
        parent.lock().unwrap().snapshot_lanes.insert("AAPL".into(),aapl.clone());
        parent.lock().unwrap().snapshot_lanes.insert("IWM".into(),iwm.clone());
        let now=snipers_core::calendar::legacy_eastern_ms("2026-10-14 10:00:00").unwrap();
        let event:Event=serde_json::from_value(json!({"ev":"T","sym":"AAPL","p":101.25,"s":20,"t":now,"q":1,"c":[0]})).unwrap();
        fanout(&parent,Market::Stocks,&event,now);
        assert_eq!(aapl.lock().unwrap().ticks["AAPL"].price,Some(101.25));
        assert!(iwm.lock().unwrap().ticks.is_empty());assert!(parent.lock().unwrap().ticks.is_empty());
        assert!(!aapl.lock().unwrap().bot_enabled);
        phase(&parent,Market::Stocks,"connecting","test");
        assert_eq!(aapl.lock().unwrap().stocks.phase,"connecting");
        phase(&parent,Market::Stocks,"streaming","another asset received data");
        assert_eq!(iwm.lock().unwrap().stocks.phase,"connecting");
    }
}
