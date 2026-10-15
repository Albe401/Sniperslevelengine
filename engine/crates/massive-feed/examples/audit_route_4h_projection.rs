//! Offline chronological audit of empirical library; no data download or orders.
use massive_feed::route_4h_projection::{project, Sample};
fn main() {
    let mut output=Vec::new();
    for symbol in ["SPY","QQQ"] {
        let raw=std::fs::read(format!("docs/route-4h-study/{symbol}-4h-study.json")).unwrap();
        let report:serde_json::Value=serde_json::from_slice(&raw).unwrap();
        let samples:Vec<Sample>=serde_json::from_value(report["projectionSamples"].clone()).unwrap();
        let began=std::time::Instant::now();let mut max_n=0;let mut estimates=0;
        for row in &samples {
            let p=project(&row.scenario,&samples,&row.horizon);
            assert!(p.latest_deadline.is_none_or(|t|t<row.scenario.at));
            assert!(!p.probability_claim);
            max_n=max_n.max(p.n);estimates+=usize::from(p.typical.is_some());
            let mut earlier=samples.clone();earlier.retain(|r|r.deadline<row.scenario.at);
            assert_eq!(serde_json::to_value(&p).unwrap(),serde_json::to_value(project(&row.scenario,&earlier,&row.horizon)).unwrap());
        }
        output.push(serde_json::json!({"symbol":symbol,"samples":samples.len(),"maxEarlierComparableCases":max_n,"historicalEstimates":estimates,"auditMs":began.elapsed().as_millis(),"futureLeakage":false}));
    }
    let raw=serde_json::to_string_pretty(&output).unwrap();
    std::fs::write("docs/route-4h-study/projection-audit.json",&raw).unwrap();println!("{raw}");
}
