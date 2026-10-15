//! Offline replay: levels supplied by the user are not certified historical snapshots.
use snipers_core::magnet_reclaim::{self,Bar};
fn main(){
    let path=std::env::args().nth(1).expect("minute-bar JSON path");
    let data:serde_json::Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let open=snipers_core::calendar::rfc3339_ms("2026-09-29T13:30:00Z").unwrap();
    let rows=data["results"].as_array().unwrap();let mut bars=Vec::new();let mut results=Vec::new();
    for pair in rows.windows(2){
        let a=&pair[0];let b=&pair[1];let t=a["t"].as_u64().unwrap();
        if t<open||t>open+75*60000||(t-open)%120000!=0||b["t"].as_u64()!=Some(t+60000){continue;}
        bars.push(Bar{start_ms:t,open:a["o"].as_f64().unwrap(),close:b["c"].as_f64().unwrap(),
            low:a["l"].as_f64().unwrap().min(b["l"].as_f64().unwrap()),high:a["h"].as_f64().unwrap().max(b["h"].as_f64().unwrap())});
        let r=magnet_reclaim::evaluate(&bars,Some(737.),Some(740.),t+120000);
        if r.detected {results.push(r);}
    }
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"symbol":"QQQ","session":"2026-09-29",
        "levels_source":"USER_RECOLLECTION_NOT_HISTORICAL_SNAPSHOT","magnet":737,"call_wall":740,
        "scope":"PRICE_PATTERN_ONLY_NOT_FULL_BOT_BACKTEST","observations":results})).unwrap());
}
