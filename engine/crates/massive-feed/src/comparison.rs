//! Offline comparison adapters. Never connects, changes production or sends orders.
use crate::{recording::Recording, telemetry::Telemetry, trade_units::stock_trade_size, Event, Market};
use serde_json::{json, Value};

fn metric(name: &str, unit: &str, baseline: f64, candidate: f64) -> Value {
    json!({"name":name,"unit":unit,"baseline":baseline,"candidate":candidate,
        "difference":candidate-baseline,"relativePercent":if baseline!=0. {Some((candidate-baseline)/baseline.abs()*100.)}else{None}})
}

pub fn compare(recording: &Recording) -> Result<Value, &'static str> {
    if recording.schema != 1 || recording.records.len() > 10_000 { return Err("Registro no admitido"); }
    let mut legacy = snipers_core::cvd::Engine::default();
    let mut decimal = snipers_core::cvd::Engine::analysis();
    let mut profile = crate::auction_live::Engine::default();
    let mut telemetry = Telemetry::default();
    let (mut now, mut first, mut selected, mut duplicates, mut invalid_quantity) = (0, None, 0, 0, 0);
    let (mut legacy_sum, mut decimal_sum, mut fractional) = (0., 0., 0);
    let mut ledger = Vec::new();
    let start = std::time::Instant::now();
    for row in &recording.records {
        if !matches!(row.market, Market::Stocks) { continue; }
        let (symbol, at) = match &row.event { Event::Trade{sym,t,..}|Event::Quote{sym,t,..} => (sym,*t), _ => continue };
        if symbol != &recording.symbol { continue; }
        selected += 1;
        first = Some(first.map_or(row.received_ms, |old:u64| old.min(row.received_ms)));
        now = now.max(row.received_ms);
        if !telemetry.observe(Market::Stocks, &row.event, row.received_ms) { duplicates += 1; continue; }
        match &row.event {
            Event::Quote{bp,ap,bs,ask_size,..} => {
                legacy.quote_with_depth(*bp,*ap,Some(*bs),Some(*ask_size),at);
                decimal.quote_with_depth(*bp,*ap,Some(*bs),Some(*ask_size),at);
            }
            Event::Trade{p,s,t,q,metadata,..} => {
                legacy.trade(symbol,*p,*s,*t,*q,&metadata.c,row.received_ms);
                if s.is_finite() && *s>0. { legacy_sum += s; }
                let shares = stock_trade_size(*s,metadata.ds.as_deref());
                if let Some(shares) = shares {
                    decimal_sum += shares;
                    fractional += usize::from(shares.fract()!=0.);
                    decimal.trade(symbol,*p,shares,*t,*q,&metadata.c,row.received_ms);
                    profile.trade(symbol,*p,shares,*t,*q,&metadata.c,row.received_ms);
                } else { invalid_quantity += 1; }
                ledger.push(json!({"at":t,"receivedMs":row.received_ms,"sequence":q,"price":p,
                    "legacySize":s,"decimalSize":metadata.ds,"normalizedShares":shares,"conditions":metadata.c}));
            }
            _ => (),
        }
    }
    if selected==0 { return Err("Sin operaciones o cotizaciones del activo"); }
    let baseline = legacy.snapshot(&recording.symbol,now,true);
    let candidate = decimal.snapshot(&recording.symbol,now,true);
    let amt = profile.snapshot(&recording.symbol,now,true,vec![]);
    let mut metrics = vec![metric("input_quantity_sum","shares",legacy_sum,decimal_sum)];
    for (period,a,b) in [("premarket",&baseline.premarket,&candidate.premarket),("regular",&baseline.regular,&candidate.regular)] {
        for (name,x,y) in [("buy",a.buy,b.buy),("sell",a.sell,b.sell),("unknown",a.unknown,b.unknown),("delta",a.delta,b.delta),("volume",a.buy+a.sell+a.unknown,b.buy+b.sell+b.unknown)] {
            metrics.push(metric(&format!("cvd_{period}_{name}"),"shares",x,y));
        }
    }
    Ok(json!({"schemaVersion":1,"symbol":recording.symbol,"scope":"Same retained recording replayed through CVD adapters; not the complete desktop pipeline or LIVE/REST reconciliation",
        "versions":{"baseline":"historical-cvd-legacy-s-reference","candidate":snipers_core::stock_volume_policy::ID},
        "window":{"firstReceivedMs":first,"lastReceivedMs":now},
        "coverage":{"recordedEvents":recording.records.len(),"selectedStockEvents":selected,"telemetryRejectedEvents":duplicates,
            "invalidDecimalQuantity":invalid_quantity,"fractionalTrades":fractional,"overwrittenRecords":recording.overwritten_records,
            "partial":true,"sessionCompletenessCertified":false},
        "metrics":metrics,"baselineCvd":baseline,"candidateCvd":candidate,
        "amtControl":{"quantityPolicy":"Decimal quantity; shared consolidated-volume candidate; archived legacy AMT remains a separate reference", "reading":amt},
        "tradeLedger":ledger,"replayElapsedMs":start.elapsed().as_secs_f64()*1000.,
        "recommendation":"Review differences; no automatic promotion or replacement",
        "notCompared":["candles","options","structural_routes","bots","UI","whole_session_backfill"]}))
}

#[cfg(test)] mod tests {
 use super::*;
 use crate::telemetry::Record;
 fn recording(decimal: Option<&str>) -> Recording {
  let t=snipers_core::calendar::legacy_eastern_ms("2026-10-12 10:00:00").unwrap();
  let quote=Event::Quote{sym:"QQQ".into(),bp:99.99,ap:100.,bs:10.,ask_size:10.,t};
  let trade:Event=serde_json::from_value(json!({"ev":"T","sym":"QQQ","p":100.,"s":1,"ds":decimal,"t":t+1,"q":1,"c":[0]})).unwrap();
  Recording{schema:1,symbol:"QQQ".into(),overwritten_records:0,records:vec![Record{market:Market::Stocks,received_ms:t,event:quote},Record{market:Market::Stocks,received_ms:t+1,event:trade.clone()},Record{market:Market::Stocks,received_ms:t+1,event:trade}]}
 }
 #[test] fn decimal_difference_is_measured_and_duplicate_does_not_inflate() {
  let r=compare(&recording(Some("1.25"))).unwrap();
  assert_eq!(r["baselineCvd"]["regular"]["buy"],1.);
  assert_eq!(r["candidateCvd"]["regular"]["buy"],1.25);
  assert_eq!(r["amtControl"]["reading"]["totalVolume"],1.25);
  assert_eq!(r["tradeLedger"].as_array().unwrap().len(),1);
  assert_eq!(r["coverage"]["sessionCompletenessCertified"],false);
 }
 #[test] fn unchanged_integer_inputs_have_identical_cvd_results() {
  let r=compare(&recording(None)).unwrap();
  for m in r["metrics"].as_array().unwrap(){assert_eq!(m["difference"],0.);}
 }
 #[test] fn invalid_decimal_is_rejected_without_fabricating_a_candidate_volume() {
  let r=compare(&recording(Some("bad"))).unwrap();
  assert_eq!(r["coverage"]["invalidDecimalQuantity"],1);
  assert_eq!(r["candidateCvd"]["regular"]["buy"],0.);
 }
}
