//! Native closed-minute counterpart of the chart's recovery/test/continuation.
//! Additional confirmation only: never creates an entry or enables a broker.
use crate::intraday_strategy::Reading;
use serde_json::{json, Value};
use snipers_core::calendar::{self, Day};
#[derive(Clone, Debug, PartialEq)]
struct Bar {
    t: u64,
    o: f64,
    h: f64,
    l: f64,
    c: f64,
    v: f64,
}
#[derive(Clone)]
struct Level {
    key: String,
    p: f64,
    known: u64,
}
#[derive(Clone)]
struct Candidate {
    anchor: Level,
    side: f64,
    impulse: Bar,
    test: Option<Bar>,
    follow: Option<Bar>,
    target: Option<f64>,
}
#[derive(Clone, Default)]
pub struct Tracker {
    identity: String,
    bars: Vec<Bar>,
    known: std::collections::BTreeMap<String, u64>,
    candidate: Option<Candidate>,
    cvd_bar: u64,
    cvd_sign: i8,
    cvd_count: u32,
    cvd_segment: String,
}
fn n(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}
fn fresh(v: &Value, now: u64, age: u64) -> bool {
    v.as_u64().is_some_and(|t| t <= now && now - t <= age)
}
fn median(mut a: Vec<f64>) -> f64 {
    a.sort_by(f64::total_cmp);
    let k = a.len() / 2;
    if a.len() % 2 == 0 {
        (a[k - 1] + a[k]) / 2.
    } else {
        a[k]
    }
}
fn bars(frame: &Value, now: u64) -> Vec<Bar> {
    let Some(date) = calendar::date_at(now) else {
        return vec![];
    };
    let Day::Open(day) = calendar::schedule(date) else {
        return vec![];
    };
    let mut out = std::collections::BTreeMap::new();
    for v in frame["chartHistory"]["minutes"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let Some(t) = v["timestampMs"].as_u64() else {
            continue;
        };
        if t < day.open_ms
            || t >= day.close_ms
            || (t - day.open_ms) % 60000 != 0
            || t.checked_add(60000).is_none_or(|end| end > now)
        {
            continue;
        }
        let (Some(o), Some(h), Some(l), Some(c), Some(volume)) = (
            n(&v["open"]),
            n(&v["high"]),
            n(&v["low"]),
            n(&v["close"]),
            n(&v["volume"]),
        ) else {
            continue;
        };
        if l <= 0. || o <= 0. || c <= 0. || h < o.max(c) || l > o.min(c) || h < l || volume < 0. {
            continue;
        }
        out.insert(
            t,
            Bar {
                t,
                o,
                h,
                l,
                c,
                v: volume,
            },
        );
    }
    out.into_values().collect()
}
impl Tracker {
    fn reset(&mut self) {
        *self = Self::default();
    }
    fn levels(&mut self, bs: &[Bar], now: u64) -> Vec<Level> {
        let mut raw = Vec::new();
        let Day::Open(day) = calendar::schedule(calendar::date_at(now).unwrap()) else {
            return vec![];
        };
        let opening: Vec<_> = bs.iter().filter(|b| b.t < day.open_ms + 900000).collect();
        if opening.len() == 15
            && opening
                .iter()
                .enumerate()
                .all(|(i, b)| b.t == day.open_ms + i as u64 * 60000)
        {
            raw.push((
                "opening-high".into(),
                opening
                    .iter()
                    .map(|b| b.h)
                    .fold(f64::NEG_INFINITY, f64::max),
            ));
            raw.push((
                "opening-low".into(),
                opening.iter().map(|b| b.l).fold(f64::INFINITY, f64::min),
            ));
        }
        for group in bs.windows(5) {
            if !group.windows(2).all(|p| p[1].t - p[0].t == 60000) {
                continue;
            }
            let b = &group[2];
            if group.iter().enumerate().all(|(i, v)| i == 2 || v.h < b.h) {
                raw.push((format!("pivot-high:{}", b.t), b.h));
            }
            if group.iter().enumerate().all(|(i, v)| i == 2 || v.l > b.l) {
                raw.push((format!("pivot-low:{}", b.t), b.l));
            }
        }
        raw.into_iter()
            .map(|(key, p)| {
                let known = *self.known.entry(key.clone()).or_insert(now);
                Level { key, p, known }
            })
            .collect()
    }
    fn cvd(&mut self, f: &Value, k: &Candidate, bar: u64, now: u64) -> bool {
        let c = &f["cvd"];
        let w = &c["recentFive"];
        let regular = &c["regular"];
        let day = calendar::date_at(now)
            .map(|d| d.to_string())
            .unwrap_or_default();
        let valid = c["available"] == true
            && c["symbol"] == f["symbol"]
            && c["date"] == day
            && fresh(&c["at"], now, 3000)
            && fresh(&regular["lastMs"], now, 15000)
            && w["complete"] == true
            && fresh(&w["endMs"], now, 3000)
            && w["endMs"]
                .as_u64()
                .zip(w["startMs"].as_u64())
                .is_some_and(|(end, start)| {
                    end.checked_sub(start) == Some(300000)
                        && regular["sinceMs"].as_u64().is_some_and(|t| t <= start)
                        && calendar::date_at(start) == calendar::date_at(now)
                        && calendar::date_at(now).is_some_and(
                            |d| matches!(calendar::schedule(d),Day::Open(s) if start>=s.open_ms),
                        )
                })
            && regular["gaps"].as_u64().is_some();
        let Some((buy, sell, unknown, delta)) = n(&w["buy"])
            .zip(n(&w["sell"]))
            .zip(n(&w["unknown"]))
            .zip(n(&w["delta"]))
            .map(|(((a, b), c), d)| (a, b, c, d))
        else {
            self.cvd_count = 0;
            return false;
        };
        let total = buy + sell + unknown;
        let sign = if delta > unknown {
            1
        } else if -delta > unknown {
            -1
        } else {
            0
        };
        if !valid
            || buy.min(sell).min(unknown) < 0.
            || total <= 0.
            || !total.is_finite()
            || (buy + sell) / total < 0.7
            || (delta - buy + sell).abs() > 1e-6_f64.max(delta.abs() * 1e-9)
            || sign == 0
        {
            self.cvd_count = 0;
            self.cvd_bar = 0;
            return false;
        }
        let segment = format!(
            "{}:{}:{}:{}",
            k.anchor.key, k.impulse.t, regular["sinceMs"], regular["gaps"]
        );
        if segment != self.cvd_segment {
            self.cvd_count = 0;
            self.cvd_bar = 0;
            self.cvd_segment = segment;
        }
        if bar > self.cvd_bar {
            self.cvd_count =
                if self.cvd_sign == sign && self.cvd_bar.checked_add(60000) == Some(bar) {
                    self.cvd_count + 1
                } else {
                    1
                };
            self.cvd_bar = bar;
            self.cvd_sign = sign;
        } else if sign != self.cvd_sign {
            self.cvd_sign = sign;
            self.cvd_count = 0;
        }
        self.cvd_count >= 2 && f64::from(sign) == k.side
    }
    pub fn apply(&mut self, f: &Value, r: &mut Reading, now: u64) {
        if !r.quick_context.is_object() {
            r.quick_context = json!({});
        }
        let identity = format!(
            "{}:{:?}:{}",
            r.symbol,
            calendar::date_at(now),
            f["startedMs"]
        );
        let valid = f["symbol"] == r.symbol
            && fresh(&f["publishedMs"], now, 3000)
            && f["stocks"]["phase"] == "streaming"
            && f["metrics"]["phase"] == "Sesión RTH"
            && r.at <= now
            && now - r.at <= 5000;
        let session = calendar::date_at(now).and_then(|d| match calendar::schedule(d) {
            Day::Open(s) => Some(s),
            _ => None,
        });
        if !valid || session.is_none_or(|s| now < s.open_ms || now >= s.close_ms) {
            self.reset();
            r.quick_context["chartRoute"] = json!({"state":"waiting","reason":"Sesión o precio sin vigencia","strategyConnected":true});
            return;
        }
        if self.identity != identity {
            self.reset();
            self.identity = identity;
        }
        let bs = bars(f, now);
        let Some(last) = bs.last().cloned() else {
            self.reset();
            return;
        };
        let corrected = self
            .bars
            .iter()
            .any(|b| bs.iter().find(|v| v.t == b.t) != Some(b));
        if now - last.t - 60000 > 90000 || corrected {
            self.reset();
            r.quick_context["chartRoute"] = json!({"state":"waiting","reason":"Cierres caducados o corregidos","strategyConnected":true});
            return;
        }
        let levels = self.levels(&bs, now);
        let usable: Vec<_> = levels
            .iter()
            .filter(|l| l.known <= last.t)
            .cloned()
            .collect();
        let mut state = "waiting";
        let mut reason = "Esperando una secuencia nueva en vivo";
        if self.bars.is_empty() {
            let start = bs.len().saturating_sub(20);
            self.bars = bs[start..].to_vec();
            if let Some(i) = self.bars.windows(2).rposition(|p| p[1].t - p[0].t != 60000) {
                self.bars = self.bars[i + 1..].to_vec();
            }
        } else if self.bars.last().is_some_and(|b| last.t > b.t) {
            if self.bars.last().is_some_and(|b| last.t - b.t != 60000) {
                self.candidate = None;
                self.cvd_count = 0;
                self.bars.clear();
            }
            if let Some(mut k) = self.candidate.take() {
                let tol = k.anchor.p * 0.0003;
                if !usable
                    .iter()
                    .any(|l| l.key == k.anchor.key && l.p == k.anchor.p)
                    || (last.c - k.anchor.p) * k.side < -tol
                    || last.t - k.impulse.t > 600000
                {
                    state = "invalidated";
                    reason = "Referencia perdida o secuencia vencida";
                } else if k.target.is_some_and(|t| (last.c - t) * k.side >= 0.) {
                    state = "reached";
                    reason = "Referencia alcanzada";
                } else {
                    if k.test.is_none()
                        && last.l <= k.anchor.p + tol
                        && last.h >= k.anchor.p - tol
                        && (if k.side > 0. {
                            last.l > k.impulse.l
                        } else {
                            last.h < k.impulse.h
                        })
                        && (last.c - k.anchor.p) * k.side > 0.
                        && last.v < k.impulse.v
                        && last.h - last.l <= k.impulse.h - k.impulse.l
                    {
                        k.test = Some(last.clone());
                    } else if let Some(test) = &k.test {
                        if k.target.is_none()
                            && (last.c - test.c) * k.side > tol
                            && last.v >= test.v
                        {
                            let limited = limited_backing(f, last.c, k.side, k.anchor.p, now);
                            if limited
                                && k.follow
                                    .as_ref()
                                    .is_none_or(|b| (last.c - b.c) * k.side < -tol)
                            {
                                k.follow = Some(last.clone());
                            } else {
                                k.target = usable
                                    .iter()
                                    .filter(|l| (l.p - last.c) * k.side > tol)
                                    .min_by(|a, b| {
                                        (a.p - last.c).abs().total_cmp(&(b.p - last.c).abs())
                                    })
                                    .map(|l| l.p);
                            }
                        } else if k.target.is_none() {
                            k.follow = None;
                        }
                    }
                    self.candidate = Some(k);
                }
            } else if self.bars.len() >= 8 {
                let vol = median(self.bars.iter().map(|b| b.v).collect());
                let spread = median(self.bars.iter().map(|b| b.h - b.l).collect());
                let mut near = usable.clone();
                near.sort_by(|a, b| (a.p - last.c).abs().total_cmp(&(b.p - last.c).abs()));
                'find: for l in near {
                    for side in [1., -1.] {
                        let tol = l.p * 0.0003;
                        let location = if last.h > last.l {
                            (last.c - last.l) / (last.h - last.l)
                        } else {
                            0.5
                        };
                        let crossed = if side > 0. {
                            last.l < l.p - tol && last.c > l.p + tol && location >= 0.7
                        } else {
                            last.h > l.p + tol && last.c < l.p - tol && location <= 0.3
                        };
                        if crossed
                            && vol > 0.
                            && spread > 0.
                            && last.v >= vol * 1.2
                            && last.h - last.l >= spread
                        {
                            self.candidate = Some(Candidate {
                                anchor: l.clone(),
                                side,
                                impulse: last.clone(),
                                test: None,
                                follow: None,
                                target: None,
                            });
                            break 'find;
                        }
                    }
                }
            }
            self.bars.push(last.clone());
            if self.bars.len() > 20 {
                self.bars.remove(0);
            }
        }
        let mut target = None;
        let mut stop = None;
        let mut rr = None;
        let mut sign = 0.;
        let mut confirmed = false;
        if let Some(k) = self.candidate.clone() {
            sign = k.side;
            target = k.target;
            stop = Some(k.anchor.p - k.side * k.anchor.p * 0.0003);
            state = if k.target.is_some() {
                "sustained"
            } else if k.test.is_some() {
                "test"
            } else {
                "recovery"
            };
            let cvd = if k.test.is_some() {
                self.cvd(f, &k, last.t, now)
            } else {
                self.cvd_count = 0;
                false
            };
            if let Some(t) = target {
                let risk = (r.spot - stop.unwrap()) * sign;
                let reward = (t - r.spot) * sign;
                rr = (risk > 0. && reward > 0.).then_some(reward / risk);
                let barrier = strong_obstacle(f, r.spot, t, sign, now);
                confirmed = cvd
                    && rr.is_some_and(|v| v >= 1.8)
                    && !barrier
                    && (r.spot - last.c) * sign >= 0.
                    && r.flow_one * sign >= 0.
                    && r.flow_five * sign >= 0.;
                reason = if confirmed {
                    "Recuperación, prueba, continuación y CVD confirmados"
                } else {
                    "Esperando CVD, recorrido 1,8R o reacción ante muro importante"
                };
            }
        }
        let side = if sign > 0. {
            "CALL"
        } else if sign < 0. {
            "PUT"
        } else {
            "PENDIENTE"
        };
        let baseline = r.eligible;
        // Existing admission and risk management remain authoritative. A confirmed
        // chart sequence contributes its target only to an already eligible same-side entry.
        if confirmed && baseline {
            if r.entry_side == side {
                r.next = target;
            } else {
                r.eligible = false;
                r.blockers
                    .push("Ruta intradía de la gráfica confirmada en dirección contraria".into());
            }
        }
        r.quick_context["chartRoute"] = json!({"version":"bot-chart-route-v1","key":self.candidate.as_ref().map(|k|format!("{}:{}:{}",self.identity,k.anchor.key,k.impulse.t)),"symbol":r.symbol,"at":now,"state":state,"side":side,"target":target,"invalidation":stop,"rewardRisk":rr,"confirmed":confirmed,"reason":reason,"cvdCloses":self.cvd_count,"baselineEligible":baseline,"finalEligible":r.eligible,"strategyConnected":true,"changesRiskLevels":false,"atr14FiveMinute":atr(&bs,now),"targetAtr":atr(&bs,now).zip(target).map(|(a,t)|(t-r.spot).abs()/a),"executionEnabled":false});
    }
}
fn structure(f: &Value, now: u64) -> Option<&Value> {
    let snap = &f["optionSnapshotStatus"];
    if snap["phase"] != "ready" || !fresh(&snap["observedMs"], now, 300000) {
        return None;
    }
    let date = calendar::date_at(now)?.to_string();
    let hs: Vec<_> = snap["horizons"]
        .as_array()?
        .iter()
        .filter(|h| {
            h["mode"] == "DAY"
                && h["expirations"]
                    .as_array()
                    .is_some_and(|ds| ds.len() == 1 && ds[0] == date)
                && h["observedMs"] == snap["observedMs"]
        })
        .collect();
    (hs.len() == 1).then(|| &hs[0]["structure"])
}
fn limited_backing(f: &Value, spot: f64, side: f64, anchor: f64, now: u64) -> bool {
    let Some(s) = structure(f, now) else {
        return false;
    };
    let weak = |p: Option<f64>, score: Option<f64>| {
        p.zip(score)
            .is_some_and(|(p, v)| (p == anchor || (spot - p) * side >= 0.) && v < 60.)
    };
    let wall = &s[if side > 0. { "putWall" } else { "callWall" }];
    if weak(n(&wall["strike"]), n(&wall["score"]))
        || ["callWall", "putWall"].iter().any(|key| {
            n(&s[key]["strike"]) == Some(anchor) && n(&s[key]["score"]).is_some_and(|v| v < 60.)
        })
    {
        return true;
    }
    if s["routes"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|v| n(&v["barrier"]) == Some(anchor) && n(&v["barrierScore"]).is_some_and(|v| v < 60.))
    {
        return true;
    }
    let d = &f["levelDynamics"];
    d["symbol"] == f["symbol"]
        && d["available"] == true
        && fresh(&d["at"], now, 3000)
        && d["levels"].as_array().into_iter().flatten().any(|l| {
            l["provisional"] != true
                && n(&l["strike"]).is_some_and(|p| {
                    p == anchor
                        || ((spot - p) * side >= 0.
                            && l["name"] == if side > 0. { "PUT WALL" } else { "CALL WALL" })
                })
                && n(&l["change"]).is_some_and(|v| v <= -5.)
        })
}
pub(crate) fn strong_obstacle(f: &Value, spot: f64, target: f64, side: f64, now: u64) -> bool {
    let Some(s) = structure(f, now) else {
        return false;
    };
    if !s["provisional"].is_null() {
        return false;
    }
    let check = |p: Option<f64>, score: Option<f64>| {
        p.zip(score).is_some_and(|(p, score)| {
            score >= 60. && (p - spot) * side > spot * 0.0003 && (target - p) * side > spot * 0.0003
        })
    };
    if ["callWall", "putWall"]
        .iter()
        .any(|key| check(n(&s[key]["strike"]), n(&s[key]["score"])))
    {
        return true;
    }
    s["routes"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|v| check(n(&v["barrier"]), n(&v["barrierScore"])))
}
fn atr(bs: &[Bar], now: u64) -> Option<f64> {
    let Day::Open(day) = calendar::schedule(calendar::date_at(now)?) else {
        return None;
    };
    let end = day.open_ms + (now - day.open_ms) / 300000 * 300000;
    let start = end.checked_sub(4500000)?;
    if start < day.open_ms {
        return None;
    }
    let mut five = Vec::new();
    for t in (start..end).step_by(300000) {
        let group: Vec<_> = bs.iter().filter(|b| b.t >= t && b.t < t + 300000).collect();
        if group.len() != 5
            || !group
                .iter()
                .enumerate()
                .all(|(i, b)| b.t == t + i as u64 * 60000)
        {
            return None;
        }
        five.push((
            group.iter().map(|b| b.h).fold(f64::NEG_INFINITY, f64::max),
            group.iter().map(|b| b.l).fold(f64::INFINITY, f64::min),
            group[4].c,
        ));
    }
    let value = five
        .windows(2)
        .map(|p| {
            (p[1].0 - p[1].1)
                .max((p[1].0 - p[0].2).abs())
                .max((p[1].1 - p[0].2).abs())
        })
        .sum::<f64>()
        / 14.;
    (value.is_finite() && value > 0.).then_some(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn open() -> u64 {
        calendar::legacy_eastern_ms("2026-10-09 09:30:00").unwrap()
    }
    fn candle(i: u64, l: f64, h: f64, c: f64, v: f64) -> Value {
        json!({"timestampMs":open()+i*60000,"open":c,"high":h,"low":l,"close":c,"volume":v})
    }
    fn frame(i: u64) -> Value {
        let now = open() + (i + 1) * 60000;
        let mut bs: Vec<_> = (0..=i.min(20))
            .map(|t| candle(t, 98., 100., 99., 100.))
            .collect();
        bs[15] = candle(15, 98., 105., 99., 100.);
        if i >= 21 {
            bs.push(candle(21, 99., 102., 101.8, 200.));
        }
        if i >= 22 {
            bs.push(candle(22, 99.99, 101.9, 100.2, 50.));
        }
        if i >= 23 {
            bs.push(candle(23, 100.1, 101.8, 101.2, 100.));
        }
        json!({"symbol":"QQQ","startedMs":open(),"publishedMs":now,"stocks":{"phase":"streaming"},"metrics":{"phase":"Sesión RTH"},"chartHistory":{"minutes":bs},"cvd":{"symbol":"QQQ","date":"2026-10-09","at":now,"available":true,"regular":{"sinceMs":open(),"lastMs":now,"gaps":0},"recentFive":{"startMs":now-300000,"endMs":now,"buy":100.,"sell":10.,"unknown":5.,"delta":90.,"complete":true}}})
    }
    fn reading(i: u64) -> Reading {
        Reading {
            symbol: "QQQ".into(),
            at: open() + (i + 1) * 60000,
            spot: if i == 23 { 101.2 } else { 99. },
            entry_side: "CALL".into(),
            eligible: true,
            ready: true,
            next: Some(110.),
            flow_one: 0.2,
            flow_five: 0.2,
            quick_context: json!({}),
            ..Default::default()
        }
    }
    fn run(t: &mut Tracker, i: u64) -> Reading {
        let mut r = reading(i);
        let at = r.at;
        t.apply(&frame(i), &mut r, at);
        r
    }
    #[test]
    fn same_sequence_as_chart_requires_later_test_and_two_distinct_cvd_closes() {
        let mut t = Tracker::default();
        assert_eq!(
            run(&mut t, 20).quick_context["chartRoute"]["state"],
            "waiting"
        );
        assert_eq!(
            run(&mut t, 21).quick_context["chartRoute"]["state"],
            "recovery"
        );
        let test = run(&mut t, 22);
        assert_eq!(test.quick_context["chartRoute"]["state"], "test");
        assert_eq!(test.next, Some(110.));
        let r = run(&mut t, 23);
        assert_eq!(r.quick_context["chartRoute"]["confirmed"], true);
        assert_eq!(r.next, Some(105.));
        assert_eq!(r.quick_context["chartRoute"]["cvdCloses"], 2);
        let again = run(&mut t, 23);
        assert_eq!(again.quick_context["chartRoute"]["cvdCloses"], 2);
    }
    #[test]
    fn no_replay_and_no_promotion_of_ineligible_baseline() {
        let mut t = Tracker::default();
        let r = run(&mut t, 23);
        assert_eq!(r.next, Some(110.));
        assert_eq!(r.quick_context["chartRoute"]["confirmed"], false);
        let mut t = Tracker::default();
        for i in 20..23 {
            run(&mut t, i);
        }
        let mut r = reading(23);
        r.eligible = false;
        let at = r.at;
        t.apply(&frame(23), &mut r, at);
        assert!(!r.eligible);
        assert_eq!(r.next, Some(110.));
    }
    #[test]
    fn confirmed_opposite_sequence_blocks_admission() {
        let mut t = Tracker::default();
        for i in 20..23 {
            run(&mut t, i);
        }
        let mut r = reading(23);
        r.entry_side = "PUT".into();
        let at = r.at;
        t.apply(&frame(23), &mut r, at);
        assert!(!r.eligible);
        assert_eq!(r.next, Some(110.));
    }
    #[test]
    fn stale_incomplete_foreign_and_corrected_evidence_cannot_set_target() {
        for mode in 0..5 {
            let mut t = Tracker::default();
            for i in 20..23 {
                run(&mut t, i);
            }
            let mut f = frame(23);
            let mut r = reading(23);
            match mode {
                0 => f["publishedMs"] = json!(r.at - 3001),
                1 => f["cvd"]["recentFive"]["complete"] = json!(false),
                2 => f["symbol"] = json!("SPY"),
                3 => f["chartHistory"]["minutes"][10]["volume"] = json!(101.),
                _ => f["cvd"]["recentFive"]["unknown"] = json!(1000.),
            }
            let at = r.at;
            t.apply(&f, &mut r, at);
            assert_eq!(r.next, Some(110.), "mode {mode}");
        }
    }
    #[test]
    fn strong_intermediate_wall_blocks_arrival() {
        let mut t = Tracker::default();
        for i in 20..23 {
            run(&mut t, i);
        }
        let mut f = frame(23);
        let mut r = reading(23);
        f["optionSnapshotStatus"] = json!({"phase":"ready","observedMs":r.at,"horizons":[{"mode":"DAY","observedMs":r.at,"expirations":["2026-10-09"],"structure":{"callWall":{"strike":102.,"score":80.}}}]});
        let at = r.at;
        t.apply(&f, &mut r, at);
        assert_eq!(r.quick_context["chartRoute"]["confirmed"], false);
        assert_eq!(r.next, Some(110.));
    }
    #[test]
    fn put_sequence_has_symmetric_target_and_confirmation() {
        let mut t = Tracker::default();
        for i in 20..=23 {
            let mut f = frame(i);
            for b in f["chartHistory"]["minutes"].as_array_mut().unwrap() {
                let h = b["high"].as_f64().unwrap();
                let l = b["low"].as_f64().unwrap();
                b["high"] = json!(200. - l);
                b["low"] = json!(200. - h);
                b["open"] = json!(200. - b["open"].as_f64().unwrap());
                b["close"] = json!(200. - b["close"].as_f64().unwrap());
            }
            f["cvd"]["recentFive"]["buy"] = json!(10.);
            f["cvd"]["recentFive"]["sell"] = json!(100.);
            f["cvd"]["recentFive"]["delta"] = json!(-90.);
            let mut r = reading(i);
            r.spot = 200. - r.spot;
            r.entry_side = "PUT".into();
            r.flow_one = -0.2;
            r.flow_five = -0.2;
            let at = r.at;
            t.apply(&f, &mut r, at);
            if i == 23 {
                assert_eq!(r.quick_context["chartRoute"]["confirmed"], true);
                assert_eq!(r.next, Some(95.));
            }
        }
    }
    #[test]
    fn disconnect_and_gap_require_a_new_sequence() {
        let mut t = Tracker::default();
        for i in 20..23 {
            run(&mut t, i);
        }
        let mut f = frame(23);
        f["stocks"]["phase"] = json!("disconnected");
        let mut r = reading(23);
        let at = r.at;
        t.apply(&f, &mut r, at);
        assert!(t.candidate.is_none());
        assert_eq!(run(&mut t, 23).next, Some(110.));
        let mut t = Tracker::default();
        run(&mut t, 20);
        run(&mut t, 21);
        assert_eq!(run(&mut t, 23).next, Some(110.));
        assert!(t.candidate.is_none());
    }
    #[test]
    fn atr_matches_chart_sma_of_fourteen_five_minute_true_ranges() {
        let now = open() + 75 * 60000;
        let bs = (0..75)
            .map(|i| Bar {
                t: open() + i * 60000,
                o: 100.,
                h: 101.,
                l: 99.,
                c: 100.,
                v: 1.,
            })
            .collect::<Vec<_>>();
        assert_eq!(atr(&bs, now), Some(2.));
        assert_eq!(atr(&bs[..74], now), None);
        assert_eq!(atr(&bs, open() + 74 * 60000), None);
    }
}
