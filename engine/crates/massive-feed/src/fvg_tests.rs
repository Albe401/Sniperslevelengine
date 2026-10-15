use super::*;
fn fixture(tf: u64, bear: bool, extra: &[(f64, f64, f64, f64)]) -> (Value, u64) {
    let open = snipers_core::calendar::rfc3339_ms("2026-10-06T09:30:00-04:00").unwrap();
    let shape = [
        (100., 101., 99., 100.),
        (100., 104., 100., 104.),
        (103., 105., 103., 104.),
    ];
    let mut minutes = vec![];
    for (o, h, l, c) in shape {
        for _ in 0..tf {
            let t = open + minutes.len() as u64 * 60000;
            let (o, h, l, c) = if bear {
                (200. - o, 200. - l, 200. - h, 200. - c)
            } else {
                (o, h, l, c)
            };
            minutes.push(json!({"timestampMs":t,"open":o,"high":h,"low":l,"close":c,"volume":10.}));
        }
    }
    for &(o, h, l, c) in extra {
        minutes.push(json!({"timestampMs":open+minutes.len() as u64*60000,"open":o,"high":h,"low":l,"close":c,"volume":10.}));
    }
    let now = open + minutes.len() as u64 * 60000;
    let reference = if bear { 98. } else { 102. };
    (
        json!({"symbol":"QQQ","publishedMs":now,"stocks":{"phase":"streaming"},"ticks":{"QQQ":{"price":104.,"tradeMs":now}},"chartHistory":{"minutes":minutes},"metrics":{"session":"2026-10-06","asofMs":now,"vwap":reference},"optionSnapshotStatus":{"symbol":"QQQ","phase":"ready","observedMs":now,"horizons":[{"mode":"DAY","observedMs":now,"expirations":["2026-10-06"],"structure":{"complete":true,"callWall":{"strike":reference},"putWall":{"strike":reference}}}]}}),
        now,
    )
}
fn gap(s: &Value, now: u64, tf: u64) -> Value {
    read(s, "QQQ", 104., Some(110.), "CALL", now)["zones"]
        .as_array()
        .unwrap()
        .iter()
        .find(|z| z["timeframe"] == tf)
        .unwrap()
        .clone()
}
#[test]
fn closed_gap_geometry_midpoint_and_bear_symmetry() {
    for (bear, low, high, side) in [(false, 101., 103., "BULLISH"), (true, 97., 99., "BEARISH")] {
        let (s, now) = fixture(2, bear, &[]);
        let z = gap(&s, now, 2);
        assert_eq!(z["low"], low);
        assert_eq!(z["high"], high);
        assert_eq!(z["midpoint"], (low + high) / 2.);
        assert_eq!(z["side"], side);
        assert_eq!(z["state"], "NEW");
        assert_eq!(z["strength"], "STRONG");
        assert!(read(&s, "QQQ", 104., None, "CALL", now - 1)["zones"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}
#[test]
fn context_15m_is_session_anchored_and_closed() {
    let (s, now) = fixture(15, false, &[]);
    let z = gap(&s, now, 15);
    assert_eq!(z["createdMs"], now);
    assert_eq!(z["low"], 101.);
    assert!(read(&s, "QQQ", 104., None, "CALL", now - 1)["zones"]
        .as_array()
        .unwrap()
        .iter()
        .all(|z| z["timeframe"] != 15));
}
#[test]
fn partial_retest_fill_and_crossing_are_distinct() {
    let (s, now) = fixture(
        2,
        false,
        &[(104., 105., 102., 104.), (104., 105., 102., 104.)],
    );
    let r = read(&s, "QQQ", 104., Some(110.), "CALL", now);
    let z = gap(&s, now, 2);
    assert_eq!(z["state"], "PARTIAL");
    assert_eq!(z["filledFraction"], 0.5);
    assert_eq!(r["alignment"], "supports-pullback");
    for (close, state) in [(104., "FILLED"), (100., "CROSSED")] {
        let (s, now) = fixture(2, false, &[(104., 105., 100., close)]);
        let z = gap(&s, now, 2);
        assert_eq!(z["state"], state);
        assert_eq!(z["supportRetest"], false);
        assert!(z["filledMs"].as_u64().is_some());
    }
}
#[test]
fn duplicates_gaps_invalid_ohlc_and_foreign_data_never_create_known_evidence() {
    for kind in 0..3 {
        let (mut s, now) = fixture(2, false, &[]);
        match kind {
            0 => {
                let row = s["chartHistory"]["minutes"][2].clone();
                s["chartHistory"]["minutes"]
                    .as_array_mut()
                    .unwrap()
                    .push(row);
            }
            1 => {
                s["chartHistory"]["minutes"]
                    .as_array_mut()
                    .unwrap()
                    .remove(2);
            }
            _ => s["chartHistory"]["minutes"][2]["high"] = json!(90.),
        };
        assert!(read(&s, "QQQ", 104., None, "CALL", now)["zones"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    let (mut s, now) = fixture(2, false, &[(104., 105., 102., 104.)]);
    s["chartHistory"]["minutes"].as_array_mut().unwrap().pop();
    assert_eq!(gap(&s, now, 2)["state"], "UNKNOWN");
    assert_eq!(read(&s, "SPY", 104., None, "CALL", now)["available"], false);
}
#[test]
fn path_obstacle_blocks_only_existing_entry_without_new_votes_or_risk_levels() {
    let (s, now) = fixture(2, true, &[]);
    let mut r = crate::intraday_strategy::Reading {
        symbol: "QQQ".into(),
        spot: 96.,
        next: Some(105.),
        entry_side: "CALL".into(),
        ready: true,
        eligible: true,
        management_ready: true,
        net: 0.7,
        quick_context: json!({}),
        ..Default::default()
    };
    apply(&s, &mut r, now);
    assert!(!r.eligible);
    assert_eq!(r.quick_context["fvg"]["alignment"], "opposing-path");
    assert_eq!(r.next, Some(105.));
    assert_eq!(r.net, 0.7);
    assert!(r.management_ready);
    let (s, now) = fixture(
        2,
        false,
        &[(104., 105., 102., 104.), (104., 105., 102., 104.)],
    );
    r.spot = 104.;
    r.eligible = false;
    apply(&s, &mut r, now);
    assert!(!r.eligible, "FVG support cannot manufacture eligibility");
}
#[test]
fn missing_or_stale_confluence_cannot_block_a_path() {
    let (mut s, now) = fixture(2, true, &[]);
    s["metrics"] = Value::Null;
    s["optionSnapshotStatus"]["symbol"] = json!("SPY");
    let r = read(&s, "QQQ", 96., Some(105.), "CALL", now);
    assert_eq!(r["pathBlocked"], false);
    s["publishedMs"] = json!(now - 3001);
    assert_eq!(
        read(&s, "QQQ", 96., Some(105.), "CALL", now)["available"],
        false
    );
}
#[test]
fn later_bars_preserve_creation_geometry_and_obstacle_is_symmetric() {
    let (a, at) = fixture(2, false, &[]);
    let (b, bt) = fixture(
        2,
        false,
        &[(104., 105., 102., 104.), (104., 105., 102., 104.)],
    );
    let x = gap(&a, at, 2);
    let y = gap(&b, bt, 2);
    for field in ["id", "createdMs", "low", "high", "midpoint"] {
        assert_eq!(x[field], y[field]);
    }
    assert_eq!(
        read(&a, "QQQ", 104., Some(98.), "PUT", at)["pathBlocked"],
        true
    );
    let (a, at) = fixture(2, true, &[(96., 100., 95., 100.), (100., 101., 99., 100.)]);
    assert_eq!(
        read(&a, "QQQ", 100., Some(105.), "CALL", at)["pathBlocked"],
        false,
        "crossed gaps no longer act as intact resistance"
    );
}
#[test]
fn full_session_work_is_bounded() {
    let (mut s, start) = fixture(2, false, &[]);
    let open = start - 360000;
    let rows = s["chartHistory"]["minutes"].as_array_mut().unwrap();
    for i in 6..388 {
        let p = 100. + (i % 6) as f64;
        rows.push(json!({"timestampMs":open+i*60000,"open":p,"high":p+0.5,"low":p-0.5,"close":p+0.2,"volume":10.}));
    }
    let now = open + 388 * 60000;
    s["publishedMs"] = json!(now);
    s["ticks"]["QQQ"]["tradeMs"] = json!(now);
    let timer = std::time::Instant::now();
    for _ in 0..20 {
        let r = read(&s, "QQQ", 104., Some(110.), "CALL", now);
        assert!(r["zones"].as_array().unwrap().len() <= 40);
    }
    println!(
        "FVG full-session average: {:.3} ms",
        timer.elapsed().as_secs_f64() * 1000. / 20.
    );
}
