//! Retrospective event-time simulation. Never produces prospective capture evidence.
use massive_feed::{
    live::now_ms,
    option_activity_dataset::{self, Dataset},
    option_activity_history::Report,
    option_premium_dataset,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use snipers_core::{
    option_activity_baseline::{self, Sample},
    option_activity_detector, option_premium_detector,
};
use std::{collections::BTreeMap, path::Path};
fn event_key(e: &(u64, usize, usize, usize)) -> (u64, usize, usize, usize) {
    (
        e.0,
        match e.1 {
            1 => 0,
            2 => 1,
            _ => 2,
        },
        e.2,
        e.3,
    )
}
fn read<T: DeserializeOwned>(p: &Path) -> Result<T, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}
fn retrospective_reference(
    samples: &[Sample],
    cutoff: u64,
    actual_now: u64,
) -> Result<option_activity_baseline::Baseline, &'static str> {
    let prior: Vec<_> = samples
        .iter()
        .filter(|s| s.end_ms < cutoff)
        .cloned()
        .collect();
    let mut b = option_activity_baseline::build(&prior, cutoff, 20, actual_now)?;
    // Synthetic availability only in this isolated diagnostic; actual provenance stays in the report.
    b.built_ms = cutoff - 1;
    b.available_before_cutoff = true;
    Ok(b)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    if a.len() != 4 {
        return Err("Usage: replay_option_session PREMIUM_DATASET ACCUMULATOR_SNAPSHOT VOLUME_DATASET_DIRECTORY OUTPUT_DIRECTORY".into());
    }
    let now = now_ms();
    let d: Dataset = read(Path::new(&a[0]))?;
    let snapshot: Value = read(Path::new(&a[1]))?;
    if snapshot["version"] != "option-premium-reference-accumulator-v1"
        || snapshot["baseline"].is_null()
        || snapshot["sourceErrors"]
            .as_array()
            .is_none_or(|e| !e.is_empty())
    {
        return Err("Invalid source snapshot".into());
    }
    let mut reports = vec![];
    for p in &d.reports {
        let r: Report = read(Path::new(p))?;
        if r.request.date != d.date
            || r.request.underlying != d.underlying
            || r.request.window_minutes != 5
            || option_premium_dataset::certify(&r)
                .certified_premium
                .is_none()
        {
            return Err("Uncertified target report".into());
        }
        reports.push(r);
    }
    if reports.len() != 6 {
        return Err("Expected six selected contracts".into());
    }
    let open = reports[0].session.open_ms;
    let close = reports[0].session.close_ms;
    let stock: Value = read(Path::new(&d.context_reports[0]))?;
    let mut bars = vec![];
    for v in stock["bars"].as_array().ok_or("Missing stock bars")? {
        bars.push(massive_feed::live::Bar {
            timestamp_ms: v["timestampMs"].as_u64().ok_or("Missing timestamp")?,
            open: v["open"].as_f64().ok_or("Missing open")?,
            high: v["high"].as_f64().ok_or("Missing high")?,
            low: v["low"].as_f64().ok_or("Missing low")?,
            close: v["close"].as_f64().ok_or("Missing close")?,
            volume: v["volume"].as_f64().ok_or("Missing volume")?,
            vwap: None,
        });
    }
    bars.sort_by_key(|b| b.timestamp_ms);
    let mut prior = vec![];
    for r in snapshot["sourceRecords"]
        .as_array()
        .ok_or("Missing sources")?
    {
        for s in r["certifiedSamples"].as_array().ok_or("Missing samples")? {
            prior.push(serde_json::from_value::<Sample>(s.clone())?);
        }
    }
    let premium_reference = retrospective_reference(&prior, open, now)?;
    let mut volume_history = vec![];
    let mut volume_target = None;
    for entry in std::fs::read_dir(&a[2])? {
        let p = entry?.path();
        if p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("dataset-") && n.ends_with(".json"))
        {
            let v: Dataset = read(&p)?;
            if v.date == d.date {
                volume_target = Some(v);
            } else {
                volume_history.extend(v.samples);
            }
        }
    }
    let volume_reference = retrospective_reference(&volume_history, open, now)?;
    let volume_target = volume_target.ok_or("Missing target volume dataset")?;
    let mut events = vec![];
    for (i, r) in reports.iter().enumerate() {
        for (j, t) in r.trades.iter().enumerate() {
            events.push((t.sip_ns, 0usize, i, j));
        }
    }
    for (j, b) in bars.iter().enumerate() {
        events.push(((b.timestamp_ms + 60000) * 1_000_000, 1, 0, j));
    }
    // Close windows before processing trades exactly at their end (half-open windows).
    for end in (open + 300000..=close).step_by(300000) {
        events.push((end * 1_000_000, 2, 0, 0));
    }
    events.sort_by_key(event_key);
    let expected: Vec<_> = reports
        .iter()
        .map(|r| option_premium_dataset::samples(r, &bars, open + 60000).1)
        .collect();
    let mut sums: BTreeMap<(usize, u64), (usize, f64, f64)> = BTreeMap::new();
    let mut seen_bars = vec![];
    let mut readings = vec![];
    let mut outcomes = vec![];
    let mut pending_labels: Vec<(u64, u64, String, u64, f64)> = vec![];
    let mut trade_count = 0;
    let mut mismatches = 0;
    for (clock, kind, i, j) in &events {
        let ms = clock / 1_000_000;
        if *kind == 0 {
            let t = &reports[*i].trades[*j];
            let start = open + (ms - open) / 300000 * 300000;
            let sum = sums.entry((*i, start)).or_default();
            sum.0 += 1;
            sum.1 += t.contracts;
            sum.2 += t.price * t.contracts * 100.;
            trade_count += 1;
        } else if *kind == 1 {
            seen_bars.push(&bars[*j]);
            pending_labels.retain(|(due,end,ticker,horizon,anchor)| {
                if *due==ms {outcomes.push(json!({"availableMs":ms,"windowEndMs":end,"ticker":ticker,"horizonMinutes":horizon,"returnPct":(bars[*j].close/anchor-1.)*100.}));false} else {true}
            });
        } else {
            let start = ms - 300000;
            for (ri, r) in reports.iter().enumerate() {
                let total = sums.get(&(ri, start)).copied().unwrap_or_default();
                let w = r
                    .windows
                    .iter()
                    .find(|w| w.start_ms == start)
                    .ok_or("Missing archived window")?;
                if total.0 != w.trades
                    || (total.1 - w.observed_contracts).abs() > 1e-7
                    || (total.2 - w.observed_premium).abs() > 1e-6 + 1e-10 * total.2.abs()
                {
                    mismatches += 1;
                }
                if let Some(s) = expected[ri].iter().find(|s| s.start_ms == start) {
                    if !seen_bars.iter().any(|b| {
                        b.timestamp_ms + 60000 == s.price_available_ms
                            && b.close == s.underlying_price
                    }) {
                        return Err("Future or missing price context".into());
                    }
                    let mut simulated = s.clone();
                    simulated.source_collected_ms = ms;
                    simulated.contracts = total.1;
                    simulated.premium = Some(total.2);
                    let premium =
                        option_premium_detector::evaluate(&premium_reference, &simulated, ms);
                    let volume = volume_target
                        .samples
                        .iter()
                        .find(|v| v.ticker == s.ticker && v.start_ms == start)
                        .map(|v| {
                            let mut v = v.clone();
                            v.source_collected_ms = ms;
                            option_activity_detector::evaluate(&volume_reference, &v, ms)
                        });
                    readings.push(json!({"availableMs":ms,"startMs":start,"ticker":s.ticker,"trades":total.0,"contracts":total.1,"grossPremium":total.2,"premiumReading":premium,"volumeReading":volume}));
                    if let Some(anchor) = seen_bars.iter().find(|b| b.timestamp_ms + 60000 == ms) {
                        for h in [5, 15, 30] {
                            pending_labels.push((
                                ms + h * 60000,
                                ms,
                                s.ticker.clone(),
                                h,
                                anchor.close,
                            ));
                        }
                    }
                }
            }
        }
    }
    if mismatches > 0 {
        return Err(format!("{mismatches} archive mismatches").into());
    }
    let mut statuses = BTreeMap::<String, usize>::new();
    for r in &readings {
        for engine in ["premiumReading", "volumeReading"] {
            let key = format!(
                "{engine}:{}:{}",
                r[engine]["status"].as_str().unwrap_or("MISSING"),
                r[engine]["reason"].as_str().unwrap_or("")
            );
            *statuses.entry(key).or_default() += 1;
        }
    }
    let result = json!({"version":"option-session-retrospective-replay-v1","status":"REPLAY_VERIFIED","date":d.date,"builtMs":now,"sourceDataset":a[0],"sourceSnapshot":a[1],"retrospective":true,"prospectiveEligible":false,"simulatedReferenceAvailability":true,"simulatedSourceAvailability":true,"actualArrivalLatencyKnown":false,"eventOrdering":"SIP trades; stock bars at minute close; half-open windows","scope":"six selected QQQ options, reported trades and closed underlying minutes; volume uses archived eligible-option-bar samples","summary":{"tradesReplayed":trade_count,"stockMinutesReplayed":bars.len(),"eventsProcessed":events.len(),"readings":readings.len(),"windowReconciliationErrors":mismatches,"priorPremiumDates":premium_reference.cohorts.iter().flat_map(|c|c.dates.clone()).collect::<std::collections::BTreeSet<_>>(),"statuses":statuses,"outcomesAvailable":outcomes.len(),"outcomesPendingAtClose":pending_labels.len()},"readings":readings,"outcomes":outcomes,"primaryHorizonMinutes":15,"limitations":["Historical retrieval cannot reproduce network latency, late arrivals or intraday corrections","Only the archived six-contract universe is available","Reference availability and historical collection clocks are explicitly simulated in isolated copies","Volume bars are evaluated at window close, not reconstructed as per-trade volume","No prospective captures or trial units are created"],"formulaChanged":false,"executionEnabled":false,"ordersSent":0});
    let file =
        option_activity_dataset::save_json(Path::new(&a[3]), "retrospective-replay", &result)?;
    println!("{}", json!({"file":file,"summary":result["summary"]}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundary_trade_is_processed_after_the_closing_window() {
        let mut events = vec![
            (300, 0, 0, 0),
            (299, 0, 0, 0),
            (300, 2, 0, 0),
            (300, 1, 0, 0),
        ];
        events.sort_by_key(event_key);
        assert_eq!(
            events,
            vec![
                (299, 0, 0, 0),
                (300, 1, 0, 0),
                (300, 2, 0, 0),
                (300, 0, 0, 0)
            ]
        );
    }
    #[test]
    fn target_session_is_excluded_from_retrospective_reference() {
        let cutoff = snipers_core::calendar::legacy_eastern_ms("2026-10-09 09:30:00").unwrap();
        let sample = Sample {
            underlying: "QQQ".into(),
            ticker: "O:QQQ261009C00752000".into(),
            kind: "call".into(),
            expiration: "2026-10-09".into(),
            strike: 752.,
            start_ms: cutoff + 300000,
            end_ms: cutoff + 600000,
            underlying_price: 752.,
            price_available_ms: cutoff + 300000,
            contracts: 99999.,
            premium: Some(999999.),
            measurement: "reported-trades".into(),
            source_collected_ms: cutoff + 600000,
            quality_complete: true,
        };
        let b = retrospective_reference(&[sample], cutoff, cutoff + 86400000).unwrap();
        assert_eq!(b.accepted_samples, 0);
        assert!(b.cohorts.is_empty());
    }
}
