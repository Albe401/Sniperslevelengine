//! Route confirmation for new admission; never changes position management or risk levels.
use crate::intraday_strategy::Reading;
use serde_json::{json, Value};
pub fn attach(r: &mut Reading, review: &Value, now: u64) {
    let current = r.at <= now
        && now - r.at <= 5000
        && (r.ready || r.management_ready)
        && r.spot.is_finite()
        && r.spot > 0.;
    let parallel:Vec<_>=[("CALL",1.),("PUT",-1.)].into_iter().map(|(side,sign)|{
        let mut ahead:Vec<_>=if current {r.levels.iter().copied().filter(|p|p.is_finite()&&*p>0.&&(*p-r.spot)*sign>r.spot*0.0003).collect()}else{Vec::new()};
        ahead.sort_by(|a,b|(a-r.spot).abs().total_cmp(&(b-r.spot).abs()));ahead.dedup();
        json!({"side":side,"state":if ahead.is_empty(){"pending"}else{"conditional"},"activationReference":ahead.first(),"nextReference":ahead.get(1),"invalidation":null,"confirmedEntry":false})
    }).collect();
    let reading = &review["reading"];
    let usable = current
        && review["available"] == true
        && review["executionEnabled"] == false
        && review["mode"] == "review-only"
        && reading["version"] == crate::route_4h::VERSION
        && reading["symbol"] == r.symbol
        && reading["at"]
            .as_u64()
            .is_some_and(|t| t <= now && now - t <= 20000);
    let scenario = if usable {
        serde_json::from_value::<crate::route_4h::Scenario>(reading["scenario"].clone())
            .ok()
            .filter(|s| {
                s.symbol == r.symbol
                    && matches!(s.side, 1 | -1)
                    && s.at <= now
                    && [s.origin, s.target, s.invalidation]
                        .iter()
                        .all(|p| p.is_finite() && *p > 0.)
                    && (s.target - s.origin) * s.side as f64 > 0.
                    && (s.origin - s.invalidation) * s.side as f64 > 0.
            })
    } else {
        None
    };
    let active = scenario.as_ref().is_some_and(|_| {
        review["observation"]["status"] == "pending"
            && review["observation"]["deadline"]
                .as_u64()
                .is_some_and(|end| end > now)
    });
    let side = scenario
        .as_ref()
        .map(|s| if s.side == 1 { "CALL" } else { "PUT" });
    let alignment = if !active || !matches!(r.entry_side.as_str(), "CALL" | "PUT") {
        "pending"
    } else if side == Some(r.entry_side.as_str()) {
        "supports"
    } else {
        "opposes"
    };
    let projections = if active {
        review["projections"].clone()
    } else {
        Value::Null
    };
    r.quick_context["priceRoute"] = json!({"version":"bot-price-route-v1","symbol":r.symbol,"at":now,"mode":"context-only","executionEnabled":false,"parallel":parallel,"fourHourAvailable":active,"fourHour":if usable{reading.clone()}else{Value::Null},"observation":if usable{review["observation"].clone()}else{Value::Null},"projections":projections,"intradayAlignment":alignment,"confirmation":if usable{review["confirmation"].clone()}else{Value::Null},"probabilityClaim":false,"usesUiSelection":false});
}
/// Never promotes an ineligible reading. Insufficient historical support stays informational.
pub fn confirm_entry(r: &mut Reading, now: u64) {
    let route = &r.quick_context["priceRoute"];
    let baseline = r.eligible;
    let valid = r.at <= now && now - r.at <= 5000
        && route["symbol"] == r.symbol && route["version"] == "bot-price-route-v1"
        && route["at"].as_u64().is_some_and(|t| t <= now && now-t <= 5000)
        && route["fourHourAvailable"] == true
        && route["observation"]["status"] == "pending"
        && route["observation"]["deadline"].as_u64().is_some_and(|t| t > now);
    let supported = route["projections"].as_array().is_some_and(|ps| ps.iter().any(|p|
        p["horizon"] == "swing2" && p["state"] == "descriptive"
        && p["n"].as_u64().is_some_and(|n| n >= 20)));
    let alignment = route["intradayAlignment"].as_str().unwrap_or("pending");
    let sign = match r.entry_side.as_str() { "CALL" => 1., "PUT" => -1., _ => 0. };
    let scenario = &route["fourHour"]["scenario"];
    let within = scenario["side"].as_i64().filter(|s| matches!(s, 1 | -1)).is_some_and(|s|
        scenario["target"].as_f64().zip(scenario["invalidation"].as_f64()).is_some_and(|(target, stop)|
            target.is_finite() && stop.is_finite() && target > 0. && stop > 0.
            && (target-r.spot)*s as f64 > 0. && (r.spot-stop)*s as f64 > 0.));
    let state = if !valid || !supported || !within || sign == 0. { "pending" }
        else if alignment == "supports" { "supports" }
        else if alignment == "opposes" {
            let bars = &r.closed_bars;
            let extra = bars.len() >= 3 && bars[bars.len()-3..].windows(2).all(|pair|
                pair[0]["t"].as_u64().zip(pair[1]["t"].as_u64()).is_some_and(|(a,b)| a.checked_add(120000)==Some(b))
                && pair[0]["close"].as_f64().zip(pair[1]["close"].as_f64()).is_some_and(|(a,b)|
                    a.is_finite() && b.is_finite() && a>0. && b>0. && (b-a)*sign>0.))
                && bars.last().is_some_and(|b| b["t"].as_u64().and_then(|t|t.checked_add(120000))
                    .is_some_and(|end|end<=now && now-end<=120000)
                    && b["close"].as_f64().is_some_and(|close|(r.spot-close)*sign>=0.));
            if extra { "countertrend-confirmed" } else {
                r.eligible = false;
                r.blockers.push("Conflicto 4H: esperando tres cierres consecutivos de 2m en la dirección intradía".into());
                "requires-extra-confirmation"
            }
        } else { "pending" };
    r.quick_context["priceRoute"]["mode"] = json!("entry-confirmation");
    r.quick_context["priceRoute"]["entryConfirmation"] = json!({"state":state,"baselineEligible":baseline,"finalEligible":r.eligible,"historicalSupport":supported,"changesRiskLevels":false});
}
#[cfg(test)]
mod tests {
    use super::*;
    fn reading() -> Reading {
        Reading {
            at: 100000,
            ready: true,
            eligible: true,
            management_ready: true,
            symbol: "QQQ".into(),
            entry_side: "CALL".into(),
            spot: 100.,
            next: Some(104.),
            levels: vec![98., 99., 101., 102.],
            quick_context: json!({"cvd":{"status":"confirms"}}),
            ..Default::default()
        }
    }
    fn review() -> Value {
        json!({"available":true,"mode":"review-only","executionEnabled":false,"reading":{"version":"route-4h-v1","symbol":"QQQ","at":100000,"scenario":{"id":"s","symbol":"QQQ","at":90000,"side":1,"setup":"breakout","structure":"up","origin":100.,"target":105.,"targetSource":"known-pivot","invalidation":98.,"blockMinutes":240}},"observation":{"status":"pending","deadline":200000},"projections":[{"horizon":"swing2","state":"insufficient","n":3}],"confirmation":{"state":"pending"}})
    }
    #[test]
    fn attaches_context_without_changing_trade_decisions() {
        let mut r = reading();
        let before = (
            r.eligible,
            r.ready,
            r.next,
            r.entry_side.clone(),
            r.blockers.clone(),
        );
        attach(&mut r, &review(), 100000);
        assert_eq!(
            before,
            (
                r.eligible,
                r.ready,
                r.next,
                r.entry_side.clone(),
                r.blockers.clone()
            )
        );
        assert_eq!(
            r.quick_context["priceRoute"]["intradayAlignment"],
            "supports"
        );
        assert_eq!(r.quick_context["cvd"]["status"], "confirms");
        assert_eq!(
            r.quick_context["priceRoute"]["parallel"][0]["activationReference"],
            101.
        );
    }
    #[test]
    fn rejects_foreign_stale_disconnected_or_expired_four_hour() {
        for i in 0..5 {
            let mut v = review();
            match i {
                0 => v["reading"]["symbol"] = json!("SPY"),
                1 => v["reading"]["at"] = json!(79999),
                2 => v["available"] = json!(false),
                3 => v["observation"]["status"] = json!("invalidated"),
                _ => v["observation"]["deadline"] = json!(100000),
            }
            let mut r = reading();
            attach(&mut r, &v, 100000);
            assert_eq!(r.quick_context["priceRoute"]["fourHourAvailable"], false);
            assert!(r.quick_context["priceRoute"]["projections"].is_null());
            assert!(r.eligible);
        }
    }
    #[test]
    fn opposing_structure_is_context_not_an_entry_override() {
        let mut r = reading();
        r.entry_side = "PUT".into();
        attach(&mut r, &review(), 100000);
        assert_eq!(
            r.quick_context["priceRoute"]["intradayAlignment"],
            "opposes"
        );
        assert!(r.eligible);
        assert!(r.management_ready);
    }
    #[test]
    fn stale_current_reading_cannot_publish_scenarios() {
        let mut r = reading();
        r.at = 94999;
        attach(&mut r, &review(), 100000);
        assert_eq!(r.quick_context["priceRoute"]["fourHourAvailable"], false);
        assert_eq!(
            r.quick_context["priceRoute"]["parallel"][0]["activationReference"],
            Value::Null
        );
    }
    #[test]
    fn confirmation_requires_extra_close_without_promoting_or_changing_management() {
        for side in ["CALL", "PUT"] {
            let sign = if side == "CALL" { 1. } else { -1. };
            let mut v = review();
            v["reading"]["scenario"]["side"] = json!(-sign as i64);
            v["reading"]["scenario"]["target"] = json!(100.-5.*sign);
            v["reading"]["scenario"]["invalidation"] = json!(100.+2.*sign);
            v["projections"][0] = json!({"horizon":"swing2","state":"descriptive","n":20});
            v["reading"]["at"] = json!(500000);
            v["observation"]["deadline"] = json!(600000);
            let mut r = reading(); r.at=500000; r.entry_side=side.into();
            r.closed_bars=vec![json!({"t":260000,"close":100.-2.*sign}),json!({"t":380000,"close":100.-sign})];
            attach(&mut r,&v,500000); confirm_entry(&mut r,500000);
            assert!(!r.eligible); assert!(r.management_ready); assert!(r.ready); assert_eq!(r.next,Some(104.));
            r.closed_bars.insert(0,json!({"t":140000,"close":100.-3.*sign}));
            r.eligible=true; attach(&mut r,&v,500000); confirm_entry(&mut r,500000); assert!(r.eligible);
            r.eligible=false; attach(&mut r,&v,500000); confirm_entry(&mut r,500000); assert!(!r.eligible);
            r.eligible=true; r.closed_bars[0]["t"]=json!(139999);
            attach(&mut r,&v,500000); confirm_entry(&mut r,500000); assert!(!r.eligible);
        }
    }
    #[test]
    fn insufficient_history_and_stale_context_preserve_admission() {
        let mut r=reading(); r.entry_side="PUT".into();
        attach(&mut r,&review(),100000); confirm_entry(&mut r,100000);
        assert!(r.eligible); assert_eq!(r.quick_context["priceRoute"]["entryConfirmation"]["state"],"pending");
        r.quick_context["priceRoute"]["at"]=json!(0);
        confirm_entry(&mut r,100000); assert!(r.eligible);
    }
    #[test]
    fn five_asset_context_cost_is_bounded() {
        let v = review();
        let start = std::time::Instant::now();
        for _ in 0..500 {
            for symbol in ["QQQ", "SPY", "AAPL", "NVDA", "TSLA"] {
                let mut r = reading();
                r.symbol = symbol.into();
                attach(&mut r, &v, 100000);
                confirm_entry(&mut r,100000);
            }
        }
        let ms = start.elapsed().as_secs_f64() * 1000. / 500.;
        println!("Route context five assets: {ms:.3} ms/cycle");
        assert!(ms < 50.);
    }
}
