//! Deterministic synthetic replay and CPU benchmark. No network, credentials or orders.
use serde_json::json;
use snipers_core::{cvd, execution_evidence};
use std::{hint::black_box, time::Instant};
const BASE: u64 = 1_790_949_600_000; // 2026-10-02 10:00 ET, open session.
fn fixture(side: i32, depth: bool) -> cvd::Engine {
    let mut e = cvd::Engine::default();
    for i in 0..31 {
        let at = BASE + i * 1000;
        e.quote_with_depth(
            100.,
            100.02,
            depth.then_some(100.),
            depth.then_some(100.),
            at,
        );
        e.trade(
            "QQQ",
            if side == 1 { 100.02 } else { 100. },
            if i >= 21 { 2000. } else { 100. },
            at,
            Some(i + 1),
            &[0],
            at,
        );
    }
    e
}
fn ui_frame(r: cvd::Read) -> serde_json::Value {
    let mut v = serde_json::to_value(r).unwrap();
    let e = v["executionEvidence"].as_object_mut().unwrap();
    e.remove("recentTrades");
    e.remove("recentQuotes");
    for w in e.get_mut("windows").unwrap().as_array_mut().unwrap() {
        w.as_object_mut().unwrap().remove("zones");
    }
    v
}
fn main() {
    let now = BASE + 30_000;
    let buyer = fixture(-1, true);
    let seller = fixture(1, true);
    assert_eq!(
        buyer
            .snapshot("QQQ", now, true)
            .execution_evidence
            .absorption
            .candidates[0]
            .absorbing_side,
        "buy"
    );
    assert_eq!(
        seller
            .snapshot("QQQ", now, true)
            .execution_evidence
            .absorption
            .candidates[0]
            .absorbing_side,
        "sell"
    );
    let mut breakthrough = fixture(1, true);
    breakthrough.quote_with_depth(100.2, 100.22, Some(100.), Some(100.), now);
    let invalidated = breakthrough.snapshot("QQQ", now, true);
    assert!(invalidated
        .execution_evidence
        .absorption
        .candidates
        .is_empty());
    let disconnected = buyer.snapshot("QQQ", now, false);
    assert!(disconnected
        .execution_evidence
        .absorption
        .candidates
        .is_empty());
    let no_depth = fixture(-1, false).snapshot("QQQ", now, true);
    assert!(no_depth.execution_evidence.absorption.candidates.is_empty());
    // Full bounded buffers with high local activity and many price buckets.
    let mut stress = execution_evidence::Engine::default();
    for i in 0..10_000 {
        let at = BASE + i * 4;
        let price = 100. + (i % 100) as f64 * 0.01;
        stress.quote(price - 0.02, price, Some(100.), Some(100.), at);
        stress.trade(
            at,
            i + 1,
            price,
            if i >= 7500 { 100000. } else { 100. },
            1,
            &[0],
            true,
        );
    }
    let mut samples = Vec::new();
    for _ in 0..120 {
        let t = Instant::now();
        black_box(stress.snapshot(BASE + 39_996, true));
        samples.push(t.elapsed().as_secs_f64() * 1000.);
    }
    samples.sort_by(f64::total_cmp);
    println!("{}",serde_json::to_string_pretty(&json!({"basis":"synthetic replay; not real-market profitability validation","now":now,"buyer":ui_frame(buyer.snapshot("QQQ",now,true)),"seller":ui_frame(seller.snapshot("QQQ",now,true)),"breakthrough":ui_frame(invalidated),"disconnected":ui_frame(disconnected),"noDepth":ui_frame(no_depth),"benchmark":{"events":10000,"samples":120,"medianMs":samples[60],"p95Ms":samples[114],"maxMs":samples[119]}})).unwrap());
}
