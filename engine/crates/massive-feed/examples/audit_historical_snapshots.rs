//! Offline replay of original snapshots. No feed connection, credentials or broker.
use serde_json::{json, Value};
use std::{collections::BTreeMap, io::{BufRead, BufReader}, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory=PathBuf::from(std::env::args().nth(1).ok_or("snapshot directory required")?);
    let mut files=std::fs::read_dir(directory)?.filter_map(Result::ok).map(|e|e.path())
        .filter(|p|p.extension().is_some_and(|e|e=="jsonl")).collect::<Vec<_>>();files.sort();
    let mut counts:BTreeMap<String,u64>=BTreeMap::new();
    let mut blockers:BTreeMap<String,u64>=BTreeMap::new();
    let mut states:BTreeMap<String,u64>=BTreeMap::new();
    let mut last:BTreeMap<String,u64>=BTreeMap::new();
    let mut charts:BTreeMap<String,massive_feed::bot_chart_route::Tracker>=BTreeMap::new();
    let mut rebounds:BTreeMap<String,massive_feed::bot_ema9_rebound::Tracker>=BTreeMap::new();
    let mut errors=Vec::new();
    for file in &files {for (line_no,line) in BufReader::new(std::fs::File::open(file)?).lines().enumerate() {
        let row:Value=match serde_json::from_str(&line?){Ok(r)=>r,Err(e)=>{errors.push(format!("{}:{} {e}",file.display(),line_no+1));continue}};
        let symbol=row["symbol"].as_str().unwrap_or("");let now=row["at"].as_u64().unwrap_or(0);
        *counts.entry("rows".into()).or_default()+=1;
        if now==0||symbol.is_empty()||row["frame"]["symbol"]!=symbol {
            *counts.entry("invalidIdentity".into()).or_default()+=1;continue;
        }
        if last.get(symbol).is_some_and(|t|*t>now){*counts.entry("outOfOrder".into()).or_default()+=1;continue;}
        last.insert(symbol.into(),now);
        // Independent snapshot replay: missing intermediate feed events are never fabricated.
        let frame=&row["frame"];
        let mut r=massive_feed::intraday_strategy::read_paper(frame,now);
        let context=massive_feed::smc::read(frame,symbol,now);
        let state=context["state"].as_str().unwrap_or("unknown").to_owned();
        *states.entry(state).or_default()+=1;
        let eligible=r.eligible;let target=r.next;let management=r.management_ready;
        massive_feed::smc_bot::apply(context,&mut r,now);
        *counts.entry("replayed".into()).or_default()+=1;
        *counts.entry("baselineEligible".into()).or_default()+=u64::from(eligible);
        *counts.entry("finalEligible".into()).or_default()+=u64::from(r.eligible);
        *counts.entry("smcBlocked".into()).or_default()+=u64::from(eligible&&!r.eligible);
        *counts.entry("partial".into()).or_default()+=u64::from(r.paper_partial);
        *counts.entry("emaAvailable".into()).or_default()+=u64::from(r.ema.available);
        if (!eligible&&r.eligible)||target!=r.next||management!=r.management_ready {
            errors.push(format!("SMC safety invariant changed: {symbol} {now}"));
        }
        charts.entry(symbol.into()).or_default().apply(frame,&mut r,now);
        rebounds.entry(symbol.into()).or_default().apply(frame,&mut r,now);
        *counts.entry("afterChartEmaEligible".into()).or_default()+=u64::from(r.eligible);
        *counts.entry("chartConfirmed".into()).or_default()+=u64::from(r.quick_context["chartRoute"]["confirmed"]==true);
        *counts.entry("ema9Confirmed".into()).or_default()+=u64::from(r.quick_context["ema9Rebound"]["confirmed"]==true);
        for b in r.blockers {*blockers.entry(b).or_default()+=1;}
    }}
    println!("{}",serde_json::to_string_pretty(&json!({"version":"historical-snapshot-replay-v1","files":files.len(),
        "counts":counts,"smcStates":states,"blockers":blockers,"errors":errors,
        "executionEnabled":false,"validated":false,
        "scope":"Original snapshots at original timestamps; current pure intraday, SMC, chart-route and EMA9 trackers. Trackers see only saved captures, not missing intermediate ticks. No buffer history, external route worker or broker replay invented. Engine eligibility is not an executed trade or a win rate."}))?);
    Ok(())
}
