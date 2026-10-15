//! Paper strategy derived from the descriptive intraday model. No broker side effects.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use snipers_core::calendar::{date_at, Day};

pub const VERSION: &str = "intraday-control-v1";
pub const PAPER_MIN_STRENGTH: f64 = 0.35;
pub fn snapshot_input(s:&crate::live::LiveState)->Value {
    let opening=snipers_core::calendar::date_at(s.published_ms).and_then(|date|match snipers_core::calendar::schedule(date){Day::Open(session)=>s.minutes.get(&session.open_ms),_=>None});
    json!({"optionTiming":s.option_timing,"gapReport":s.gap_report,"rthOpen":opening,"symbol":s.symbol,"startedMs":s.started_ms,"publishedMs":s.published_ms,
        "ticks":s.ticks,"stocks":s.stocks,"options":s.options,"metrics":s.metrics,
        "cvd":s.cvd,"levelDynamics":s.level_dynamics,"chartHistory":s.chart_history,"flow":s.flow,"localVolume":s.local_volume,
        "optionSnapshotStatus":s.option_snapshot_status,"greekSupport":s.greek_support,"neuroContext":s.neuro_context})
}
fn num(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}
fn positive(v: &Value) -> Option<f64> {
    num(v).filter(|n| *n > 0.)
}
fn ratio(a: &Value, b: &Value) -> Option<f64> {
    let (a, b) = (num(a)?, num(b)?);
    (a >= 0. && b >= 0. && a + b > 0.).then(|| ((a - b) / (a + b)).clamp(-1., 1.))
}
fn recent(v: &Value, now: u64, age: u64) -> bool {
    v.as_u64()
        .is_some_and(|t| t <= now && now - t <= age && date_at(t) == date_at(now))
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Reading {
    #[serde(default)]
    pub quick_context:Value,
    #[serde(default)]
    pub neuro_context: crate::neuro_context::Context,
    #[serde(default)]
    pub swing_bars: Vec<Value>,
    #[serde(default)]
    pub daily_range: Option<f64>,
    #[serde(default)]
    pub flow_expiry: String,
    #[serde(default)]
    pub structural_entry: StructuralEntry,
    #[serde(default)]
    pub entry_side: String,
    #[serde(default)]
    pub entry_basis: String,
    #[serde(default)]
    pub magnet_reclaim:snipers_core::magnet_reclaim::Read,
    #[serde(default)]
    pub paper_partial: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub greek_support: snipers_core::greek_support::Context,
    pub symbol: String,
    pub session: String,
    pub at: u64,
    pub evidence_ms: u64,
    pub observation: String,
    pub spot: f64,
    pub net: f64,
    pub coverage: f64,
    pub price: f64,
    pub flow_one: f64,
    pub flow_five: f64,
    pub ready: bool,
    pub management_ready: bool,
    pub eligible: bool,
    pub next: Option<f64>,
    #[serde(default)]
    pub vwap: Option<f64>,
    #[serde(default)]
    pub levels: Vec<f64>,
    #[serde(default)]
    pub closed_bars: Vec<Value>,
    #[serde(default)]
    pub ema: crate::bot_ema::EmaRead,
    pub components: Vec<(String, f64, f64)>,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct StructuralEntry {
    pub eligible: bool,
    pub side: String,
    pub call_strength: Option<f64>,
    pub put_strength: Option<f64>,
    pub pattern: snipers_core::magnet_reclaim::Read,
    pub blockers: Vec<String>,
    pub key: String,
}
fn structural_entry(structure: &Value, bars: &[Bar], spot: f64, now: u64, ready: bool, f1: Option<f64>, f5: Option<f64>) -> StructuralEntry {
    let score=|field:&str| num(&structure[field]["score"]).filter(|n|(0.0..=100.0).contains(n));
    let mut result=StructuralEntry {call_strength:score("callWall"),put_strength:score("putWall"),..Default::default()};
    let candles=bars.iter().map(|b|snipers_core::magnet_reclaim::Bar{start_ms:b.t,open:b.open,high:b.high,low:b.low,close:b.close}).collect::<Vec<_>>();
    for (side,field,direction) in [("CALL","callWall",1.),("PUT","putWall",-1.)] {
        let pattern=snipers_core::magnet_reclaim::evaluate_side(&candles,positive(&structure["selected"]["M1"]),positive(&structure[field]["strike"]),now,direction>0.);
        if !pattern.detected {continue;}
        result.side=side.into();
        result.key=format!("{}:{}:{:?}:{:?}",side,bars.last().map(|b|b.t).unwrap_or(0),pattern.magnet,pattern.target);
        let target=pattern.target.unwrap();
        let stop=pattern.invalidation.unwrap();
        let risk=(spot-stop)*direction;
        let reward=(target-spot)*direction;
        for (ok,reason) in [
            (ready,"Faltan datos vigentes y cobertura para evaluar la estructura"),
            (score(field).is_some_and(|s|s>=60.),"Muro objetivo sin fuerza verificada de al menos 60/100"),
            ((spot-pattern.magnet.unwrap())*direction>0.,"El precio volvió al lado incorrecto del imán"),
            (risk>0.&&reward>0.&&reward/risk>=1.8,"Recorrido actual inferior a 1,8 veces la invalidación"),
            (f1.zip(f5).is_some_and(|(a,b)|a*direction> -0.35&&b*direction> -0.35),"Flujo ausente o fuertemente contrario al rebote"),
        ] {if !ok {result.blockers.push(reason.into());}}
        // Unknown route coverage is not equivalent to a clear path.
        let route=structure["routes"].as_array().and_then(|rows|rows.iter().find(|v|v["direction"]==side));
        if let Some(route)=route.filter(|v|v["structuralReady"]==true) {
            if positive(&route["barrier"]).is_some_and(|level|(level-spot)*direction>0.&&(target-level)*direction>0.
                &&num(&route["barrierScore"]).is_none_or(|s|s>=60.||s<0.)) {
                result.blockers.push("Obstáculo intermedio fuerte o sin fuerza verificada".into());
            }
        } else {result.blockers.push("Ruta hacia el muro todavía no verificada".into());}
        result.pattern=pattern;
        result.eligible=result.blockers.is_empty();
        break;
    }
    result
}
#[derive(Clone)]
struct Bar {
    t: u64,
    open: f64,
    close: f64,
    low: f64,
    high: f64,
}
fn bars(s: &Value, now: u64) -> Vec<Bar> {
    let Some(date) = date_at(now) else {
        return vec![];
    };
    let Day::Open(day) = snipers_core::calendar::schedule(date) else {
        return vec![];
    };
    let mut minutes = std::collections::BTreeMap::new();
    for v in s["chartHistory"]["minutes"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let Some(t) = v["timestampMs"].as_u64() else {
            continue;
        };
        if t < day.open_ms || t + 60000 > now.min(day.close_ms) || (t - day.open_ms) % 60000 != 0 {
            continue;
        }
        let (Some(open), Some(close), Some(low), Some(high), Some(volume)) = (
            positive(&v["open"]),
            positive(&v["close"]),
            positive(&v["low"]),
            positive(&v["high"]),
            num(&v["volume"]),
        ) else {
            continue;
        };
        if low > open.min(close) || high < open.max(close) || volume < 0. {
            continue;
        }
        minutes.insert(
            t,
            Bar {
                t,
                open,
                close,
                low,
                high,
            },
        );
    }
    let mut out = vec![];
    for (t, a) in &minutes {
        if (t - day.open_ms) % 120000 != 0 {
            continue;
        }
        if let Some(b) = minutes.get(&(t + 60000)) {
            out.push(Bar {
                t: *t,
                open: a.open,
                close: b.close,
                low: a.low.min(b.low),
                high: a.high.max(b.high),
            });
        }
    }
    if out.last().is_none_or(|b| now - b.t > 300000) {
        return vec![];
    }
    if let Some(gap) = out.windows(2).rposition(|w| w[1].t - w[0].t != 120000) {
        out.drain(..=gap);
    }
    out.into_iter()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}
fn swing_bars(s:&Value,now:u64)->Vec<Value>{
    let Some(date)=date_at(now) else {return vec![]};let Day::Open(day)=snipers_core::calendar::schedule(date) else{return vec![]};
    let mut rows=std::collections::BTreeMap::new();
    for b in s["chartHistory"]["minutes"].as_array().into_iter().flatten(){
        let Some(t)=b["timestampMs"].as_u64().filter(|t|*t>=day.open_ms&&*t+60000<=now.min(day.close_ms)&&(*t-day.open_ms)%60000==0) else {continue};
        let (Some(o),Some(h),Some(l),Some(c))=(positive(&b["open"]),positive(&b["high"]),positive(&b["low"]),positive(&b["close"])) else {continue};
        if l<=o.min(c)&&h>=o.max(c){rows.insert(t,(o,h,l,c));}
    }
    let mut out=vec![];
    for (&t,b) in &rows {if (t-day.open_ms)%1_800_000!=0{continue;}
        let parts:Vec<_>=(0..30).filter_map(|i|rows.get(&(t+i*60000))).collect();if parts.len()!=30{continue;}
        out.push(json!({"t":t,"open":b.0,"high":parts.iter().map(|b|b.1).fold(f64::NEG_INFINITY,f64::max),"low":parts.iter().map(|b|b.2).fold(f64::INFINITY,f64::min),"close":parts.last().unwrap().3}));
    }out
}
pub fn read(s: &Value, now: u64) -> Reading {
    with_neuro(read_mode(s, now, false))
}
/// Explicit Paper-only experiment: observed samples are not full-market coverage.
pub fn read_paper(s: &Value, now: u64) -> Reading {
    with_neuro(read_mode(s, now, true))
}
// A closed rejection candle plus a live breakout: never authorize on a touch alone.
fn vwap_rebound(bars: &[Bar], spot: f64, vwap: f64, levels: &[f64], f1: f64, f5: f64) -> Option<(&'static str, f64)> {
    let last = bars.last()?;
    let prior = bars.iter().rev().nth(1)?;
    if last.t != prior.t + 120000 { return None; }
    for (side, direction) in [("CALL", 1.0), ("PUT", -1.0)] {
        let extreme = if direction > 0. { last.low } else { last.high };
        let trigger = if direction > 0. { last.high } else { last.low };
        let rejection = (last.close - last.open) * direction > 0.
            && (last.close - vwap) * direction > 0.
            && (extreme - vwap).abs() / vwap <= 0.0003
            && (spot - trigger) * direction > 0.;
        let target = levels.iter().copied().filter(|p| (p - spot) * direction > 0.)
            .min_by(|a,b| (a-spot).abs().total_cmp(&(b-spot).abs()));
        let risk = (spot - extreme) * direction;
        if rejection && f1 * direction >= 0.15 && f5 * direction >= -0.15
            && risk > 0. && target.is_some_and(|p| (p-spot)*direction/risk >= 1.8) {
            return Some((side, target.unwrap()));
        }
    }
    None
}
fn with_neuro(mut r:Reading)->Reading {
    if r.neuro_context.available {r.warnings.push(r.neuro_context.narrative(&r.entry_side));}
    else {r.warnings.push(format!("Neuro: {}",r.neuro_context.reason));}
    r
}
fn observed_flow(w: &Value) -> Option<Value> {
    if w["event_count"].as_u64()? < 5 {
        return None;
    }
    let premium = |label: &str| num(&w["buckets"][label]["premium"]).filter(|n| *n >= 0.);
    let bull = premium("CALL BUY")? + premium("PUT SELL")?;
    let bear = premium("PUT BUY")? + premium("CALL SELL")?;
    let unknown = premium("UNKNOWN")?;
    let classified = bull + bear;
    let total = classified + unknown;
    if !total.is_finite() || classified <= 0. || classified / total < 0.5 {
        return None;
    }
    let mut sample = w.clone();
    sample["bullish_pct"] = json!(100. * bull / classified);
    sample["bearish_pct"] = json!(100. * bear / classified);
    Some(sample)
}
fn read_mode(s: &Value, now: u64, paper_partial: bool) -> Reading {
    let mut r = Reading {
        daily_range:positive(&s["dailyRange"]),flow_expiry:s["flowExpiry"].as_str().unwrap_or("").into(),
        paper_partial,
        symbol: s["symbol"].as_str().unwrap_or("").into(),
        at: now,
        ..Default::default()
    };
    let today = date_at(now).map(|d| d.to_string()).unwrap_or_default();
    r.ema=crate::bot_ema::read(&s["chartHistory"],now);
    if let Ok(c)=serde_json::from_value::<crate::neuro_context::Context>(s["neuroContext"].clone()) {
        let tick=&s["ticks"][&r.symbol];
        r.neuro_context=c.checked(&r.symbol,now,positive(&tick["price"]).zip(tick["tradeMs"].as_u64()),s["stocks"]["phase"]=="streaming");
    }
    if let Ok(context) =
        serde_json::from_value::<snipers_core::greek_support::Context>(s["greekSupport"].clone())
    {
        if context.symbol == r.symbol && context.at <= now && now - context.at <= 3000 {
            r.greek_support = context;
        }
    }
    r.session = format!("{}:{}:{}", r.symbol, today, s["startedMs"]);
    let t = &s["ticks"][&r.symbol];
    let m = &s["metrics"];
    let snap = &s["optionSnapshotStatus"];
    let regular=date_at(now).is_some_and(|date| matches!(snipers_core::calendar::schedule(date),Day::Open(day) if now>=day.open_ms && now<day.close_ms));
    for (ok, reason) in [
        (r.symbol == "QQQ" || s["backgroundBot"]==true && snipers_core::universe::stock_asset(&r.symbol).is_some(), "Activo no habilitado para el bot"),
        (regular, "Fuera de la sesión regular"),
        (
            recent(&s["publishedMs"], now, 3000),
            "Publicación de datos ausente o vencida",
        ),
        (
            s["stocks"]["phase"] == "streaming",
            "Stocks sin conexión vigente",
        ),
        (
            s["options"]["phase"] == "streaming",
            "Opciones sin conexión vigente",
        ),
        (t["symbol"] == r.symbol, "Precio de otro activo o ausente"),
        (
            recent(&t["tradeMs"], now, 5000),
            "Última operación de QQQ no vigente (máximo 5 s)",
        ),
        (
            recent(&s["options"]["lastEventMs"], now, 15000),
            "Último evento de opciones no vigente (máximo 15 s)",
        ),
    ] {
        if !ok {
            r.blockers.push(reason.into());
        }
    }
    for (name, time) in [
        ("Precio QQQ", &t["tradeMs"]),
        ("Opciones", &s["options"]["lastEventMs"]),
    ] {
        if let Some(time) = time.as_u64().filter(|time| *time > now) {
            r.blockers.push(format!(
                "{}: reloj del proveedor adelantado {} ms",
                name,
                time - now
            ));
        }
    }
    if !r.blockers.is_empty() {
        return r;
    }
    let Some(spot) = positive(&t["price"]) else {
        r.blockers.push("Precio QQQ inválido".into());
        return r;
    };
    r.spot = spot;
    r.evidence_ms = s["options"]["lastEventMs"].as_u64().unwrap_or(0);
    let structure = snap["horizons"]
        .as_array()
        .and_then(|rows| {
            rows.iter().find(|h| {
                (h["mode"] == "DAY" && s["backgroundBot"]!=true || s["backgroundBot"]==true && h["expirations"].as_array().is_some_and(|dates|dates.len()==1&&dates[0]==s["flowExpiry"]))
                    && (h["structure"]["complete"] == true
                        || paper_partial && h["structure"].is_object())
            })
        })
        .map(|h| &h["structure"]);
    let valid_structure =
        snap["phase"] == "ready" && recent(&snap["observedMs"], now, 900000) && structure.is_some();
    let structure = if valid_structure {
        structure.unwrap()
    } else {
        &Value::Null
    };
    let use_provisional = paper_partial
        && structure["provisional"].is_object()
        && (structure["complete"] != true
            || structure["gexAvailable"] == false
            || structure["liquidityComplete"] == false);
    if paper_partial && (structure["complete"] != true || use_provisional) {
        r.warnings
            .push("Paper experimental: niveles provisionales; cadena DAY incompleta".into());
    }
    // Production stores observed levels under provisional while certified
    // fields stay empty. Only this explicitly Paper path consumes that subset.
    let structure = if use_provisional {
        &structure["provisional"]
    } else {
        structure
    };
    let mut partial_windows = s["flow"]["intraday_windows"].clone();
    if paper_partial {
        for w in partial_windows.as_array_mut().into_iter().flatten() {
            if w["premium_complete"] != true {
                if let Some(sample) = observed_flow(w) {
                    r.warnings.push(format!(
                        "Flujo {}m: muestra parcial observada, no cobertura completa",
                        w["minutes"]
                    ));
                    *w = sample;
                    // Local acceptance flag only; never rewrite the engine's completeness.
                    w["paper_sample"] = json!(true);
                }
            }
        }
    }
    let window = |minutes: u64| {
        partial_windows.as_array().and_then(|rows| {
            rows.iter().find(|w| {
                w["minutes"] == minutes
                    && w["event_count"].as_u64().unwrap_or(0) > 0
                    && (w["premium_complete"] == true || paper_partial && w["paper_sample"] == true)
                    && num(&w["bullish_pct"])
                        .zip(num(&w["bearish_pct"]))
                        .is_some_and(|(a, b)| {
                            (0.0..=100.0).contains(&a)
                                && (0.0..=100.0).contains(&b)
                                && a + b <= 100.01
                        })
            })
        })
    };
    let one = window(1).unwrap_or(&Value::Null);
    let five = window(5).unwrap_or(&Value::Null);
    let f1 = ratio(&one["bullish_pct"], &one["bearish_pct"]);
    let f5 = ratio(&five["bullish_pct"], &five["bearish_pct"]);
    let history_ok =
        m["session"] == today && m["phase"] == "Sesión RTH" && recent(&m["asofMs"], now, 180000);
    let bars = if history_ok { bars(s, now) } else { vec![] };
    r.observation = json!([one, five, bars.last().map(|b| b.t)]).to_string();
    let vwap = if history_ok {
        positive(&m["vwap"])
    } else {
        None
    };
    r.vwap = vwap;
    r.swing_bars=swing_bars(s,now);
    r.magnet_reclaim=snipers_core::magnet_reclaim::evaluate(
        &bars.iter().map(|b|snipers_core::magnet_reclaim::Bar{start_ms:b.t,open:b.open,high:b.high,low:b.low,close:b.close}).collect::<Vec<_>>(),
        positive(&structure["selected"]["M1"]),positive(&structure["callWall"]["strike"]),now);
    r.levels = [
        positive(&structure["putWall"]["strike"]),
        positive(&structure["callWall"]["strike"]),
        positive(&structure["selected"]["M1"]),
    ]
    .into_iter()
    .flatten()
    .collect();
    r.closed_bars = bars
        .iter()
        .map(|b| json!({"t":b.t,"open":b.open,"close":b.close,"high":b.high,"low":b.low}))
        .collect();
    let price = bars.last().map(|last| {
        let mut parts = vec![];
        if last.close != last.open {
            parts.push(if last.close > last.open { 0.35 } else { -0.35 });
        }
        if let Some(v) = vwap {
            let d = (last.close - v) / v * 100.;
            parts.push(if d.abs() <= 0.03 {
                0.
            } else {
                (d / 0.35).clamp(-1., 1.)
            });
        }
        let recent = bars.iter().rev().take(4).collect::<Vec<_>>();
        if recent.len() >= 3 {
            let first = recent.last().unwrap().close;
            parts.push(((last.close - first) / first * 100. / 0.35).clamp(-1., 1.));
        }
        if parts.is_empty() {
            0.
        } else {
            parts.iter().sum::<f64>() / parts.len() as f64
        }
    });
    let mut interactions: Vec<(f64, f64)> = vec![];
    if let Some(last) = bars.last() {
        let touched = |n: f64| {
            bars.iter()
                .rev()
                .take(4)
                .any(|b| b.low <= n * 1.0018 && b.high >= n * 0.9982)
        };
        for (field, put) in [("putWall", true), ("callWall", false)] {
            if let Some(level) = positive(&structure[field]["strike"]).filter(|n| touched(*n)) {
                let score =
                    72. + (num(&structure[field]["score"]).unwrap_or(0.).max(0.) * 0.18).min(18.);
                if put && last.close > level && last.low <= level * 1.0018 {
                    interactions.push((score, 1.));
                } else if put && last.close < level {
                    interactions.push((78., -1.));
                } else if !put && last.close < level && last.high >= level * 0.9982 {
                    interactions.push((score, -1.));
                } else if !put && last.close > level {
                    interactions.push((78., 1.));
                }
            }
        }
        if let Some(level) = positive(&structure["selected"]["M1"]).filter(|n| touched(*n)) {
            let previous = bars.iter().rev().nth(1);
            let (score, bias) = if last.close > level {
                (
                    if previous.is_some_and(|p| p.close <= level * 1.0018) {
                        84.
                    } else {
                        68.
                    },
                    1.,
                )
            } else if last.close < level {
                (
                    if previous.is_some_and(|p| p.close >= level * 0.9982) {
                        84.
                    } else {
                        68.
                    },
                    -1.,
                )
            } else {
                (55., 0.)
            };
            interactions.push((score, bias));
        }
    }
    for route in structure["routes"].as_array().into_iter().flatten() {
        if let (Some(level), Some(score)) =
            (positive(&route["barrier"]), num(&route["barrierScore"]))
        {
            if (level - spot).abs() / level * 100. <= 0.35 {
                interactions.push((
                    score.clamp(55., 100.),
                    if route["direction"] == "CALL" {
                        -1.
                    } else {
                        1.
                    },
                ));
            }
        }
    }
    if interactions.is_empty()
        && [
            positive(&structure["putWall"]["strike"]),
            positive(&structure["selected"]["M1"]),
            positive(&structure["callWall"]["strike"]),
        ]
        .into_iter()
        .flatten()
        .any(|level| (level - spot).abs() / level * 100. <= 0.35)
    {
        interactions.push((45., 0.));
    }
    interactions.sort_by(|a, b| b.0.total_cmp(&a.0));
    let level = interactions
        .first()
        .map(|(score, bias)| score * bias / 100.);
    let pressure = num(&structure["quickPressure"])
        .filter(|p| (0. ..=100.).contains(p))
        .map(|p| (2. * p - 100.) / 100.);
    let local = &s["localVolume"];
    let local_ok = local["symbol"] == r.symbol
        && local["usable"] == true
        && num(&local["last_trade_age_seconds"]).is_some_and(|age| {
            age >= 0.0
                && age
                    + (now.saturating_sub(s["publishedMs"].as_u64().unwrap_or(0))) as f64 / 1000.0
                    <= 15.0
        });
    for (name, signal, weight) in [
        ("FLOW 1M", f1, 24.),
        ("FLOW 5M", f5, 12.),
        (
            "CALL BUY / PUT BUY 1M",
            ratio(
                &one["buckets"]["CALL BUY"]["contracts"],
                &one["buckets"]["PUT BUY"]["contracts"],
            ),
            18.,
        ),
        ("PRECIO 2M + VWAP", price, 20.),
        ("NIVEL EN CONTROL", level, 16.),
        ("PRESIÓN QUICK READ", pressure, 6.),
        (
            "RVOL / PARTICIPACIÓN",
            if local_ok {
                ratio(&local["call_contracts_1m"], &local["put_contracts_1m"])
            } else {
                None
            },
            4.,
        ),
    ] {
        if let Some(signal) = signal {
            r.components
                .push((name.into(), signal.clamp(-1., 1.), weight));
        }
    }
    r.coverage = r.components.iter().map(|c| c.2).sum();
    r.net = if r.coverage > 0. {
        r.components.iter().map(|c| c.1 * c.2).sum::<f64>() / r.coverage
    } else {
        0.
    };
    r.price = price.unwrap_or(0.);
    r.flow_one = f1.unwrap_or(0.);
    r.flow_five = f5.unwrap_or(0.);
    r.ready = valid_structure
        && vwap.is_some()
        && price.is_some()
        && f1.is_some()
        && f5.is_some()
        && r.coverage >= 65.;
    r.management_ready = valid_structure && vwap.is_some() && price.is_some();
    let sign = r.net.signum();
    let magnet = positive(&structure["selected"]["M1"]);
    let route = structure["routes"].as_array().and_then(|rows| {
        rows.iter()
            .find(|v| v["direction"] == if sign > 0. { "CALL" } else { "PUT" })
    });
    let mut targets = vec![
        magnet,
        positive(&structure["callWall"]["strike"]),
        positive(&structure["putWall"]["strike"]),
    ];
    if let Some(route) = route.filter(|r| r["structuralReady"] == true) {
        targets.extend([
            positive(&route["barrier"]),
            positive(&route["target"]),
            positive(
                &route[if sign > 0.0 {
                    "vacuumLow"
                } else {
                    "vacuumHigh"
                }],
            ),
        ]);
    }
    r.next = targets
        .into_iter()
        .flatten()
        .filter(|level| (level - spot) * sign > 0.)
        .min_by(|a, b| (a - spot).abs().total_cmp(&(b - spot).abs()));
    let min_strength = if paper_partial {
        PAPER_MIN_STRENGTH
    } else {
        0.55
    };
    r.eligible = r.ready
        && r.net.abs() >= min_strength
        && r.flow_one * sign >= 0.15
        && r.flow_five * sign > 0.
        && r.price * sign > 0.
        && vwap.is_some_and(|v| (spot - v) * sign > 0.)
        && r.next.is_some();
    for (ok, reason) in [
        (
            valid_structure,
            "Estructura DAY incompleta, provisional o vencida",
        ),
        (vwap.is_some(), "VWAP RTH no disponible"),
        (
            price.is_some(),
            "Faltan velas cerradas de 2 minutos vigentes",
        ),
        (
            f1.is_some(),
            "Flujo del vencimiento analizado de 1m incompleto o sin dirección clasificada",
        ),
        (
            f5.is_some(),
            "Flujo del vencimiento analizado de 5m incompleto o sin dirección clasificada",
        ),
        (r.coverage >= 65., "Cobertura inferior al 65%"),
        (
            r.net.abs() >= min_strength,
            if paper_partial {
                "Fuerza inferior a 35/100"
            } else {
                "Fuerza inferior a 55/100"
            },
        ),
        (
            r.flow_one * sign >= 0.15,
            "Flujo 1m no confirma el lado de entrada",
        ),
        (r.flow_five * sign > 0., "Flujo 5m no acompaña"),
        (r.price * sign > 0., "Precio no acompaña el flujo"),
        (
            vwap.is_some_and(|v| (spot - v) * sign > 0.),
            "Precio y VWAP no alineados",
        ),
        (r.next.is_some(), "Sin nivel verificado por delante"),
    ] {
        if !ok {
            r.blockers.push(reason.into());
        }
    }

    r.entry_side=if sign>0. {"CALL"}else{"PUT"}.into();
    r.entry_basis="directional".into();
    if paper_partial {
        let enough_room=directional_room(spot,r.next,bars.last().map(|b|(b.low,b.high)),sign);
        if !enough_room {
            r.eligible=false;
            r.blockers.push("Recorrido direccional inferior a 1,8 veces el riesgo hasta el extremo de la última vela".into());
        }
        // Aggregate flow cannot authorize an entry while the newest candle rejects its side.
        let confirmed_candle=bars.last().is_some_and(|b|(b.close-b.open)*sign>0. && (spot-b.open)*sign>0.);
        if !confirmed_candle {
            r.eligible=false;
            r.blockers.push("Esperar cierre de vela a favor y precio que sostenga su apertura".into());
        }
        if r.ready {
            if let Some((side,target)) = vwap_rebound(&bars,spot,vwap.unwrap(),&r.levels,r.flow_one,r.flow_five) {
                let direction=if side=="CALL" {1.}else{-1.};
                let clear_route=structure["routes"].as_array().and_then(|rows|rows.iter().find(|v|v["direction"]==side))
                    .is_some_and(|route|route["structuralReady"]==true && !positive(&route["barrier"]).is_some_and(|level|
                        (level-spot)*direction>0. && (target-level)*direction>0.
                        && num(&route["barrierScore"]).is_none_or(|score|score>=60. || score<0.)));
                if clear_route {
                r.eligible=true;
                r.entry_side=side.into();
                r.entry_basis="vwap-rebound".into();
                r.next=Some(target);
                r.blockers.clear();
                }
            }
        }
        r.structural_entry=structural_entry(structure,&bars,spot,now,r.ready,f1,f5);
        if r.structural_entry.eligible {
            r.eligible=true;
            r.entry_side=r.structural_entry.side.clone();
            r.entry_basis="magnet-wall".into();
            r.next=r.structural_entry.pattern.target;
            r.blockers.clear();
        } else if !r.eligible && r.structural_entry.pattern.detected {
            r.blockers.extend(r.structural_entry.blockers.iter().map(|b|format!("Ruta del imán: {b}")));
        }
    }
    crate::quick_bot_context::apply(s,&mut r,now);
    crate::fvg::apply(s,&mut r,now);
    crate::gap_bot_context::apply(s,&mut r,now);
    r
}

fn directional_room(spot:f64,target:Option<f64>,bar:Option<(f64,f64)>,sign:f64)->bool {
    target.zip(bar).is_some_and(|(target,(low,high))| {
        let stop=if sign>0. {low}else{high};
        let risk=(spot-stop)*sign;
        let reward=(target-spot)*sign;
        [spot,target,low,high,sign].iter().all(|v|v.is_finite())
            && risk>0. && reward>0. && reward/risk>=1.8
    })
}
#[cfg(test)]
mod entry_room_regression {
    use super::directional_room;
    #[test] fn nvda_1326_and_mirrored_call_cannot_chase_a_nearby_wall() {
        assert!(!directional_room(230.136,Some(230.),Some((230.179,230.4386)),-1.));
        assert!(!directional_room(230.864,Some(231.),Some((230.5614,230.821)),1.));
        assert!(directional_room(230.136,Some(229.),Some((230.1,230.4386)),-1.));
    }
}
#[derive(Clone, Default)]
pub struct Tracker {
    session: String,
    evidence: u64,
    observation: String,
    at: u64,
    side: i8,
    route: String,
    count: u32,
    signal_ms: u64,
    prior_net: Option<f64>,
    opposing_count: u32,
    exit: bool,
}
impl Tracker {
    pub fn update(&mut self, r: &Reading, armed_ms: u64, held: Option<&str>) -> Value {
        if self.session != r.session || r.at.saturating_sub(self.at) > 15000 || !r.ready {
            *self = Self {
                session: r.session.clone(),
                ..Default::default()
            };
        }
        let side=match r.entry_side.as_str(){"CALL"=>1,"PUT"=>-1,_=>if r.net>0.{1}else{-1}};
        let route=if r.entry_basis=="ema9-rebound" {format!("ema9:{}",r.quick_context["ema9Rebound"]["key"])}else if r.quick_context["chartRoute"]["confirmed"]==true {format!("chart:{}",r.quick_context["chartRoute"]["key"])}else if r.entry_basis=="magnet-wall" {r.structural_entry.key.clone()}else if r.entry_basis=="vwap-rebound" {format!("vwap-rebound:{}:{:?}",r.closed_bars.last().map(|b|b["t"].as_u64().unwrap_or(0)).unwrap_or(0),r.next)}else{"directional".into()};
        if !r.eligible || self.side!=side || self.route!=route || held.is_none() && self.signal_ms>0 && r.at.saturating_sub(self.signal_ms)>10000 {
            self.count=0;
            self.signal_ms=0;
        }
        if r.ready
            && r.evidence_ms > self.evidence
            && r.evidence_ms >= armed_ms
            && r.observation != self.observation
        {
            if r.eligible {
                self.count = if self.side == side { self.count + 1 } else { 1 };
                self.side = side;
                self.route = route;
                if self.count == 2 {
                    self.signal_ms = r.at;
                }
            } else {
                self.count = 0;
                self.signal_ms = 0;
            }
            if let Some(held) = held {
                let direction = if held == "CALL" { 1. } else { -1. };
                let opposite = r.flow_one * direction <= -0.15 && r.price * direction < 0.;
                let gaining = self
                    .prior_net
                    .is_some_and(|n| (r.net - n) * direction <= -0.06);
                let reversed = r.net * direction <= -0.15;
                if opposite && (gaining || reversed || self.opposing_count > 0) {
                    self.opposing_count += 1;
                } else {
                    self.opposing_count = 0;
                }
                if self.opposing_count >= 2 {
                    self.exit = true;
                }
            }
            self.prior_net = Some(r.net);
            self.evidence = r.evidence_ms;
            self.observation = r.observation.clone();
            self.at = r.at;
        }
        let entry =
            r.eligible && self.count >= 2 && self.signal_ms >= armed_ms && self.signal_ms > 0;
        json!({"strategy":VERSION,"symbol":r.symbol,"entriesEnabled":true,"reviewedMs":r.at,
            "reading":r,"oppositionExit":self.exit,"position":if entry {json!({"tradeId":self.signal_ms,"trade":{"openedMs":self.signal_ms,"closedMs":null,"side":if side>0{"CALL"}else{"PUT"},"entry":r.spot}})}else{Value::Null}})
    }
}

#[cfg(test)]
mod structural_entry_tests {
    use super::*;
    #[test]
    fn vwap_rebound_waits_for_breakout_and_rejects_chasing_or_opposing_flow() {
        for direction in [1.,-1.] {
            let p=|n:f64| if direction>0. {n}else{664.1-n};
            let candle=|t,o,h,l,c| Bar {t,open:p(o),high:p(if direction>0.{h}else{l}),low:p(if direction>0.{l}else{h}),close:p(c)};
            let bars=vec![candle(0,332.58,332.73,332.14,332.345),candle(120000,332.345,332.8,332.08,332.61)];
            let levels=vec![p(335.)];
            let expected=if direction>0.{"CALL"}else{"PUT"};
            assert_eq!(vwap_rebound(&bars,p(332.85),p(332.05),&levels,0.2*direction,0.),Some((expected,p(335.))));
            assert!(vwap_rebound(&bars,p(332.6),p(332.05),&levels,0.2*direction,0.).is_none());
            assert!(vwap_rebound(&bars,p(334.),p(332.05),&levels,0.2*direction,0.).is_none());
            assert!(vwap_rebound(&bars,p(332.85),p(332.05),&levels,0.2*direction,-0.4*direction).is_none());
            assert!(vwap_rebound(&bars,p(332.85),p(332.05),&[],0.2*direction,0.).is_none());
        }
    }
    fn fixture(call:bool)->Value {
        let mut s:Value=serde_json::from_str(include_str!("../../../fixtures/intraday-strategy.json")).unwrap();
        let now=s["publishedMs"].as_u64().unwrap();
        let p=|n:f64|if call {n}else{1474.-n};
        s["ticks"]["QQQ"]["price"]=json!(p(737.07));
        s["metrics"]["vwap"]=json!(p(738.));
        s["localVolume"]=Value::Null;
        for w in s["flow"]["intraday_windows"].as_array_mut().unwrap() {
            w["bullish_pct"]=json!(50.);w["bearish_pct"]=json!(50.);
            w["buckets"]["CALL BUY"]["contracts"]=json!(50.);
            w["buckets"]["PUT BUY"]["contracts"]=json!(50.);
        }
        let st=&mut s["optionSnapshotStatus"]["horizons"][0]["structure"];
        st["selected"]["M1"]=json!(737.);st["callWall"]["strike"]=json!(740.);st["putWall"]["strike"]=json!(734.);
        st["quickPressure"]=json!(50.);
        st["routes"]=json!([{"direction":"CALL","structuralReady":true,"barrier":740.,"barrierScore":80.},{"direction":"PUT","structuralReady":true,"barrier":734.,"barrierScore":80.}]);
        let candles=[(736.95,737.2218,736.26,736.37),(736.37,737.15,736.275,737.1499),(737.07,737.55,736.87,737.07)];
        let mut minutes=vec![];
        for (i,(o,h,l,c)) in candles.into_iter().enumerate(){
            for j in 0..2 {minutes.push(json!({"timestampMs":now-360000+i as u64*120000+j*60000,"open":p(o),"high":p(h).max(p(l)),"low":p(l).min(p(h)),"close":p(c),"volume":100}));}
        }
        s["chartHistory"]["minutes"]=json!(minutes);
        s
    }
    #[test] fn neutral_flow_rebound_enters_call_and_put_independent_of_net_and_vwap_direction(){
        for call in [true,false] {
            let s=fixture(call);let now=s["publishedMs"].as_u64().unwrap();
            let mut r=read_paper(&s,now);
            assert!(r.ready&&r.eligible,"{:?}",r.blockers);
            assert!(r.structural_entry.eligible);assert_eq!(r.entry_basis,"magnet-wall");
            assert_eq!(r.entry_side,if call{"CALL"}else{"PUT"});
            assert!(r.net.abs()<PAPER_MIN_STRENGTH);assert!(!read(&s,now).eligible);
            let mut tracker=Tracker::default();
            assert!(tracker.update(&r,now,None)["position"].is_null());
            assert!(tracker.update(&r,now,None)["position"].is_null());
            r.evidence_ms+=1000;r.at+=1000;r.observation="new classified event".into();
            assert_eq!(tracker.update(&r,now,None)["position"]["trade"]["side"],r.entry_side);
            // Losing eligibility or changing route without fresh evidence must immediately cancel the pending signal.
            r.structural_entry.key.push_str("changed");
            assert!(tracker.update(&r,now,None)["position"].is_null());
            r.eligible=false;
            assert!(tracker.update(&r,now,None)["position"].is_null());
        }
    }
    #[test] fn structural_route_rejects_weak_walls_obstacles_opposition_late_prices_and_bad_data(){
        for call in [true,false] {
            for mode in 0..12 {
                let mut s=fixture(call);let now=s["publishedMs"].as_u64().unwrap();
                let field=if call{"callWall"}else{"putWall"};let index=if call{0}else{1};
                match mode {
                    0=>s["optionSnapshotStatus"]["horizons"][0]["structure"][field]["score"]=json!(59.),
                    1=>s["optionSnapshotStatus"]["horizons"][0]["structure"]["routes"][index]["barrier"]=json!(if call{738.}else{736.}),
                    2=>s["optionSnapshotStatus"]["horizons"][0]["structure"]["routes"]=json!([]),
                    3=>{s["flow"]["intraday_windows"][0]["bullish_pct"]=json!(if call{30}else{70});s["flow"]["intraday_windows"][0]["bearish_pct"]=json!(if call{70}else{30});},
                    4=>s["ticks"]["QQQ"]["price"]=json!(if call{739.9}else{734.1}),
                    5=>s["ticks"]["QQQ"]["price"]=json!(if call{736.9}else{737.1}),
                    6=>s["ticks"]["QQQ"]["tradeMs"]=json!(now-5001),
                    7=>s["options"]["phase"]=json!("disconnected"),
                    8=>s["flow"]=Value::Null,
                    9=>s["chartHistory"]["minutes"].as_array_mut().unwrap().clear(),
                    10=>s["optionSnapshotStatus"]["observedMs"]=json!(now-900001),
                    _=>s["optionSnapshotStatus"]["horizons"][0]["structure"][field]["score"]=Value::Null,
                }
                let r=read_paper(&s,now);assert!(!r.structural_entry.eligible,"call={call} mode={mode}");assert!(!r.eligible,"call={call} mode={mode}");
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn five_background_assets_keep_independent_confirmation_and_expiration_scope(){
        let base=fixture();let now=base["publishedMs"].as_u64().unwrap();
        let mut trackers=std::collections::BTreeMap::new();
        for root in ["QQQ","SPY","AAPL","NVDA","TSLA"] {
            let mut s=base.clone();s["symbol"]=json!(root);s["backgroundBot"]=json!(true);s["flowExpiry"]=json!("2026-09-25");
            s["ticks"]=json!({root:{"symbol":root,"price":501.8,"tradeMs":now}});
            s["optionSnapshotStatus"]["horizons"][0]["expirations"]=json!(["2026-09-25"]);
            let mut r=read_paper(&s,now);assert!(r.eligible,"{root}: {:?}",r.blockers);
            let tracker=trackers.entry(root).or_insert_with(Tracker::default);
            assert!(tracker.update(&r,now,None)["position"].is_null());
            r.at+=1000;r.evidence_ms+=1000;r.observation="new own event".into();
            assert_eq!(tracker.update(&r,now,None)["symbol"],root);
            s["flowExpiry"]=json!("2026-10-02");assert!(!read_paper(&s,now).ready);
            s["flowExpiry"]=json!("2026-09-25");s["ticks"][root]["tradeMs"]=json!(now-5001);assert!(!read_paper(&s,now).ready);
        }
        assert_eq!(trackers.len(),5);
    }
    fn fixture() -> Value {
        serde_json::from_str(include_str!("../../../fixtures/intraday-strategy.json")).unwrap()
    }
    #[test]
    fn neuro_reaches_bot_reading_without_manufacturing_confirmation() {
        let mut s=fixture();let now=s["publishedMs"].as_u64().unwrap();let before=read_paper(&s,now);
        let context=crate::neuro_context::Context{symbol:"QQQ".into(),at:now,spot:s["ticks"]["QQQ"]["price"].as_f64(),realtime:true,regular:true,net_gamma:Some(-20.),charm_flow:Some(-10.),contracts:100,used:80,..Default::default()};
        s["neuroContext"]=serde_json::to_value(context).unwrap();
        let after=SnapshotBuffer::default().read_paper(s.clone(),now);
        assert!(after.neuro_context.available);
        assert_eq!(before.net,after.net);assert_eq!(before.eligible,after.eligible);
        assert!(after.warnings.iter().any(|w|w.contains("Gamma del modelo")));
        let report=Tracker::default().update(&after,now,None);
        assert_eq!(report["reading"]["neuro_context"]["used"],80);
        s["neuroContext"]["symbol"]=json!("SPY");assert!(!read_paper(&s,now).neuro_context.available);
        s["neuroContext"]["symbol"]=json!("QQQ");s["neuroContext"]["at"]=json!(now-45001);assert!(!read_paper(&s,now).neuro_context.available);
    }
    #[test]
    fn greek_context_is_support_not_an_entry_vote() {
        let mut s = fixture();
        let now = s["publishedMs"].as_u64().unwrap();
        let before = read(&s, now);
        s["greekSupport"] = serde_json::to_value(snipers_core::greek_support::Context {
            symbol: "QQQ".into(),
            at: now,
            available: true,
            interpretation: "support".into(),
            ..Default::default()
        })
        .unwrap();
        let after = read(&s, now);
        assert!(after.greek_support.available);
        assert_eq!(before.net, after.net);
        assert_eq!(before.eligible, after.eligible);
        assert_eq!(before.coverage, after.coverage);
        s["greekSupport"]["at"] = json!(now - 3001);
        assert!(!read(&s, now).greek_support.available);
    }
    #[test]
    fn audit_rejects_invalid_flow_missing_local_volume_and_discontinuous_history() {
        let base = fixture();
        let now = base["publishedMs"].as_u64().unwrap();
        for (a, b) in [(101., 0.), (80., 80.), (-1., 50.)] {
            let mut s = base.clone();
            s["flow"]["intraday_windows"][0]["bullish_pct"] = json!(a);
            s["flow"]["intraday_windows"][0]["bearish_pct"] = json!(b);
            let r = read(&s, now);
            assert!(!r.ready && !r.eligible);
            assert!(!r.components.iter().any(|c| c.0 == "CALL BUY / PUT BUY 1M"));
        }
        let mut s = base.clone();
        s["localVolume"]["call_contracts_1m"] = Value::Null;
        assert!(!read(&s, now)
            .components
            .iter()
            .any(|c| c.0 == "RVOL / PARTICIPACIÓN"));
        s = base.clone();
        s["localVolume"]["last_trade_age_seconds"] = json!(14.5);
        assert!(!read(&s, now + 1000)
            .components
            .iter()
            .any(|c| c.0 == "RVOL / PARTICIPACIÓN"));
        s = base;
        s["chartHistory"]["minutes"]
            .as_array_mut()
            .unwrap()
            .remove(4);
        let closed = bars(&s, now);
        assert_eq!(closed.len(), 1);
        assert!(closed.windows(2).all(|w| w[1].t - w[0].t == 120000));
    }
    #[test]
    fn audit_target_matches_nearest_verified_intraday_level() {
        let mut s = fixture();
        let now = s["publishedMs"].as_u64().unwrap();
        let st = &mut s["optionSnapshotStatus"]["horizons"][0]["structure"];
        st["selected"]["M1"] = json!(510.0);
        st["callWall"]["strike"] = json!(504.0);
        st["routes"] = json!([{"direction":"CALL","structuralReady":true,"barrier":503.0,"barrierScore":70.0,"target":506.0,"vacuumLow":502.5}]);
        assert_eq!(read(&s, now).next, Some(502.5));
        s["optionSnapshotStatus"]["horizons"][0]["structure"]["routes"][0]["structuralReady"] =
            json!(false);
        assert_eq!(read(&s, now).next, Some(504.0));
    }
    #[test]
    fn full_intraday_model_and_strict_entry_gate() {
        let s = fixture();
        let now = s["publishedMs"].as_u64().unwrap();
        let r = read(&s, now);
        assert!(r.ready && r.eligible);
        assert_eq!(r.coverage, 100.);
        assert_eq!(r.next, Some(504.));
        assert!((r.net - 0.776380).abs() < 0.0001, "{}", r.net);
        for field in ["chartHistory", "flow", "optionSnapshotStatus"] {
            let mut invalid = s.clone();
            invalid[field] = Value::Null;
            assert!(!read(&invalid, now).eligible);
        }
        assert!(!read(&s, now + 16000).ready);
        let mut wrong = s.clone();
        wrong["symbol"] = "SPY".into();
        assert!(!read(&wrong, now).ready);
        let mut wrong = s.clone();
        wrong["metrics"]["vwap"] = 600.into();
        assert!(!read(&wrong, now).eligible);
    }
    #[test]
    fn no_weekly_flow_substitution_or_partial_candles() {
        let mut s = fixture();
        let now = s["publishedMs"].as_u64().unwrap();
        s["flow"]["windows"] = s["flow"]["intraday_windows"].clone();
        s["flow"]["intraday_windows"] = json!([]);
        assert!(!read(&s, now).ready);
        s = fixture();
        s["chartHistory"]["minutes"] = json!([s["chartHistory"]["minutes"][6]]);
        assert!(!read(&s, now).ready);
    }
    #[test]
    fn entries_need_two_new_observations_after_arming() {
        let s = fixture();
        let now = s["publishedMs"].as_u64().unwrap();
        let mut r = read(&s, now);
        let mut tracker = Tracker::default();
        assert!(tracker.update(&r, now + 1, None)["position"].is_null());
        r.evidence_ms += 2;
        r.observation = r.evidence_ms.to_string();
        r.at += 2;
        assert!(tracker.update(&r, now + 1, None)["position"].is_null());
        r.at += 2;
        assert!(tracker.update(&r, now + 1, None)["position"].is_null());
        r.evidence_ms += 2;
        r.observation = r.evidence_ms.to_string();
        r.at += 2;
        assert_eq!(
            tracker.update(&r, now + 1, None)["position"]["trade"]["side"],
            "CALL"
        );
        r.session = "next session".into();
        r.evidence_ms += 2;
        r.observation = r.evidence_ms.to_string();
        r.at += 2;
        assert!(tracker.update(&r, now + 1, None)["position"].is_null());
    }
    #[test]
    fn growing_opposition_exits_symmetrically_and_latches() {
        for (held, direction) in [("CALL", 1.), ("PUT", -1.)] {
            let s = fixture();
            let mut r = read(&s, s["publishedMs"].as_u64().unwrap());
            let mut tracker = Tracker::default();
            r.net = 0.7 * direction;
            tracker.update(&r, 0, Some(held));
            r.net = 0.5 * direction;
            r.flow_one = -0.3 * direction;
            r.price = -0.1 * direction;
            r.at += 2000;
            r.evidence_ms += 2000;
            r.observation = r.evidence_ms.to_string();
            assert_eq!(tracker.update(&r, 0, Some(held))["oppositionExit"], false);
            assert_eq!(tracker.update(&r, 0, Some(held))["oppositionExit"], false);
            r.at += 2000;
            r.evidence_ms += 2000;
            r.observation = r.evidence_ms.to_string();
            assert_eq!(tracker.update(&r, 0, Some(held))["oppositionExit"], true);
            r.net = direction;
            r.at += 2000;
            r.evidence_ms += 2000;
            r.observation = r.evidence_ms.to_string();
            assert_eq!(tracker.update(&r, 0, Some(held))["oppositionExit"], true);
        }
    }
}
#[cfg(test)]
mod blocking_diagnostics_tests {
    use super::*;
    fn fixture() -> Value {
        serde_json::from_str(include_str!("../../../fixtures/intraday-strategy.json")).unwrap()
    }
    #[test]
    fn complete_reading_has_no_blockers_and_still_requires_two_new_observations() {
        let s = fixture();
        let now = s["publishedMs"].as_u64().unwrap();
        let mut r = read(&s, now);
        assert!(r.blockers.is_empty());
        assert!(r.eligible);
        let mut t = Tracker::default();
        assert!(t.update(&r, now, None)["position"].is_null());
        r.at += 1000;
        r.evidence_ms += 1000;
        r.observation = "second distinct observation".into();
        assert!(!t.update(&r, now, None)["position"].is_null());
    }
    #[test]
    fn reports_clock_offset_without_silently_weakening_entry_gate() {
        let mut s = fixture();
        let now = s["publishedMs"].as_u64().unwrap();
        s["ticks"]["QQQ"]["tradeMs"] = json!(now + 650);
        let r = read(&s, now);
        assert!(!r.eligible);
        assert!(r.blockers.iter().any(|b| b.contains("adelantado 650 ms")));
    }
    #[test]
    fn explains_provisional_structure_and_missing_five_minute_flow() {
        let mut s = fixture();
        let now = s["publishedMs"].as_u64().unwrap();
        let subset = s["optionSnapshotStatus"]["horizons"][0]["structure"].clone();
        s["optionSnapshotStatus"]["horizons"][0]["structure"] =
            json!({"complete":false,"provisional":subset});
        s["flow"]["intraday_windows"][1]["premium_complete"] = json!(false);
        let r = read(&s, now);
        assert!(!r.eligible);
        assert!(r.blockers.iter().any(|b| b.contains("DAY incompleta")));
        assert!(r.blockers.iter().any(|b| b.contains("5m incompleto")));
    }
}

/// Hold slightly future observations until their original event times arrive.
#[cfg(test)]
mod paper_sample_tests {
    use super::*;
    #[test]
    fn paper_consumes_provisional_levels_when_oi_is_complete_but_gamma_is_missing() {
        use snipers_core::options::{normalize, Observation};
        let mut s: Value =
            serde_json::from_str(include_str!("../../../fixtures/intraday-strategy.json")).unwrap();
        let now = s["publishedMs"].as_u64().unwrap();
        let contracts = [
            ("O:QQQ260925P00499000", 499., "put", Some(0.02)),
            ("O:QQQ260925C00505000", 505., "call", Some(0.02)),
            ("O:QQQ260925C00510000", 510., "call", None),
        ].into_iter().map(|(ticker, strike, kind, gamma)| {
            let mut c = normalize(&json!({"details":{"ticker":ticker,"strike_price":strike,"contract_type":kind,"expiration_date":"2026-09-25"},"open_interest":100,"day":{"volume":20},"greeks":{"gamma":gamma}}),"QQQ").unwrap();
            c.premium_multiplier = Some(100.);
            (ticker.into(), c)
        }).collect();
        let obs = Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: now,
            observed_date: "2026-09-25".into(),
            oi_reference_date: None,
            contracts,
        };
        let structure =
            snipers_core::structure::calculate(&obs, None, &["2026-09-25".into()], 501.8).unwrap();
        assert!(structure.complete);
        assert!(!structure.liquidity_complete && !structure.gex_available);
        assert!(structure.call_wall.is_none());
        assert!(structure.provisional.is_some());
        s["optionSnapshotStatus"]["horizons"][0]["structure"] = json!(structure);
        let strict = read(&s, now);
        let paper = read_paper(&s, now);
        assert!(strict.next.is_none());
        assert!(!strict.eligible);
        assert_eq!(paper.next, Some(505.));
        assert!(paper.levels.contains(&499.) && paper.levels.contains(&505.));
        assert!(paper
            .warnings
            .iter()
            .any(|w| w.contains("niveles provisionales")));
        assert!(paper.ready && paper.eligible);
        assert!(!read_paper(&s, now + 900001).eligible);
    }

    #[test]
    fn paper_admits_aggressive_signal_without_changing_strict_mode_or_confirmation() {
        let base: Value =
            serde_json::from_str(include_str!("../../../fixtures/intraday-strategy.json")).unwrap();
        let now = base["publishedMs"].as_u64().unwrap();
        let mut admitted = false;
        for bull in 50..=90 {
            let mut s = base.clone();
            for w in s["flow"]["intraday_windows"].as_array_mut().unwrap() {
                w["bullish_pct"] = json!(bull);
                w["bearish_pct"] = json!(100 - bull);
                w["buckets"]["CALL BUY"]["contracts"] = json!(bull);
                w["buckets"]["PUT BUY"]["contracts"] = json!(100 - bull);
            }
            let mut paper = read_paper(&s, now);
            if (0.35..0.55).contains(&paper.net.abs()) {
                if paper.flow_one < 0.15 || paper.flow_five <= 0. {
                    assert!(!paper.eligible, "strength cannot replace flow confirmation");
                    continue;
                }
                assert!(paper.eligible);
                assert!(!read(&s, now).eligible);
                let mut tracker = Tracker::default();
                assert!(tracker.update(&paper, now, None)["position"].is_null());
                assert!(tracker.update(&paper, now, None)["position"].is_null());
                paper.at += 1000;
                paper.evidence_ms += 1000;
                paper.observation = "second independent observation".into();
                assert!(!tracker.update(&paper, now, None)["position"].is_null());
                admitted |= paper.net.abs() < 0.50;
            } else if paper.net.abs() < 0.35 {
                assert!(!paper.eligible);
                assert!(paper
                    .blockers
                    .iter()
                    .any(|b| b == "Fuerza inferior a 35/100"));
            }
        }
        assert!(
            admitted,
            "fixture must exercise the newly admitted strength band"
        );
    }

    fn fixture() -> Value {
        let mut s: Value =
            serde_json::from_str(include_str!("../../../fixtures/intraday-strategy.json")).unwrap();
        s["optionSnapshotStatus"]["horizons"][0]["structure"]["complete"] = json!(false);
        for w in s["flow"]["intraday_windows"].as_array_mut().unwrap() {
            w["premium_complete"] = json!(false);
            w["bullish_pct"] = Value::Null;
            w["bearish_pct"] = Value::Null;
            for (label, premium) in [
                ("CALL BUY", 900.),
                ("PUT BUY", 100.),
                ("CALL SELL", 0.),
                ("PUT SELL", 0.),
                ("UNKNOWN", 50.),
            ] {
                w["buckets"][label] = json!({"premium":premium,"contracts":premium/10.});
            }
        }
        s
    }
    #[test]
    fn partial_samples_only_enable_paper_and_keep_missing_data_visible() {
        let s = fixture();
        let now = s["publishedMs"].as_u64().unwrap();
        assert!(!read(&s, now).eligible);
        let r = read_paper(&s, now);
        assert!(r.ready && r.eligible && r.paper_partial);
        assert_eq!(r.warnings.len(), 4);
        assert!(r.warnings.iter().any(|w|w.starts_with("Neuro:")));
        assert_eq!(s["flow"]["intraday_windows"][0]["premium_complete"], false);
        let mut tracker = Tracker::default();
        assert!(tracker.update(&r, now, None)["position"].is_null());
        assert!(tracker.update(&r, now, None)["position"].is_null());
        let mut next = r;
        next.at += 1000;
        next.evidence_ms += 1000;
        next.observation = "new evidence".into();
        assert!(!tracker.update(&next, now, None)["position"].is_null());
    }
    #[test]
    fn paper_rejects_empty_unclassified_sparse_invalid_or_stale_evidence() {
        let base = fixture();
        let now = base["publishedMs"].as_u64().unwrap();
        for mode in 0..8 {
            let mut s = base.clone();
            match mode {
                0 => s["flow"] = Value::Null,
                1 => {
                    s["flow"]["intraday_windows"][0]["buckets"]["UNKNOWN"]["premium"] =
                        json!(10000.)
                }
                2 => s["flow"]["intraday_windows"][0]["event_count"] = json!(4),
                3 => {
                    s["flow"]["intraday_windows"][0]["buckets"]["CALL BUY"]["premium"] = json!(-1.)
                }
                4 => s["ticks"]["QQQ"]["tradeMs"] = json!(now - 5001),
                5 => s["metrics"]["vwap"] = Value::Null,
                6 => s["chartHistory"] = Value::Null,
                _ => s["options"]["phase"] = json!("disconnected"),
            }
            assert!(!read_paper(&s, now).eligible, "mode {mode}");
        }
        assert!(!read_paper(&base, now + 16000).ready);
    }
    #[test]
    fn paper_sample_opposing_flow_does_not_become_an_entry() {
        let mut s = fixture();
        let now = s["publishedMs"].as_u64().unwrap();
        s["flow"]["intraday_windows"][1]["buckets"]["PUT BUY"]["premium"] = json!(10000.);
        assert!(!read_paper(&s, now).eligible);
    }
}
/// Entry freshness checks remain strict; never substitute across sessions/assets.
#[derive(Clone, Default)]
pub struct SnapshotBuffer {
    pending: std::collections::VecDeque<Value>,
}
impl SnapshotBuffer {
    /// Same settled observation used by the strategy; never advance the local clock.
    pub fn settled_frame(&self, now: u64) -> Option<Value> {
        self.pending.iter().rev().find(|v| {
            let symbol=v["symbol"].as_str().unwrap_or("");
            recent(&v["publishedMs"],now,3000)
                && recent(&v["ticks"][symbol]["tradeMs"],now,5000)
                && recent(&v["options"]["lastEventMs"],now,15000)
        }).cloned()
    }
    pub fn read(&mut self, s: Value, now: u64) -> Reading {
        self.read_mode(s, now, false)
    }
    pub fn read_paper(&mut self, s: Value, now: u64) -> Reading {
        self.read_mode(s, now, true)
    }
    fn read_mode(&mut self, s: Value, now: u64, paper_partial: bool) -> Reading {
        let symbol = s["symbol"].as_str().unwrap_or("");
        let trade = s["ticks"][symbol]["tradeMs"].as_u64().unwrap_or(0);
        let option = s["options"]["lastEventMs"].as_u64().unwrap_or(0);
        let future = trade.max(option).saturating_sub(now);
        if future > 1000
            || s["stocks"]["phase"] != "streaming"
            || s["options"]["phase"] != "streaming"
        {
            self.pending.clear();
            return with_neuro(read_mode(&s, now, paper_partial));
        }
        self.pending.retain(|v| {
            v["symbol"] == s["symbol"]
                && v["startedMs"] == s["startedMs"]
                && recent(&v["publishedMs"], now, 3000)
        });
        self.pending.push_back(s);
        while self.pending.len() > 20 {
            self.pending.pop_front();
        }
        let chosen = self
            .pending
            .iter()
            .rev()
            .find(|v| {
                let symbol = v["symbol"].as_str().unwrap_or("");
                v["ticks"][symbol]["tradeMs"]
                    .as_u64()
                    .is_some_and(|t| t <= now)
                    && v["options"]["lastEventMs"]
                        .as_u64()
                        .is_some_and(|t| t <= now)
            })
            .unwrap_or_else(|| self.pending.back().unwrap());
        with_neuro(read_mode(chosen, now, paper_partial))
    }
}
#[cfg(test)]
mod snapshot_buffer_tests {
    use super::*;
    fn fixture() -> Value {
        serde_json::from_str(include_str!("../../../fixtures/intraday-strategy.json")).unwrap()
    }
    #[test]
    fn provider_ahead_stream_settles_without_future_entry_evidence() {
        let s = fixture();
        let base = s["publishedMs"].as_u64().unwrap();
        let mut b = SnapshotBuffer::default();
        for i in 0..8 {
            let now = base + i * 250;
            let mut v = s.clone();
            v["publishedMs"] = json!(now);
            v["ticks"]["QQQ"]["tradeMs"] = json!(now + 650);
            v["options"]["lastEventMs"] = json!(now + 650);
            let r = b.read(v, now);
            if i < 3 {
                assert!(!r.ready);
                assert!(b.settled_frame(now).is_none());
            } else {
                assert!(r.ready);
                let settled=b.settled_frame(now).unwrap();
                assert!(settled["ticks"]["QQQ"]["tradeMs"].as_u64().unwrap()<=now);
                assert!(settled["options"]["lastEventMs"].as_u64().unwrap()<=now);
                assert!(r.evidence_ms <= now);
                assert!(now - r.evidence_ms <= 5000);
            }
        }
    }
    #[test]
    fn never_hides_disconnect_large_clock_fault_asset_change_or_expired_publication() {
        let s = fixture();
        let now = s["publishedMs"].as_u64().unwrap();
        for mode in 0..4 {
            let mut b = SnapshotBuffer::default();
            assert!(b.read(s.clone(), now).ready);
            let mut bad = s.clone();
            match mode {
                0 => bad["options"]["phase"] = json!("denied"),
                1 => bad["ticks"]["QQQ"]["tradeMs"] = json!(now + 1001),
                2 => bad["symbol"] = json!("AAPL"),
                _ => bad["publishedMs"] = json!(now - 3001),
            }
            assert!(!b.read(bad, now).ready);
        }
    }
}
