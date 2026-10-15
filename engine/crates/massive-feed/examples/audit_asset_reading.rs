//! Read-only end-to-end feed audit: no broker, orders, or production persistence.
use massive_feed::{credentials::Credentials, live};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let symbol = std::env::args().nth(1).unwrap_or("QQQ".into());
    let seconds = std::env::args()
        .nth(2)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(120)
        .clamp(15, 900);
    let state = live::new_state_for(&symbol)?;
    let key = Credentials::load()?;
    let run = live::run(key, state.clone());
    tokio::pin!(run);
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
    let deadline = tokio::time::sleep(std::time::Duration::from_secs(seconds));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut run => break,
            _ = interval.tick() => report(&state),
            _ = &mut deadline => {report(&state);break;}
        }
    }
    Ok(())
}
fn report(state: &live::SharedState) {
    let s = live::snapshot(state);
    println!(
        "{}",
        serde_json::json!({
            "at":live::now_ms(),"symbol":s.symbol,"stocks":s.stocks,"options":s.options,
            "history":s.history,"chartMinutes":s.chart_history.minutes.len(),
            "snapshot":{"phase":s.option_snapshot_status.phase,"detail":s.option_snapshot_status.detail,"contracts":s.option_snapshot_status.contracts,"oiMissing":s.option_snapshot_status.oi_missing,"greeksMissing":s.option_snapshot_status.greeks_missing,"observedMs":s.option_snapshot_status.observed_ms,"exposureError":s.option_snapshot_status.exposure_error},
            "horizons":s.option_snapshot_status.horizons.iter().map(|h|serde_json::json!({"mode":h.mode,"complete":h.structure.complete,"readingAvailable":h.structure.reading.available,"reason":h.structure.reading.reason,"missing":h.structure.missing_contracts,"gexComplete":h.exposure.gex_complete,"liquidityComplete":h.structure.liquidity_complete,"mainMagnet":h.structure.selected.get("M1"),"liquidityMagnet":h.structure.magnet,"callWall":h.structure.call_wall.as_ref().map(|w|w.strike),"putWall":h.structure.put_wall.as_ref().map(|w|w.strike)})).collect::<Vec<_>>(),
            "trf":{"history":s.off_exchange.history,"error":s.off_exchange.error,"drops":s.off_exchange.queue_drops,"prints":s.off_exchange.analytics.session.print_count,"historyPrints":s.off_exchange.analytics.session.history_count,"complete":s.off_exchange.analytics.session.complete,"priorSessions":s.off_exchange.analytics.history.len(),"diagnostics":s.off_exchange.analytics.diagnostics}
        })
    );
}
