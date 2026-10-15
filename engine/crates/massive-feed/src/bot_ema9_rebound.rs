//! EMA9 entry confirmation for QQQ. Quote-tested pressure is not candle colour.
//! Adds admission checks only; never changes management of an open position.
use crate::intraday_strategy::Reading;
use serde_json::{json, Value};
use snipers_core::calendar::{self, Day};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub struct Tracker {
    identity: String,
    bar: u64,
    sign: i8,
    count: u32,
}
#[derive(Clone)]
struct Bar {
    t: u64,
    o: f64,
    h: f64,
    l: f64,
    c: f64,
    v: f64,
}
fn n(v: &Value) -> Option<f64> {
    v.as_f64().filter(|v| v.is_finite())
}
fn fresh(v: &Value, now: u64, age: u64) -> bool {
    v.as_u64().is_some_and(|t| t <= now && now - t <= age)
}
fn bars(f: &Value, now: u64) -> Option<Vec<Bar>> {
    let date = calendar::date_at(now)?;
    let Day::Open(day) = calendar::schedule(date) else {
        return None;
    };
    let mut out = BTreeMap::new();
    for v in f["chartHistory"]["minutes"].as_array()? {
        let t = v["timestampMs"].as_u64()?;
        if t < day.open_ms || t.checked_add(60000)? > now {
            continue;
        }
        if t >= day.close_ms || (t - day.open_ms) % 60000 != 0 {
            return None;
        }
        let b = Bar {
            t,
            o: n(&v["open"])?,
            h: n(&v["high"])?,
            l: n(&v["low"])?,
            c: n(&v["close"])?,
            v: n(&v["volume"])?,
        };
        if b.l <= 0.
            || b.o <= 0.
            || b.c <= 0.
            || b.h < b.o.max(b.c)
            || b.l > b.o.min(b.c)
            || b.v < 0.
            || out.insert(t, b).is_some()
        {
            return None;
        }
    }
    let rows = out.into_values().collect::<Vec<_>>();
    let last = rows.last()?;
    if rows.len() < 23
        || now.checked_sub(last.t + 60000)? > 60000
        || rows.windows(2).any(|w| w[1].t - w[0].t != 60000)
    {
        return None;
    }
    Some(rows)
}
impl Tracker {
    pub fn apply(&mut self, f: &Value, r: &mut Reading, now: u64) {
        if r.symbol != "QQQ" {
            return;
        }
        let baseline = r.eligible;
        let date = calendar::date_at(now)
            .map(|d| d.to_string())
            .unwrap_or_default();
        let identity = format!(
            "{}:{}:{}:{}:{}",
            r.symbol,
            date,
            f["startedMs"],
            f["cvd"]["regular"]["sinceMs"],
            f["cvd"]["regular"]["gaps"]
        );
        if identity != self.identity {
            *self = Self {
                identity,
                ..Default::default()
            };
        }
        let connected = f["symbol"] == r.symbol
            && f["stocks"]["phase"] == "streaming"
            && fresh(&f["publishedMs"], now, 3000);
        let rows = if connected { bars(f, now) } else { None };
        let mut reasons = Vec::new();
        let side = match r.entry_side.as_str() {
            "CALL" => 1.,
            "PUT" => -1.,
            _ => 0.,
        };
        let mut key = None;
        let mut volume_ratio = None;
        let mut stop = None;
        let mut rr = None;
        let mut detected = false;
        let mut buy = None;
        let mut sell = None;
        let mut unknown = None;
        let mut coverage = None;
        let mut pressure = None;
        let mut bar = 0;
        if let Some(bs) = rows.as_ref() {
            bar = bs.last().unwrap().t;
            let i = bs.len() - 1;
            let last = &bs[i];
            let prior = &bs[i - 1];
            let current = crate::bot_ema::read(&f["chartHistory"], last.t + 60000);
            let prior_ema = crate::bot_ema::read(&f["chartHistory"], prior.t + 60000);
            let atr = bs[i - 14..=i]
                .windows(2)
                .map(|w| {
                    (w[1].h - w[1].l)
                        .max((w[1].h - w[0].c).abs())
                        .max((w[1].l - w[0].c).abs())
                })
                .sum::<f64>()
                / 14.;
            if current.available && prior_ema.available && side != 0. && atr > 0. {
                let (e9, e20, p9, p20) = (
                    current.ema9.unwrap(),
                    current.ema20.unwrap(),
                    prior_ema.ema9.unwrap(),
                    prior_ema.ema20.unwrap(),
                );
                let tolerance = atr * 0.05;
                if (prior.h - prior.l) < atr * 0.5 || (last.h - last.l) < atr * 0.5 {
                    reasons.push("Esperando amplitud de rebote y continuación de al menos 0,5 ATR");
                }
                // A completed touch/rejection at the previous minute, followed by continuation.
                let before = &bs[i - 2];
                let before_ema = crate::bot_ema::read(&f["chartHistory"], before.t + 60000);
                detected = before_ema
                    .ema9
                    .is_some_and(|e| (before.c - e) * side > tolerance)
                    && (p9 - p20) * side > 0.
                    && (e9 - e20) * side > 0.
                    && if side > 0. {
                        prior.l <= p9 + tolerance
                    } else {
                        prior.h >= p9 - tolerance
                    }
                    && (prior.c - p9) * side > tolerance
                    && (prior.c - prior.o) * side > 0.
                    && (last.c - e9) * side > tolerance
                    && (last.c - prior.c) * side > 0.
                    && (last.c - last.o) * side > 0.;
                let average = bs[i - 21..i - 1].iter().map(|b| b.v).sum::<f64>() / 20.;
                volume_ratio = (average > 0.).then_some(prior.v / average);
                let invalidation = if side > 0. {
                    prior.l.min(last.l)
                } else {
                    prior.h.max(last.h)
                };
                stop = Some(invalidation);
                let risk = (r.spot - invalidation) * side;
                rr = r
                    .next
                    .filter(|t| risk > 0. && (*t - r.spot) * side > 0.)
                    .map(|t| (t - r.spot) * side / risk);
                if detected {
                    key = Some(format!(
                        "{}:{}:{}:{}",
                        self.identity, r.entry_side, prior.t, invalidation
                    ));
                }
            }
            if !detected {
                reasons.push("Esperando toque/rebote EMA 9 y continuación en velas cerradas de 1m");
            }
            if !volume_ratio.is_some_and(|v| v >= 1.0) {
                reasons
                    .push("Volumen del rebote inferior a 1 vez las 20 velas previas o sin datos");
            }
            if !r
                .vwap
                .is_some_and(|v| (last.c - v) * side > 0. && (r.spot - v) * side > 0.)
            {
                reasons.push("Precio y VWAP no sostienen el lado del rebote");
            }
            if !r.spot.is_finite() || (r.spot - last.c) * side < 0. {
                reasons.push("El precio actual no sostiene la continuación cerrada");
            }
            if !rr.is_some_and(|v| v >= 1.8) {
                reasons.push("Falta un nivel verificado con recorrido mínimo 1,8R");
            }
        } else {
            reasons.push("Esperando conexión y minutos completos vigentes para EMA 9");
        }
        let c = &f["cvd"];
        let w = &c["recentFive"];
        let regular = &c["regular"];
        let window_ok = connected
            && c["symbol"] == r.symbol
            && c["date"] == date
            && c["available"] == true
            && fresh(&c["at"], now, 3000)
            && fresh(&regular["lastMs"], now, 15000)
            && fresh(&w["endMs"], now, 3000)
            && w["complete"] == true
            && regular["gaps"].as_u64().is_some()
            && w["endMs"]
                .as_u64()
                .zip(w["startMs"].as_u64())
                .is_some_and(|(end, start)| {
                    end.checked_sub(start) == Some(300000)
                        && regular["sinceMs"].as_u64().is_some_and(|t| t <= start)
                        && calendar::date_at(start) == calendar::date_at(now)
                });
        let mut sign = 0;
        if let Some((((b, s), u), d)) = n(&w["buy"])
            .zip(n(&w["sell"]))
            .zip(n(&w["unknown"]))
            .zip(n(&w["delta"]))
            .filter(|&(((b, s), u), d)| {
                window_ok
                    && b >= 0.
                    && s >= 0.
                    && u >= 0.
                    && (b + s + u).is_finite()
                    && b + s + u > 0.
                    && (d - b + s).abs() <= 1e-6_f64.max(d.abs() * 1e-9)
            })
        {
            buy = Some(b);
            sell = Some(s);
            unknown = Some(u);
            coverage = Some((b + s) / (b + s + u) * 100.);
            pressure = Some(d / (b + s + u));
            if coverage.unwrap() >= 50. {
                sign = if d >= (b + s) * 0.20 {
                    1
                } else if -d >= (b + s) * 0.20 {
                    -1
                } else {
                    0
                };
            }
        }
        if sign == 0 || bar == 0 || bar < self.bar {
            self.count = 0;
            self.bar = 0;
            self.sign = 0;
        } else if bar > self.bar {
            self.count = if self.sign == sign && self.bar.checked_add(60000) == Some(bar) {
                self.count + 1
            } else {
                1
            };
            self.bar = bar;
            self.sign = sign;
        } else if sign != self.sign {
            self.count = 0;
            self.sign = sign;
        }
        if self.count < 2 || f64::from(sign) != side {
            reasons.push("Esperando presión/CVD a favor en dos cierres distintos, cobertura parcial ≥50% y dominio clasificado ≥20%");
        }
        if !r.ready
            || r.flow_one * side < 0.15
            || r.flow_five * side <= 0.
            || !r.flow_one.is_finite()
            || !r.flow_five.is_finite()
        {
            reasons.push("La lectura vigente y el flujo de opciones 1m/5m no confirman");
        }
        if r.next.is_some_and(|target| {
            crate::bot_chart_route::strong_obstacle(f, r.spot, target, side, now)
        }) {
            reasons.push(
                "Un muro o barrera fuerte queda antes del objetivo: esperar reacción verificada",
            );
        }
        if !baseline {
            reasons.push("La estrategia base aún no admite la entrada: revisar sus bloqueos de estructura, fuerzas y contexto");
        }
        let confirmed = reasons.is_empty() && detected && baseline;
        r.eligible = confirmed;
        if confirmed {
            r.entry_basis = "ema9-rebound".into();
        } else {
            r.blockers.extend(reasons.iter().map(|s| s.to_string()));
        }
        r.quick_context["ema9Rebound"] = json!({"profile":"aggressive-3d","cvdBasis":"presión inferida de muestra clasificada; no representa todos los compradores/vendedores","version":"ema9-rebound-v1","symbol":r.symbol,"at":now,"key":key,"confirmed":confirmed,"detected":detected,"side":r.entry_side,"state":if confirmed{"confirmada"}else{"pendiente"},"volumeRatio":volume_ratio,"buyShares":buy,"sellShares":sell,"unknownShares":unknown,"coveragePct":coverage,"pressure":pressure,"cvdCloses":self.count,"invalidation":stop,"target":r.next,"rewardRisk":rr,"reasons":reasons,"baselineEligible":baseline,"strategyConnected":true,"executionEnabled":false,"profitabilityValidated":false,"volumeBasis":"rebote/20 minutos previos; RVOL por hora histórico no disponible en este módulo"});
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(side: f64) -> (Value, Reading, u64) {
        let open = calendar::legacy_eastern_ms("2026-10-02 09:30:00").unwrap();
        let now = open + 240 * 60000;
        let p = |v: f64| 100. + side * (v - 100.);
        let mut bs=(0..240).map(|i|{let close=100.+i as f64*0.01;json!({"timestampMs":open+i*60000,"open":p(close-0.005),"close":p(close),"low":p(close-0.02).min(p(close+0.02)),"high":p(close-0.02).max(p(close+0.02)),"volume":100.})}).collect::<Vec<_>>();
        let index = 238;
        let c = 102.38;
        bs[index] = json!({"timestampMs":open+index as u64*60000,"open":p(c-0.01),"close":p(c),"low":p(c-0.08).min(p(c+0.02)),"high":p(c-0.08).max(p(c+0.02)),"volume":200.});
        let f = json!({"symbol":"QQQ","startedMs":open,"publishedMs":now,"stocks":{"phase":"streaming"},"chartHistory":{"minutes":bs},"cvd":{"symbol":"QQQ","date":"2026-10-02","at":now,"available":true,"regular":{"sinceMs":open,"lastMs":now,"gaps":0},"recentFive":{"startMs":now-300000,"endMs":now,"complete":true,"buy":if side>0.{1000.}else{10.},"sell":if side>0.{10.}else{1000.},"unknown":10.,"delta":990.*side}}});
        let r = Reading {
            symbol: "QQQ".into(),
            entry_side: if side > 0. { "CALL" } else { "PUT" }.into(),
            ready: true,
            eligible: true,
            management_ready: true,
            spot: p(102.39),
            vwap: Some(p(101.)),
            next: Some(p(105.)),
            flow_one: 0.3 * side,
            flow_five: 0.2 * side,
            ..Default::default()
        };
        (f, r, now)
    }
    #[test]
    fn repeats_cannot_manufacture_two_closes_and_never_promote_baseline() {
        for side in [1., -1.] {
            let (f, r, now) = sample(side);
            let mut tracker = Tracker::default();
            let mut first = r.clone();
            tracker.apply(&f, &mut first, now);
            assert!(first.quick_context["ema9Rebound"]["detected"] == true);
            assert!(!first.eligible);
            let mut again = r.clone();
            tracker.apply(&f, &mut again, now);
            assert_eq!(again.quick_context["ema9Rebound"]["cvdCloses"], 1);
            let mut blocked = r.clone();
            blocked.eligible = false;
            tracker.apply(&f, &mut blocked, now);
            assert!(!blocked.eligible && blocked.management_ready);
        }
    }
    #[test]
    fn confirmed_requires_all_gates_for_call_and_put() {
        for side in [1., -1.] {
            let (f, r, now) = sample(side);
            let bar = now - 60000;
            for variant in 0..9 {
                let mut tracker = Tracker {
                    identity: format!(
                        "QQQ:2026-10-02:{}:{}:{}",
                        f["startedMs"], f["cvd"]["regular"]["sinceMs"], f["cvd"]["regular"]["gaps"]
                    ),
                    bar: bar - 60000,
                    sign: side as i8,
                    count: 1,
                };
                let mut f = f.clone();
                let mut r = r.clone();
                match variant {
                    1 => f["cvd"]["recentFive"]["unknown"] = json!(10000.),
                    2 => f["stocks"]["phase"] = json!("disconnected"),
                    3 => r.eligible = false,
                    4 => r.flow_five = 0.,
                    5 => r.next = Some(r.spot + side * 0.01),
                    6 => {
                        f["chartHistory"]["minutes"][238]["volume"] = json!(10.);
                    }
                    7 => f["cvd"]["symbol"] = json!("SPY"),
                    8 => {
                        f["chartHistory"]["minutes"]
                            .as_array_mut()
                            .unwrap()
                            .remove(230);
                    }
                    _ => {}
                };
                tracker.apply(&f, &mut r, now);
                assert_eq!(
                    r.eligible,
                    variant == 0,
                    "side={side}, variant={variant}, context={}",
                    r.quick_context
                );
                assert!(r.management_ready);
                if variant == 0 {
                    assert_eq!(r.entry_basis, "ema9-rebound");
                    assert_eq!(r.next, Some(100. + side * 5.));
                }
            }
        }
    }
    #[test]
    fn stale_same_bar_pressure_flip_and_session_change_reset() {
        let (f, r, now) = sample(1.);
        let mut tracker = Tracker::default();
        let mut x = r.clone();
        tracker.apply(&f, &mut x, now);
        let mut x = r.clone();
        let mut bad = f.clone();
        bad["cvd"]["recentFive"]["delta"] = json!(-990.);
        bad["cvd"]["recentFive"]["buy"] = json!(10.);
        bad["cvd"]["recentFive"]["sell"] = json!(1000.);
        tracker.apply(&bad, &mut x, now);
        assert!(!x.eligible);
        assert_eq!(tracker.count, 0);
        let mut x = r.clone();
        tracker.apply(&f, &mut x, now + 16000);
        assert!(!x.eligible);
        assert_eq!(tracker.count, 0);
    }
    #[test]
    fn other_assets_and_unknown_inputs_are_never_reinterpreted() {
        let (f, mut r, now) = sample(1.);
        r.symbol = "SPY".into();
        Tracker::default().apply(&f, &mut r, now);
        assert!(r.eligible);
        assert!(r.quick_context["ema9Rebound"].is_null());
        let mut r = Reading {
            symbol: "QQQ".into(),
            eligible: true,
            ..Default::default()
        };
        Tracker::default().apply(&json!({}), &mut r, now);
        assert!(!r.eligible);
    }
    #[test]
    fn strong_wall_before_target_blocks_both_directions() {
        for side in [1., -1.] {
            let (mut f, mut r, now) = sample(side);
            f["optionSnapshotStatus"] = json!({"phase":"ready","observedMs":now,"horizons":[{"mode":"DAY","observedMs":now,"expirations":["2026-10-02"],"structure":{"callWall":{"strike":100.+side*3.,"score":80.},"putWall":{"strike":100.+side*3.,"score":80.}}}]});
            let mut tracker = Tracker {
                identity: format!(
                    "QQQ:2026-10-02:{}:{}:{}",
                    f["startedMs"], f["cvd"]["regular"]["sinceMs"], f["cvd"]["regular"]["gaps"]
                ),
                bar: now - 120000,
                sign: side as i8,
                count: 1,
            };
            tracker.apply(&f, &mut r, now);
            assert!(!r.eligible);
            assert!(r
                .blockers
                .iter()
                .any(|b| b.contains("muro o barrera fuerte")));
            assert!(r.management_ready);
        }
    }
    #[test]
    fn real_consecutive_closed_minutes_confirm_after_pressure_warmup() {
        for side in [1., -1.] {
            let (f, r, now) = sample(side);
            let mut prior = f.clone();
            let earlier = now - 60000;
            prior["publishedMs"] = json!(earlier);
            prior["cvd"]["at"] = json!(earlier);
            prior["cvd"]["regular"]["lastMs"] = json!(earlier);
            prior["cvd"]["recentFive"]["startMs"] = json!(earlier - 300000);
            prior["cvd"]["recentFive"]["endMs"] = json!(earlier);
            let mut tracker = Tracker::default();
            let mut first = r.clone();
            tracker.apply(&prior, &mut first, earlier);
            assert!(!first.eligible);
            assert_eq!(tracker.count, 1);
            let mut second = r.clone();
            tracker.apply(&f, &mut second, now);
            assert!(second.eligible, "{}", second.quick_context);
            assert_eq!(second.quick_context["ema9Rebound"]["cvdCloses"], 2);
        }
    }
    #[test]
    fn aggressive_partial_pressure_requires_majority_coverage_and_dominance() {
        for side in [1.,-1.] {
            for (buy,sell,unknown,accept) in [(800.,200.,900.,true),(550.,450.,900.,false),(800.,200.,1100.,false)] {
                let (mut f,mut r,now)=sample(side);
                let (b,s)=if side>0.{(buy,sell)}else{(sell,buy)};
                f["cvd"]["recentFive"]["buy"]=json!(b);f["cvd"]["recentFive"]["sell"]=json!(s);
                f["cvd"]["recentFive"]["unknown"]=json!(unknown);f["cvd"]["recentFive"]["delta"]=json!(b-s);
                let mut tracker=Tracker{identity:format!("QQQ:2026-10-02:{}:{}:{}",f["startedMs"],f["cvd"]["regular"]["sinceMs"],f["cvd"]["regular"]["gaps"]),bar:now-120000,sign:side as i8,count:1};
                tracker.apply(&f,&mut r,now);assert_eq!(r.eligible,accept,"{}",r.quick_context);
            }
        }
    }
    #[test]
    fn aggressive_engine_latency_is_bounded() {
        let(f,r,now)=sample(1.);let mut tracker=Tracker::default();let start=std::time::Instant::now();
        for _ in 0..1000{tracker.apply(&f,&mut r.clone(),now);}
        let average=start.elapsed().as_secs_f64()*1000./1000.;
        println!("EMA9 agresivo, 240 minutos: {average:.3} ms/ciclo");assert!(average<20.);
    }
}
