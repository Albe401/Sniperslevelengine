use super::*;
use crate::smc_patterns::Bar;
fn bars() -> Vec<Bar> {
    let open = calendar::rfc3339_ms("2026-10-06T09:30:00-04:00").unwrap();
    let cycle = [100., 101., 102., 101., 100., 99., 98., 99.];
    (0..32)
        .map(|i| {
            let close = cycle[i % 8];
            let previous = if i == 0 { 100. } else { cycle[(i - 1) % 8] };
            Bar {
                start: open + i as u64 * 120000,
                end: open + (i as u64 + 1) * 120000,
                open: previous,
                high: close.max(previous) + 0.4,
                low: close.min(previous) - 0.4,
                close,
                volume: 100.,
            }
        })
        .collect()
}
fn push(b: &mut Vec<Bar>, o: f64, h: f64, l: f64, c: f64, v: f64) {
    let start = b.last().unwrap().end;
    b.push(Bar {
        start,
        end: start + 120000,
        open: o,
        high: h,
        low: l,
        close: c,
        volume: v,
    });
}
fn fixture() -> (Value, u64) {
    let b = bars();
    let now = b.last().unwrap().end;
    let minutes:Vec<_>=b.iter().flat_map(|b|(0..2).map(move|i|json!({"timestampMs":b.start+i*60000,"open":b.open,"high":b.high,"low":b.low,"close":b.close,"volume":b.volume/2.}))).collect();
    (
        json!({"symbol":"QQQ","publishedMs":now,"stocks":{"phase":"streaming"},"ticks":{"QQQ":{"symbol":"QQQ","price":99.,"tradeMs":now}},"chartHistory":{"minutes":minutes}}),
        now,
    )
}
#[test]
fn pivots_are_known_only_after_two_right_hand_closes() {
    let b = bars();
    let result = smc_patterns::analyze("QQQ", 2, &b, &[]);
    assert!(!result.pivots.is_empty());
    for p in &result.pivots {
        assert_eq!(p.available_at, p.at + 3 * 120000);
        let prefix: Vec<_> = b
            .iter()
            .filter(|b| b.end < p.available_at)
            .cloned()
            .collect();
        assert!(!smc_patterns::analyze("QQQ", 2, &prefix, &[])
            .pivots
            .iter()
            .any(|q| q.at == p.at && q.kind == p.kind));
    }
}
#[test]
fn displacement_waits_for_a_later_retest_and_invalidates() {
    let mut b = bars();
    push(&mut b, 99., 115.5, 98.8, 115., 400.);
    let layer = smc_patterns::analyze("QQQ", 2, &b, &[]);
    let s = layer.setups.last().expect("displaced BOS");
    assert_eq!(s.kind, "CONTINUATION");
    assert_eq!(s.state, "WAIT_RETEST");
    let lower = s.lower;
    let upper = s.upper;
    let level = s.level;
    let invalid = s.invalidation;
    assert!(layer.events.last().unwrap().displacement);
    push(
        &mut b,
        lower + 0.2,
        level + 2.,
        lower + 0.1,
        level + 1.,
        150.,
    );
    let layer = smc_patterns::analyze("QQQ", 2, &b, &[]);
    assert_eq!(layer.setups[0].state, "CONFIRMED");
    assert!(layer.setups[0].retest_at.unwrap() > layer.setups[0].break_at);
    assert!(upper >= lower);
    push(
        &mut b,
        invalid,
        invalid + 0.1,
        invalid - 2.,
        invalid - 1.,
        150.,
    );
    assert_eq!(
        smc_patterns::analyze("QQQ", 2, &b, &[]).setups[0].state,
        "INVALIDATED"
    );
}
#[test]
fn call_put_geometry_is_symmetric() {
    let mut b = bars();
    push(&mut b, 99., 115.5, 98.8, 115., 400.);
    let mirror: Vec<_> = b
        .iter()
        .map(|b| Bar {
            open: 220. - b.open,
            high: 220. - b.low,
            low: 220. - b.high,
            close: 220. - b.close,
            ..b.clone()
        })
        .collect();
    let a = smc_patterns::analyze("QQQ", 2, &b, &[]);
    let z = smc_patterns::analyze("QQQ", 2, &mirror, &[]);
    assert_eq!(a.events.last().unwrap().kind, z.events.last().unwrap().kind);
    assert_eq!(
        a.events.last().unwrap().side,
        -z.events.last().unwrap().side
    );
    assert_eq!(
        a.setups.last().unwrap().state,
        z.setups.last().unwrap().state
    );
}
#[test]
fn wick_only_break_and_low_volume_do_not_confirm_displacement() {
    for volume in [100., 400.] {
        let mut b = bars();
        push(
            &mut b,
            99.,
            115.5,
            98.8,
            if volume == 400. { 100. } else { 115. },
            volume,
        );
        assert!(smc_patterns::analyze("QQQ", 2, &b, &[]).setups.is_empty());
    }
}
#[test]
fn sweep_requires_return_and_available_reference() {
    let mut b = bars();
    let at = b.last().unwrap().end;
    let reference = Reference {
        kind: "previous-day-high".into(),
        price: 110.,
        side: -1,
        available_at: at,
    };
    push(&mut b, 109., 112., 108., 109., 150.);
    let r = smc_patterns::analyze("QQQ", 2, &b, &[reference.clone()]);
    assert!(r.events.iter().any(|e| e.kind == "SWEEP" && e.side == -1));
    let future = Reference {
        available_at: at + 120000,
        ..reference.clone()
    };
    assert!(!smc_patterns::analyze("QQQ", 2, &b, &[future])
        .events
        .iter()
        .any(|e| e.kind == "SWEEP" && e.level == 110.));
    b.last_mut().unwrap().close = 111.;
    assert!(!smc_patterns::analyze("QQQ", 2, &b, &[reference])
        .events
        .iter()
        .any(|e| e.kind == "SWEEP" && e.level == 110.));
}
#[test]
fn reversal_needs_a_prior_matching_sweep() {
    let mut b = bars();
    push(&mut b, 99., 115.5, 98.8, 115., 400.);
    let at = b.last().unwrap().end;
    let reference = Reference {
        kind: "external-high".into(),
        price: 117.,
        side: -1,
        available_at: at,
    };
    push(&mut b, 116., 119., 114., 115., 120.);
    push(&mut b, 115., 115.2, 89.5, 90., 500.);
    let a = smc_patterns::analyze("QQQ", 2, &b, &[reference]);
    assert!(a.events.iter().any(|e| e.kind == "CHOCH" && e.side == -1));
    assert!(a
        .setups
        .iter()
        .any(|s| s.kind == "REVERSAL" && s.sweep_at.is_some()));
    let z = smc_patterns::analyze("QQQ", 2, &b, &[]);
    assert!(!z.setups.iter().any(|s| s.kind == "REVERSAL"));
}
#[test]
fn missing_minutes_and_duplicate_bars_do_not_bridge_sequences() {
    let (mut s, now) = fixture();
    let minutes = s["chartHistory"]["minutes"].as_array_mut().unwrap();
    let duplicate = minutes.last().unwrap().clone();
    minutes.push(duplicate);
    let r = read(&s, "QQQ", now);
    assert_eq!(r["state"], "pending");
    let (mut s, now) = fixture();
    s["chartHistory"]["minutes"]
        .as_array_mut()
        .unwrap()
        .remove(10);
    let r = read(&s, "QQQ", now);
    assert!(!r["layers"][0]["pivots"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["at"].as_u64().unwrap() == now - 29 * 120000));
}
#[test]
fn reconnect_stale_foreign_future_or_invalid_ohlc_stay_pending() {
    let (s, now) = fixture();
    for kind in ["stale", "future", "foreign", "disconnect", "invalid"] {
        let mut bad = s.clone();
        match kind {
            "stale" => bad["publishedMs"] = json!(now - 3001),
            "future" => bad["ticks"]["QQQ"]["tradeMs"] = json!(now + 1),
            "foreign" => bad["symbol"] = json!("SPY"),
            "disconnect" => bad["stocks"]["phase"] = json!("closed"),
            _ => {
                let n = bad["chartHistory"]["minutes"].as_array().unwrap().len();
                bad["chartHistory"]["minutes"][n - 1]["low"] = json!(10000.);
            }
        }
        assert_eq!(read(&bad, "QQQ", now)["state"], "pending", "{kind}");
    }
}
#[test]
fn unfinished_minutes_do_not_rewrite_closed_evidence() {
    let (mut s, now) = fixture();
    let initial = read(&s, "QQQ", now);
    s["chartHistory"]["minutes"].as_array_mut().unwrap().push(
        json!({"timestampMs":now,"open":1.,"high":1000.,"low":1.,"close":900.,"volume":10000.}),
    );
    assert_eq!(initial["layers"], read(&s, "QQQ", now)["layers"]);
}
#[test]
fn advisor_never_changes_existing_eligibility_risk_or_positions() {
    let (s, now) = fixture();
    let c = read(&s, "QQQ", now);
    let r = json!({"symbol":"QQQ","at":now,"eligible":false,"entry_side":"CALL","spot":99.,"next":110.,"stop":90.,"position":{"side":"CALL"}});
    let before = r.clone();
    let out = advisory(&c, &r, Some(95.), Some("CALL"), now);
    assert_eq!(r, before);
    assert_eq!(out["baselineEligible"], false);
    assert_eq!(out["executionEnabled"], false);
    assert_eq!(out["extraDirectionalVotes"], 0);
    assert_eq!(out["exitReviewSuggested"], false);
}
#[test]
fn observer_remains_separate_from_live_strategy_core() {
    let strategy = include_str!("intraday_strategy.rs");
    let live = include_str!("live.rs");
    
    for s in [strategy, live] {
        assert!(!s.contains("smc::read("));
        assert!(!s.contains("smc::advisory("));
    }
}
#[test]
fn shadow_records_transitions_once_and_never_labels_them_trade_results() {
    let (mut s, now) = fixture();
    for i in 0..2 {
        s["chartHistory"]["minutes"].as_array_mut().unwrap().push(json!({"timestampMs":now+i*60000,"open":99.,"high":115.5,"low":98.8,"close":115.,"volume":200.}));
    }
    let now = now + 120000;
    s["publishedMs"] = json!(now);
    s["ticks"]["QQQ"]["tradeMs"] = json!(now);
    s["ticks"]["QQQ"]["price"] = json!(115.);
    let baseline = json!({"symbol":"QQQ","at":now,"eligible":false,"entry_side":"CALL","entry_basis":"directional"});
    let mut observer = crate::smc_observer::Observer::default();
    let first = observer.observe(&s, &baseline, None, None, now);
    assert!(!first["newRecords"].as_array().unwrap().is_empty());
    let second = observer.observe(&s, &baseline, None, None, now);
    assert!(second["newRecords"].as_array().unwrap().is_empty());
    assert_eq!(first["retained"], second["retained"]);
    assert!(observer.summarize()["profitability"].is_null());
    assert!(!observer.records().is_empty());
    assert_eq!(
        observer.observe(&s, &baseline, None, None, now - 1)["state"],
        "reordered"
    );
}
#[test]
fn advisor_does_not_use_stale_or_foreign_confirmations() {
    let (s, now) = fixture();
    let mut context = read(&s, "QQQ", now);
    context["layers"][0]["setups"] = json!([{"side":1,"state":"CONFIRMED","retestAt":now}]);
    let r = json!({"symbol":"QQQ","at":now,"eligible":false,"entry_side":"CALL","flow_one":0.2,"flow_five":0.1,"quick_context":{"symbol":"QQQ","at":now,"cvd":{"status":"confirms"}}});
    let review = advisory(&context, &r, None, None, now);
    assert_eq!(review["entryAssessment"], "supports");
    assert_eq!(review["baselineEligible"], false);
    assert_eq!(review["confirmations"]["flow"], "supports");
    context["symbol"] = json!("SPY");
    assert_eq!(
        advisory(&context, &r, None, None, now)["entryAssessment"],
        "pending"
    );
    context["symbol"] = json!("QQQ");
    context["at"] = json!(now - 3001);
    assert_eq!(
        advisory(&context, &r, None, None, now)["entryAssessment"],
        "pending"
    );
}
#[test]
fn lost_anchor_and_opposing_choch_request_review_not_an_exit() {
    let (s, now) = fixture();
    let mut c = read(&s, "QQQ", now);
    c["layers"][0]["events"] = json!([{"kind":"CHOCH","side":-1,"at":now}]);
    let r = json!({"symbol":"QQQ","at":now,"management_ready":true,"spot":94.,"eligible":false,"entry_side":"CALL"});
    let a = advisory(&c, &r, Some(95.), Some("CALL"), now);
    assert_eq!(a["exitReviewSuggested"], true);
    assert_eq!(a["executionEnabled"], false);
    assert!(a.get("exitOrder").is_none());
    c["layers"][0]["segmentStart"] = json!(now + 1);
    assert_eq!(
        advisory(&c, &r, Some(95.), Some("CALL"), now)["exitReviewSuggested"],
        false
    );
}
#[test]
fn full_session_and_five_assets_have_bounded_cost() {
    let (mut s, now) = fixture();
    let Day::Open(day) = calendar::schedule(calendar::date_at(now).unwrap()) else {
        panic!()
    };
    let at = day.close_ms - 60000;
    let rows: Vec<_> = (day.open_ms..at)
        .step_by(60000)
        .enumerate()
        .map(|(i, t)| {
            let p = 100. + (i % 8) as f64 * 0.1;
            json!({"timestampMs":t,"open":p,"high":p+0.5,"low":p-0.5,"close":p+0.1,"volume":100.})
        })
        .collect();
    s["chartHistory"]["minutes"] = json!(rows);
    s["publishedMs"] = json!(at);
    s["ticks"]["QQQ"]["tradeMs"] = json!(at);
    let start = std::time::Instant::now();
    for _ in 0..30 {
        for symbol in ["QQQ", "SPY", "NVDA", "AAPL", "TSLA"] {
            let mut own = s.clone();
            own["symbol"] = json!(symbol);
            own["ticks"][symbol] = own["ticks"]["QQQ"].clone();
            own["ticks"][symbol]["symbol"] = json!(symbol);
            let r = read(&own, symbol, at);
            assert_eq!(r["strategyConnected"], false);
            assert!(r["layers"][0]["events"].as_array().unwrap().len() <= 64);
        }
    }
    let ms = start.elapsed().as_secs_f64() * 1000. / 30.;
    println!("SMC observer, five full-session assets: {ms:.3} ms/cycle debug");
    assert!(ms < 100.);
}
