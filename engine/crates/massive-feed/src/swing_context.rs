//! Pure experimental context. No network, UI, worker or broker integration.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snipers_core::{
    calendar::{self, Day},
    horizons,
};
pub const VERSION: &str = "structural-swing-v1";
const BAR_MS: u64 = 1_800_000;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub name: String,
    pub price: f64,
    pub horizon: String,
    pub expirations: Vec<String>,
    pub observed_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub version: String,
    pub symbol: String,
    pub at: u64,
    pub available: bool,
    pub state: String,
    pub side: String,
    pub spot: Option<f64>,
    pub snapshot_at: Option<u64>,
    pub horizons: Vec<String>,
    pub references: Vec<Reference>,
    pub anchor: Option<Reference>,
    pub next: Option<Reference>,
    pub blockers: Vec<String>,
}
fn n(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}
fn p(v: &Value) -> Option<f64> {
    n(v).filter(|n| *n > 0.)
}
fn fresh(v: &Value, now: u64, age: u64) -> bool {
    v.as_u64().is_some_and(|t| {
        t <= now && now - t <= age && calendar::date_at(t) == calendar::date_at(now)
    })
}
fn dates(v: &Value) -> Option<Vec<String>> {
    let values = v.as_array()?;
    if values.len() > 128 {
        return None;
    }
    let d = values
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    if d.iter()
        .any(|s| s.len() != 10 || calendar::rfc3339_ms(&format!("{s}T12:00:00-04:00")).is_none())
        || d.windows(2).any(|p| p[0] >= p[1])
    {
        return None;
    }
    Some(d)
}
fn exposure_side(e: &Value) -> Option<&'static str> {
    let used = e["deltaUsed"].as_u64()?;
    let eligible = e["eligible"].as_u64()?;
    let (c, p, net, cp, pp) = (
        n(&e["callDex"])?,
        n(&e["putDex"])?,
        n(&e["netDex"])?,
        n(&e["callDexPct"])?,
        n(&e["putDexPct"])?,
    );
    let gross = c.abs() + p.abs();
    if e["dexComplete"] != true
        || used != eligible
        || used == 0
        || e["unknownOi"] != 0
        || c < 0.
        || p > 0.
        || !gross.is_finite()
        || gross <= 0.
        || !(0. ..=100.).contains(&cp)
        || !(0. ..=100.).contains(&pp)
        || (cp + pp - 100.).abs() > 0.01
        || (cp - c / gross * 100.).abs() > 0.01
        || (net - c - p).abs() > 1e-6_f64.max(gross * 1e-9)
    {
        return None;
    }
    Some(if cp >= 52.5 {
        "CALL"
    } else if cp <= 47.5 {
        "PUT"
    } else {
        "NEUTRAL"
    })
}
fn fail(mut out: Context, why: &str) -> Context {
    out.available = false;
    out.state = "pending".into();
    out.next = None;
    out.blockers.push(why.into());
    out
}
/// References from the existing snapshot, never a prediction derived from OI alone.
pub fn evaluate(f: &Value, symbol: &str, active: bool, now: u64) -> Context {
    let mut out = Context {
        version: VERSION.into(),
        symbol: symbol.into(),
        at: now,
        available: false,
        state: "pending".into(),
        side: "PENDIENTE".into(),
        spot: None,
        snapshot_at: None,
        horizons: vec![],
        references: vec![],
        anchor: None,
        next: None,
        blockers: vec![],
    };
    if !active
        || f["symbol"] != symbol
        || f["stocks"]["phase"] != "streaming"
        || !fresh(&f["publishedMs"], now, 3000)
    {
        return fail(out, "Publicación propia vigente pendiente");
    }
    let Some(day) = calendar::date_at(now) else {
        return fail(out, "Calendario desconocido");
    };
    let Day::Open(session) = calendar::schedule(day) else {
        return fail(out, "Fuera de sesión hábil");
    };
    if now < session.open_ms.saturating_sub(19_800_000) || now >= session.close_ms {
        return fail(out, "Fuera de la ventana de observación");
    }
    let tick = &f["ticks"][symbol];
    let Some(spot) = p(&tick["price"])
        .filter(|_| tick["symbol"] == symbol && fresh(&tick["tradeMs"], now, 15000))
    else {
        return fail(out, "Precio propio pendiente");
    };
    out.spot = Some(spot);
    let snap = &f["optionSnapshotStatus"];
    if snap["phase"] != "ready" || !fresh(&snap["observedMs"], now, 900000) {
        return fail(out, "Snapshot estructural vigente pendiente");
    }
    let Some(universe) = dates(&snap["expirations"]).filter(|d| !d.is_empty()) else {
        return fail(out, "Universo de expiraciones inválido");
    };
    let Some(hs) = snap["horizons"].as_array().filter(|h| h.len() <= 8) else {
        return fail(out, "Horizontes pendientes");
    };
    let mut selected = vec![];
    let mut sides = vec![];
    for requested in ["DAY", "WEEK", "NEXT4"] {
        let matches = hs
            .iter()
            .filter(|h| {
                h["mode"] == requested || (requested == "DAY" && h["mode"] == "NEXT_SESSION")
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return fail(out, "Horizonte ausente o repetido");
        }
        let h = matches[0];
        let mode = h["mode"].as_str().unwrap();
        let Some(d) = dates(&h["expirations"]) else {
            return fail(out, "Fechas de horizonte inválidas");
        };
        if d.is_empty()
            || d != horizons::select(&universe, mode, now)
            || (requested == "NEXT4" && d.len() != 4)
            || h["observedMs"] != snap["observedMs"]
        {
            return fail(out, "Alcances inválidos o snapshots mezclados");
        }
        if mode == "NEXT_SESSION" && !horizons::select(&universe, "DAY", now).is_empty() {
            return fail(out, "Próxima sesión no sustituye al día disponible");
        }
        let Some(side) = exposure_side(&h["exposure"]) else {
            return fail(out, "Exposición delta incompleta o incoherente");
        };
        sides.push(side);
        out.horizons.push(mode.into());
        selected.push((h, d));
    }
    out.snapshot_at = snap["observedMs"].as_u64();
    out.available = true;
    if !sides.iter().all(|s| *s == sides[0]) || sides[0] == "NEUTRAL" {
        out.state = "mixed".into();
        out.side = "NEUTRAL".into();
        return out;
    }
    out.side = sides[0].into();
    let sign = if out.side == "CALL" { 1. } else { -1. };
    for (h, d) in selected.iter().skip(1) {
        let st = &h["structure"];
        let Some(cw) = p(&st["callWall"]["strike"]) else {
            return fail(out, "Call Wall pendiente");
        };
        let Some(pw) = p(&st["putWall"]["strike"]) else {
            return fail(out, "Put Wall pendiente");
        };
        if st["complete"] != true
            || st["provisional"] == true
            || st["provisional"].is_object()
            || pw >= cw
        {
            return fail(out, "Estructura provisional o corredor inválido");
        }
        let make = |name: &str, price: f64| Reference {
            name: name.into(),
            price,
            horizon: h["mode"].as_str().unwrap().into(),
            expirations: d.clone(),
            observed_ms: out.snapshot_at.unwrap(),
        };
        for (name, price) in [("CALL WALL", cw), ("PUT WALL", pw)] {
            out.references.push(make(name, price));
        }
        if let Some(m) = p(&st["selected"]["M1"]) {
            out.references.push(make("IMÁN PRINCIPAL", m));
        }
        if let Some(routes) = st["routes"].as_array() {
            if routes.len() > 128 {
                return fail(out, "Exceso de rutas");
            }
            for r in routes {
                for (name, field) in [("BARRERA", "barrier"), ("PULL", "pullTarget")] {
                    if let Some(price) = p(&r[field]) {
                        out.references.push(make(name, price));
                    }
                }
            }
        }
        if h["mode"] == "WEEK" {
            out.anchor = Some(make(
                if sign > 0. { "PUT WALL" } else { "CALL WALL" },
                if sign > 0. { pw } else { cw },
            ));
        }
    }
    out.references.sort_by(|a, b| a.price.total_cmp(&b.price));
    out.references.dedup_by(|a, b| a.price == b.price);
    let Some(anchor) = &out.anchor else {
        return fail(out, "Ancla semanal pendiente");
    };
    if (spot - anchor.price) * sign <= spot * 0.0003 {
        return fail(out, "Precio no sostiene el ancla semanal");
    }
    out.next = out
        .references
        .iter()
        .filter(|r| (r.price - spot) * sign > spot * 0.0003)
        .min_by(|a, b| (a.price - spot).abs().total_cmp(&(b.price - spot).abs()))
        .cloned();
    out.state = if out.next.is_some() {
        "conditional"
    } else {
        "no-reference"
    }
    .into();
    out
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntradayEvidence {
    pub symbol: String,
    pub at: u64,
    pub price_side: Option<String>,
    pub flow_side: Option<String>,
    pub cvd_side: Option<String>,
    pub qqq: Option<crate::qqq_component_reading::Report>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Confirmation {
    pub state: String,
    pub price: String,
    pub flow: String,
    pub cvd: String,
    pub components: String,
}
pub fn confirmation(side: &str, symbol: &str, e: &IntradayEvidence, now: u64) -> Confirmation {
    let own = e.symbol == symbol
        && e.at <= now
        && now - e.at <= 3000
        && calendar::date_at(e.at) == calendar::date_at(now)
        && matches!(side, "CALL" | "PUT");
    let compare = |s: Option<&str>| {
        if !own {
            "pending"
        } else {
            match s {
                Some("CALL" | "PUT") => {
                    if s == Some(side) {
                        "supports"
                    } else {
                        "opposes"
                    }
                }
                Some("NEUTRAL") => "mixed",
                _ => "pending",
            }
        }
        .to_string()
    };
    let price = compare(e.price_side.as_deref());
    let flow = compare(e.flow_side.as_deref());
    let cvd = compare(e.cvd_side.as_deref());
    let components = if symbol == "QQQ" {
        e.qqq
            .as_ref()
            .filter(|q| {
                own && q.symbol == symbol
                    && q.version == "qqq-components-v2"
                    && q.at <= now
                    && now - q.at <= 3000
                    && calendar::date_at(q.at) == calendar::date_at(now)
                    && q.available
                    && matches!(q.phase.as_str(), "Premarket" | "Sesión RTH")
                    && q.option_coverage.is_finite()
                    && q.option_coverage >= 0.
                    && q.option_coverage <= q.coverage
                    && q.price_coverage.is_finite()
                    && q.price_coverage >= 0.
                    && q.price_coverage <= q.selected_weight
                    && (q.phase != "Sesión RTH"
                        || (q.option_coverage >= 40.
                            && q.option_coverage >= q.selected_weight * 0.8))
                    && q.selected_weight.is_finite()
                    && q.selected_weight > 0.
                    && q.selected_weight <= 100.
                    && q.coverage.is_finite()
                    && q.coverage >= 40.
                    && q.coverage >= q.selected_weight * 0.8
                    && q.coverage <= q.selected_weight
                    && q.call.is_some_and(|c| {
                        c.is_finite() && (0. ..=100.).contains(&c) && q.put == Some(100. - c)
                    })
            })
            .map(|q| {
                compare(Some(if q.call.unwrap() >= 60. {
                    "CALL"
                } else if q.call.unwrap() <= 40. {
                    "PUT"
                } else {
                    "NEUTRAL"
                }))
            })
            .unwrap_or_else(|| "pending".into())
    } else {
        "not-applicable".into()
    };
    let all = [
        price.as_str(),
        flow.as_str(),
        cvd.as_str(),
        components.as_str(),
    ];
    let state = if all.contains(&"opposes") {
        "divergence"
    } else if [price.as_str(), flow.as_str(), cvd.as_str()]
        .iter()
        .all(|s| *s == "supports")
        && (symbol != "QQQ" || components == "supports")
    {
        "supported"
    } else {
        "unconfirmed"
    };
    Confirmation {
        state: state.into(),
        price,
        flow,
        cvd,
        components,
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub version: String,
    pub symbol: String,
    pub side: String,
    pub created_ms: u64,
    pub anchor: Reference,
    pub target: Reference,
    pub expires: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedBar {
    pub symbol: String,
    pub start_ms: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Engine {
    pub plan: Option<Plan>,
    pub last_now: u64,
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub retired_snapshot: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub state: String,
    pub context: Context,
    pub plan: Option<Plan>,
    pub intraday: Confirmation,
    pub remaining_dollars: Option<f64>,
    pub remaining_daily_atr: Option<f64>,
    pub reason: String,
}
fn valid_plan(p: &Plan, now: u64) -> bool {
    let expiry =
        calendar::rfc3339_ms(&format!("{}T12:00:00-04:00", p.expires)).and_then(calendar::date_at);
    let sign = if p.side == "CALL" { 1. } else { -1. };
    let provenance = p.anchor.horizon == "WEEK"
        && matches!(p.target.horizon.as_str(), "WEEK" | "NEXT4")
        && dates(&serde_json::json!(p.anchor.expirations)).is_some_and(|d| !d.is_empty())
        && dates(&serde_json::json!(p.target.expirations)).is_some_and(|d| !d.is_empty())
        && p.anchor
            .expirations
            .last()
            .zip(p.target.expirations.last())
            .is_some_and(|(a, b)| p.expires == a.min(b).as_str());
    provenance
        && p.version == VERSION
        && matches!(p.side.as_str(), "CALL" | "PUT")
        && p.created_ms <= now
        && p.anchor.price.is_finite()
        && p.target.price.is_finite()
        && p.anchor.price > 0.
        && p.target.price > 0.
        && (p.target.price - p.anchor.price) * sign > 0.
        && p.anchor.observed_ms <= p.created_ms
        && p.target.observed_ms <= p.created_ms
        && calendar::date_at(now).is_some_and(|d| d.to_string() <= p.expires)
        && expiry.is_some_and(|d| match calendar::schedule(d) {
            Day::Open(s) => now < s.close_ms,
            _ => false,
        })
}
fn closed_bars(bars: &[ClosedBar], symbol: &str, now: u64) -> Option<Vec<ClosedBar>> {
    if bars.len() > 64 {
        return None;
    }
    let Day::Open(session) = calendar::schedule(calendar::date_at(now)?) else {
        return None;
    };
    let mut result = vec![];
    for b in bars {
        let end = b.start_ms.checked_add(BAR_MS)?;
        if b.symbol != symbol
            || ![b.open, b.high, b.low, b.close]
                .iter()
                .all(|n| n.is_finite())
            || b.low <= 0.
            || b.high < b.open.max(b.close)
            || b.low > b.open.min(b.close)
            || b.start_ms < session.open_ms
            || (b.start_ms - session.open_ms) % BAR_MS != 0
            || end > session.close_ms
            || end > now
            || now - end > 5_400_000
        {
            return None;
        }
        result.push(b.clone());
    }
    result.sort_by_key(|b| b.start_ms);
    if result.windows(2).any(|p| p[0].start_ms == p[1].start_ms) {
        return None;
    }
    Some(result)
}
impl Engine {
    /// Frozen references survive overnight. Missing evidence suspends, never confirms a hold.
    pub fn update(
        &mut self,
        f: &Value,
        symbol: &str,
        active: bool,
        now: u64,
        e: &IntradayEvidence,
        bars: &[ClosedBar],
    ) -> Assessment {
        let context = evaluate(f, symbol, active, now);
        let mut state = "waiting";
        let mut reason = "Falta contexto estructural completo";
        if now < self.last_now {
            return Assessment {
                state: "suspended".into(),
                intraday: confirmation("PENDIENTE", symbol, e, now),
                context,
                plan: None,
                remaining_dollars: None,
                remaining_daily_atr: None,
                reason: "Reloj retrocedió; no avanzar estado".into(),
            };
        }
        self.last_now = now;
        if self.symbol != symbol {
            self.symbol = symbol.into();
            self.retired_snapshot = None;
            self.plan = None;
        }
        if self.plan.as_ref().is_some_and(|p| p.symbol != symbol) {
            self.plan = None;
        }
        if self.plan.as_ref().is_some_and(|p| !valid_plan(p, now)) {
            self.plan = None;
            state = "expired";
            reason = "Escenario caducado o inválido";
        }
        if let Some(plan) = &self.plan {
            let sign = if plan.side == "CALL" { 1. } else { -1. };
            let valid = closed_bars(bars, symbol, now);
            let lost = valid.as_ref().is_some_and(|v| {
                v.len() >= 2
                    && v[v.len() - 1].start_ms - v[v.len() - 2].start_ms == BAR_MS
                    && v.iter().rev().take(2).all(|b| {
                        b.start_ms >= plan.created_ms
                            && (b.close - plan.anchor.price) * sign < -plan.anchor.price * 0.0003
                    })
            });
            let reached = valid.as_ref().and_then(|v| v.last()).is_some_and(|b| {
                b.start_ms >= plan.created_ms && (b.close - plan.target.price) * sign >= 0.
            });
            if active && context.spot.is_some() && lost {
                state = "invalidated";
                reason = "Dos cierres de 30 minutos pierden el ancla congelada";
            } else if active && context.spot.is_some() && reached {
                state = "reached";
                reason = "Cierre alcanza referencia; no equivale a resultado de una operación";
            } else if !context.available
                || !context.blockers.is_empty()
                || matches!(context.state.as_str(), "mixed" | "pending")
            {
                state = "suspended";
                reason = "Evidencia insuficiente: conservar referencias sin confirmar continuidad";
            } else if context.side != plan.side {
                state = "divergence";
                reason = "Estructura actual contraria al escenario congelado";
            } else {
                state = "observing";
                reason = "Escenario condicional; confirmación intradía separada";
            }
        } else if state != "expired"
            && context.state == "conditional"
            && context.blockers.is_empty()
            && self.retired_snapshot != context.snapshot_at
        {
            if let Some((anchor, target)) = context.anchor.clone().zip(context.next.clone()) {
                let expires = anchor
                    .expirations
                    .last()
                    .unwrap()
                    .clone()
                    .min(target.expirations.last().unwrap().clone());
                self.plan = Some(Plan {
                    version: VERSION.into(),
                    symbol: symbol.into(),
                    side: context.side.clone(),
                    created_ms: now,
                    anchor,
                    target,
                    expires,
                });
                state = "observing";
                reason = "Referencias congeladas; no es señal de entrada";
            }
        }
        let plan = self.plan.clone();
        let intraday = confirmation(
            if active && context.spot.is_some() {
                plan.as_ref()
                    .map_or(context.side.as_str(), |p| p.side.as_str())
            } else {
                "PENDIENTE"
            },
            symbol,
            e,
            now,
        );
        let remaining = if matches!(state, "observing" | "divergence") {
            context
                .spot
                .zip(plan.as_ref())
                .map(|(spot, p)| (p.target.price - spot) * if p.side == "CALL" { 1. } else { -1. })
        } else {
            None
        };
        let atr = daily_atr_scale(f, symbol, now);
        if matches!(state, "invalidated" | "reached") {
            self.retired_snapshot = context.snapshot_at;
            self.plan = None;
        }
        Assessment {
            state: state.into(),
            context,
            plan,
            intraday,
            remaining_dollars: remaining,
            remaining_daily_atr: remaining
                .zip(atr)
                .map(|(d, a)| d / a)
                .filter(|n| n.is_finite()),
            reason: reason.into(),
        }
    }
}
fn daily_atr_scale(f: &Value, symbol: &str, now: u64) -> Option<f64> {
    let a = &f["dailyAtr"];
    let expected = snipers_core::atr::sessions(now)?.last()?.date.clone();
    (a["symbol"] == symbol && a["through"] == expected)
        .then(|| p(&a["value"]))
        .flatten()
}
/// Adapter for existing validated readings. Does not call apply(), alter admission, or fetch data.
pub fn intraday_evidence(
    f: &Value,
    r: &crate::intraday_strategy::Reading,
    now: u64,
) -> IntradayEvidence {
    let q = &r.quick_context;
    let mut out = IntradayEvidence {
        symbol: r.symbol.clone(),
        at: now,
        ..Default::default()
    };
    if f["symbol"] != r.symbol
        || f["stocks"]["phase"] != "streaming"
        || !fresh(&f["publishedMs"], now, 3000)
        || r.at > now
        || now - r.at > 3000
        || calendar::date_at(r.at) != calendar::date_at(now)
        || !r.ready
    {
        return out;
    }
    if r.price.is_finite() {
        out.price_side = Some(
            if r.price > 0. {
                "CALL"
            } else if r.price < 0. {
                "PUT"
            } else {
                "NEUTRAL"
            }
            .into(),
        );
    }
    if r.flow_one.is_finite()
        && r.flow_five.is_finite()
        && q["symbol"] == r.symbol
        && fresh(&q["at"], now, 3000)
    {
        out.flow_side = Some(
            if r.flow_one > 0. && r.flow_five > 0. {
                "CALL"
            } else if r.flow_one < 0. && r.flow_five < 0. {
                "PUT"
            } else {
                "NEUTRAL"
            }
            .into(),
        );
        out.cvd_side = q["cvd"]["side"]
            .as_str()
            .filter(|s| matches!(*s, "CALL" | "PUT"))
            .map(str::to_owned);
    }
    out.qqq = serde_json::from_value(f["qqq_report"].clone()).ok();
    out
}
