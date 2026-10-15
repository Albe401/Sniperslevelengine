use massive_feed::swing_context::*;
use serde_json::{json, Value};
use snipers_core::{calendar, horizons};
fn time(text: &str) -> u64 {
    calendar::rfc3339_ms(text).unwrap()
}
fn now() -> u64 {
    time("2026-10-05T10:00:00-04:00")
}
fn fixture(symbol: &str, t: u64, call: bool) -> Value {
    let all = vec![
        "2026-10-05".to_owned(),
        "2026-10-06".into(),
        "2026-10-07".into(),
        "2026-10-09".into(),
        "2026-10-12".into(),
    ];
    let hs=["DAY","WEEK","NEXT4"].map(|mode|json!({"mode":mode,"observedMs":t,"expirations":horizons::select(&all,mode,t),"exposure":{"eligible":2,"deltaUsed":2,"unknownOi":0,"dexComplete":true,"callDex":if call{800.}else{200.},"putDex":if call{-200.}else{-800.},"netDex":if call{600.}else{-600.},"callDexPct":if call{80.}else{20.},"putDexPct":if call{20.}else{80.}},"structure":{"complete":true,"callWall":{"strike":106.},"putWall":{"strike":94.},"selected":{"M1":if call{104.}else{96.}},"routes":[{"barrier":if call{102.}else{98.},"pullTarget":if call{110.}else{90.}}]}}));
    json!({"symbol":symbol,"publishedMs":t,"stocks":{"phase":"streaming"},"ticks":{symbol:{"symbol":symbol,"price":100.,"tradeMs":t}},"optionSnapshotStatus":{"phase":"ready","observedMs":t,"expirations":all,"horizons":hs}})
}
fn evidence(symbol: &str, t: u64, side: &str) -> IntradayEvidence {
    IntradayEvidence {
        symbol: symbol.into(),
        at: t,
        price_side: Some(side.into()),
        flow_side: Some(side.into()),
        cvd_side: Some(side.into()),
        qqq: None,
    }
}
fn bar(symbol: &str, start: u64, close: f64) -> ClosedBar {
    ClosedBar {
        symbol: symbol.into(),
        start_ms: start,
        open: close,
        high: close + 1.,
        low: close - 1.,
        close,
    }
}
#[test]
fn symmetric_conditional_routes_keep_horizons_separate() {
    for call in [true, false] {
        let f = fixture("NVDA", now(), call);
        let before = f.clone();
        let r = evaluate(&f, "NVDA", true, now());
        assert!(r.available);
        assert_eq!(r.state, "conditional");
        assert_eq!(r.horizons, vec!["DAY", "WEEK", "NEXT4"]);
        assert_eq!(r.anchor.unwrap().price, if call { 94. } else { 106. });
        assert_eq!(r.next.unwrap().price, if call { 102. } else { 98. });
        assert_eq!(f, before);
    }
}
#[test]
fn missing_future_stale_foreign_and_partial_stay_pending() {
    for kind in 0..15 {
        let mut f = fixture("NVDA", now(), true);
        match kind {
            0 => f["symbol"] = json!("QQQ"),
            1 => f["publishedMs"] = json!(now() - 3001),
            2 => f["publishedMs"] = json!(now() + 1),
            3 => f["ticks"]["NVDA"]["tradeMs"] = json!(now() - 15001),
            4 => f["stocks"]["phase"] = json!("disconnected"),
            5 => f["optionSnapshotStatus"]["observedMs"] = json!(now() - 900001),
            6 => f["optionSnapshotStatus"]["horizons"][0]["observedMs"] = json!(now() - 1),
            7 => f["optionSnapshotStatus"]["horizons"][2]["exposure"]["dexComplete"] = json!(false),
            8 => f["optionSnapshotStatus"]["horizons"][1]["exposure"]["netDex"] = json!(9),
            9 => {
                f["optionSnapshotStatus"]["horizons"][1]["structure"]["provisional"] =
                    json!({"partial":true})
            }
            10 => {
                f["optionSnapshotStatus"]["horizons"][2]["expirations"] =
                    json!(["2026-10-05", "2026-10-06"])
            }
            11 => {
                f["optionSnapshotStatus"]["horizons"][1]["structure"]["putWall"]["strike"] =
                    json!(107)
            }
            12 => f["optionSnapshotStatus"]["horizons"][0]["exposure"]["deltaUsed"] = json!(1),
            13 => f["optionSnapshotStatus"]["expirations"] = json!(["bad-date"]),
            _ => f["optionSnapshotStatus"]["horizons"] = json!([]),
        }
        let r = evaluate(&f, "NVDA", true, now());
        assert!(!r.available, "case {kind}");
        assert_eq!(r.state, "pending");
        assert!(r.next.is_none());
    }
    assert!(!evaluate(&fixture("NVDA", now(), true), "NVDA", false, now()).available);
}
#[test]
fn mixed_exposure_never_averages_three_horizons_into_a_direction() {
    let mut f = fixture("NVDA", now(), true);
    let bear = fixture("NVDA", now(), false);
    f["optionSnapshotStatus"]["horizons"][1]["exposure"] =
        bear["optionSnapshotStatus"]["horizons"][1]["exposure"].clone();
    let r = evaluate(&f, "NVDA", true, now());
    assert_eq!(r.state, "mixed");
    assert_eq!(r.side, "NEUTRAL");
    assert!(r.next.is_none());
}
#[test]
fn historical_and_closed_market_data_do_not_seed_a_plan() {
    for t in [
        time("2026-10-04T10:00:00-04:00"),
        time("2026-10-05T03:59:00-04:00"),
        time("2026-10-05T16:00:00-04:00"),
    ] {
        assert!(!evaluate(&fixture("NVDA", t, true), "NVDA", true, t).available);
    }
    let t = time("2026-10-05T08:00:00-04:00");
    let mut f = fixture("NVDA", t, true);
    f["optionSnapshotStatus"]["observedMs"] = json!(time("2026-10-02T15:59:00-04:00"));
    assert!(!evaluate(&f, "NVDA", true, t).available);
}
#[test]
fn intraday_divergence_does_not_rewrite_structural_direction() {
    let e = evidence("NVDA", now(), "PUT");
    let r = confirmation("CALL", "NVDA", &e, now());
    assert_eq!(r.state, "divergence");
    assert_eq!(r.components, "not-applicable");
    assert_eq!(
        evaluate(&fixture("NVDA", now(), true), "NVDA", true, now()).side,
        "CALL"
    );
    assert_eq!(
        confirmation("CALL", "NVDA", &e, now() + 3001).state,
        "unconfirmed"
    );
    assert_eq!(confirmation("CALL", "QQQ", &e, now()).state, "unconfirmed");
}
#[test]
fn qqq_requires_its_own_valid_basket_and_does_not_vote_missing_as_neutral() {
    let mut e = evidence("QQQ", now(), "CALL");
    assert_eq!(confirmation("CALL", "QQQ", &e, now()).state, "unconfirmed");
    let mut q = massive_feed::qqq_component_reading::Report::default();
    q.version = "qqq-components-v2".into();
    q.symbol = "QQQ".into();
    q.at = now();
    q.available = true;
    q.phase = "Sesión RTH".into();
    q.option_coverage = 45.;
    q.price_coverage = 50.;
    q.call = Some(75.);
    q.put = Some(25.);
    q.selected_weight = 50.;
    q.coverage = 45.;
    e.qqq = Some(q);
    assert_eq!(confirmation("CALL", "QQQ", &e, now()).state, "supported");
    e.qqq.as_mut().unwrap().call = Some(25.);
    e.qqq.as_mut().unwrap().put = Some(75.);
    assert_eq!(confirmation("CALL", "QQQ", &e, now()).state, "divergence");
    e.qqq.as_mut().unwrap().at -= 3001;
    assert_eq!(confirmation("CALL", "QQQ", &e, now()).components, "pending");
}
#[test]
fn freeze_targets_across_updates_and_json_restart() {
    let mut engine = Engine::default();
    let r = engine.update(
        &fixture("NVDA", now(), true),
        "NVDA",
        true,
        now(),
        &evidence("NVDA", now(), "CALL"),
        &[],
    );
    assert_eq!(r.state, "observing");
    let original = r.plan.unwrap();
    let text = serde_json::to_string(&engine).unwrap();
    let mut restored: Engine = serde_json::from_str(&text).unwrap();
    let t = now() + 60000;
    let mut f = fixture("NVDA", t, true);
    for h in f["optionSnapshotStatus"]["horizons"]
        .as_array_mut()
        .unwrap()
    {
        h["structure"]["routes"][0]["barrier"] = json!(103.);
    }
    let r = restored.update(&f, "NVDA", true, t, &evidence("NVDA", t, "PUT"), &[]);
    assert_eq!(r.plan.unwrap().target, original.target);
    assert_eq!(r.intraday.state, "divergence");
    assert_eq!(r.state, "observing");
}
#[test]
fn survives_overnight_but_missing_data_suspends_without_clearing_plan() {
    let mut e = Engine::default();
    e.update(
        &fixture("NVDA", now(), true),
        "NVDA",
        true,
        now(),
        &IntradayEvidence::default(),
        &[],
    );
    let target = e.plan.as_ref().unwrap().target.clone();
    let t = time("2026-10-06T10:00:00-04:00");
    let r = e.update(
        &fixture("NVDA", t, true),
        "NVDA",
        true,
        t,
        &IntradayEvidence::default(),
        &[],
    );
    assert_eq!(r.state, "observing");
    assert_eq!(e.plan.as_ref().unwrap().target, target);
    let r = e.update(
        &json!({}),
        "NVDA",
        true,
        t + 1,
        &IntradayEvidence::default(),
        &[],
    );
    assert_eq!(r.state, "suspended");
    assert!(r.remaining_dollars.is_none());
    assert!(e.plan.is_some());
}
#[test]
fn one_closed_bar_or_open_gap_duplicate_foreign_bar_never_invalidates() {
    for kind in 0..5 {
        let mut e = Engine::default();
        e.update(
            &fixture("NVDA", now(), true),
            "NVDA",
            true,
            now(),
            &IntradayEvidence::default(),
            &[],
        );
        let t = now() + 3600000;
        let a = bar("NVDA", now(), 92.);
        let mut b = bar("NVDA", now() + 1800000, 92.);
        let bars = match kind {
            0 => vec![a],
            1 => {
                b.start_ms = t;
                vec![a, b]
            }
            2 => {
                b.start_ms = now();
                vec![a, b]
            }
            3 => {
                b.symbol = "QQQ".into();
                vec![a, b]
            }
            _ => {
                b.start_ms = now() + 3600000;
                vec![a, b]
            }
        };
        let r = e.update(
            &fixture("NVDA", t, true),
            "NVDA",
            true,
            t,
            &IntradayEvidence::default(),
            &bars,
        );
        assert_ne!(r.state, "invalidated", "case {kind}");
        assert!(e.plan.is_some());
    }
}
#[test]
fn two_consecutive_closes_invalidate_and_target_close_finishes_symmetrically() {
    for call in [true, false] {
        for reaches in [true, false] {
            let mut e = Engine::default();
            e.update(
                &fixture("NVDA", now(), call),
                "NVDA",
                true,
                now(),
                &IntradayEvidence::default(),
                &[],
            );
            let close = if call {
                if reaches {
                    103.
                } else {
                    92.
                }
            } else if reaches {
                97.
            } else {
                108.
            };
            let t = now() + 3600000;
            let bars = vec![
                bar("NVDA", now(), close),
                bar("NVDA", now() + 1800000, close),
            ];
            let r = e.update(
                &fixture("NVDA", t, call),
                "NVDA",
                true,
                t,
                &IntradayEvidence::default(),
                &bars,
            );
            assert_eq!(r.state, if reaches { "reached" } else { "invalidated" });
            assert!(r.plan.is_some());
            assert!(e.plan.is_none());
        }
    }
}
#[test]
fn expired_corrupt_symbol_and_backward_clock_are_safe() {
    let mut e = Engine::default();
    e.update(
        &fixture("NVDA", now(), true),
        "NVDA",
        true,
        now(),
        &IntradayEvidence::default(),
        &[],
    );
    let before = serde_json::to_string(&e).unwrap();
    assert_eq!(
        e.update(
            &fixture("NVDA", now(), true),
            "NVDA",
            true,
            now() - 1,
            &IntradayEvidence::default(),
            &[]
        )
        .state,
        "suspended"
    );
    assert_eq!(serde_json::to_string(&e).unwrap(), before);
    let t = time("2026-10-09T16:00:00-04:00");
    assert_eq!(
        e.update(
            &fixture("NVDA", t, true),
            "NVDA",
            true,
            t,
            &IntradayEvidence::default(),
            &[]
        )
        .state,
        "expired"
    );
    assert!(e.plan.is_none());
    let mut e: Engine = serde_json::from_str(&before).unwrap();
    e.plan.as_mut().unwrap().target.price = -2.;
    assert_eq!(
        e.update(
            &fixture("NVDA", now(), true),
            "NVDA",
            true,
            now(),
            &IntradayEvidence::default(),
            &[]
        )
        .state,
        "expired"
    );
    let mut e: Engine = serde_json::from_str(&before).unwrap();
    let r = e.update(
        &fixture("QQQ", now(), false),
        "QQQ",
        true,
        now(),
        &IntradayEvidence::default(),
        &[],
    );
    assert_eq!(r.plan.unwrap().symbol, "QQQ");
}
#[test]
fn bounded_fast_input_is_not_mutated() {
    let f = fixture("NVDA", now(), true);
    let before = f.clone();
    let start = std::time::Instant::now();
    for _ in 0..5000 {
        assert_eq!(evaluate(&f, "NVDA", true, now()).state, "conditional");
    }
    let ms = start.elapsed().as_secs_f64() * 1000. / 5000.;
    println!("Swing context: {ms:.4} ms/update");
    assert!(ms < 5.);
    assert_eq!(f, before);
    let mut excessive = f.clone();
    excessive["optionSnapshotStatus"]["horizons"][1]["structure"]["routes"] =
        json!(vec![json!({"barrier":102.}); 129]);
    assert!(!evaluate(&excessive, "NVDA", true, now()).available);
}
#[test]
fn atr_is_only_distance_and_foreign_or_old_atr_is_ignored() {
    for kind in 0..3 {
        let mut f = fixture("NVDA", now(), true);
        f["dailyAtr"] = json!({"symbol":if kind==1{"QQQ"}else{"NVDA"},"through":if kind==2{"2026-10-01"}else{"2026-10-02"},"value":2.});
        let mut e = Engine::default();
        let r = e.update(&f, "NVDA", true, now(), &IntradayEvidence::default(), &[]);
        assert_eq!(r.context.side, "CALL");
        assert_eq!(r.remaining_dollars, Some(2.));
        assert_eq!(
            r.remaining_daily_atr,
            if kind == 0 { Some(1.) } else { None }
        );
    }
}
#[test]
fn adapter_reuses_existing_readings_without_modifying_them() {
    let f = fixture("NVDA", now(), true);
    let r = massive_feed::intraday_strategy::Reading {
        symbol: "NVDA".into(),
        at: now(),
        ready: true,
        price: 0.5,
        flow_one: 0.4,
        flow_five: 0.3,
        quick_context: json!({"symbol":"NVDA","at":now(),"cvd":{"side":"CALL"}}),
        ..Default::default()
    };
    let before = serde_json::to_string(&r).unwrap();
    let input = intraday_evidence(&f, &r, now());
    assert_eq!(
        confirmation("CALL", "NVDA", &input, now()).state,
        "supported"
    );
    assert_eq!(serde_json::to_string(&r).unwrap(), before);
    assert_eq!(intraday_evidence(&f, &r, now() + 3001).price_side, None);
}
#[test]
fn completed_snapshot_cannot_republish_the_same_plan_after_restart() {
    let mut e = Engine::default();
    let t = now() + 3600000;
    e.update(
        &fixture("NVDA", now(), true),
        "NVDA",
        true,
        now(),
        &IntradayEvidence::default(),
        &[],
    );
    let f = fixture("NVDA", t, true);
    let bars = vec![bar("NVDA", now() + 1800000, 103.)];
    assert_eq!(
        e.update(&f, "NVDA", true, t, &IntradayEvidence::default(), &bars)
            .state,
        "reached"
    );
    let mut restored: Engine = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
    assert!(restored
        .update(&f, "NVDA", true, t + 1, &IntradayEvidence::default(), &[])
        .plan
        .is_none());
    assert!(restored
        .update(
            &fixture("NVDA", t + 2, true),
            "NVDA",
            true,
            t + 2,
            &IntradayEvidence::default(),
            &[]
        )
        .plan
        .is_some());
}
#[test]
fn corrupted_plan_dates_cannot_extend_the_structural_horizon() {
    let mut e = Engine::default();
    e.update(
        &fixture("NVDA", now(), true),
        "NVDA",
        true,
        now(),
        &IntradayEvidence::default(),
        &[],
    );
    e.plan.as_mut().unwrap().expires = "2026-12-31".into();
    assert_eq!(
        e.update(
            &fixture("NVDA", now(), true),
            "NVDA",
            true,
            now(),
            &IntradayEvidence::default(),
            &[]
        )
        .state,
        "expired"
    );
}
#[test]
fn complete_engine_is_bounded_fast_without_side_effects() {
    let f = fixture("NVDA", now(), true);
    let e = evidence("NVDA", now(), "CALL");
    let mut engine = Engine::default();
    let start = std::time::Instant::now();
    for _ in 0..5000 {
        assert_eq!(
            engine.update(&f, "NVDA", true, now(), &e, &[]).state,
            "observing"
        );
    }
    let ms = start.elapsed().as_secs_f64() * 1000. / 5000.;
    println!("Swing full engine: {ms:.4} ms/update");
    assert!(ms < 5.);
}
