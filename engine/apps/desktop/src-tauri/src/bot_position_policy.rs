//! Explainable Paper position management. No premium thresholds or broker calls.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const GAP_MS: u64 = 120_000;
pub const CLOSE_BUFFER_MS: u64 = 15 * 60 * 1000;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub swing: bool,
    pub underlying: String,
    pub anchor: Option<f64>,
    pub target: Option<f64>,
    pub entered_ms: u64,
    pub missing_since: Option<u64>,
    pub last_bar: u64,
    pub opposing_bars: u32,
    pub fast_opposing_bars: u32,
    pub fast_last_bar: u64,
    pub market_date: String,
    pub pending_exit: Option<String>,
    pub assessment: Value,
}
fn positive(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite() && *n > 0.)
}
impl State {
    pub fn capture(v: &Value, side: &str, now: u64) -> Self {
        let r = &v["reading"];
        let sign = if side == "CALL" { 1. } else { -1. };
        let spot = positive(&r["spot"]).unwrap_or(0.);
        let anchor = r["levels"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(positive)
            .chain(positive(&r["vwap"]))
            .filter(|n| (spot - n) * sign > 0.)
            .min_by(|a, b| (a - spot).abs().total_cmp(&(b - spot).abs()));
        Self {
            underlying:r["symbol"].as_str().unwrap_or("QQQ").into(),
            anchor,
            target: positive(&r["next"]).filter(|target|spot>0.&&(target-spot)*sign>0.),
            entered_ms: now,
            ..Default::default()
        }
    }
    fn evaluate_swing(&mut self,v:&Value,side:&str,now:u64,market_open:bool)->Option<String>{
        if let Some(reason)=self.pending_exit.clone(){return Some(reason);}
        if !market_open{self.missing_since=None;return self.report("HOLD","Mercado cerrado: conservar tesis estructural; reevaluar al abrir",json!([]));}
        let r=&v["reading"];let same=r["symbol"]==self.underlying;
        let fresh=v["reviewedMs"].as_u64().is_some_and(|t|t<=now&&now-t<=5000);
        if !same||!fresh||!(r["management_ready"]==true||r["ready"]==true) {
            let opening=snipers_core::calendar::date_at(now).is_some_and(|d|matches!(snipers_core::calendar::schedule(d),snipers_core::calendar::Day::Open(day) if self.entered_ms<day.open_ms&&now<day.open_ms+180000));
            if opening {self.missing_since=None;return self.report("WAIT","Esperando evidencia de apertura para revisar posición estructural",json!([]));}
            let since=*self.missing_since.get_or_insert(now);
            return self.report(if now.saturating_sub(since)>=GAP_MS{"EXIT"}else{"WAIT"},"Contingencia de datos: recuperar lectura propia del activo; salida tras 120 s sin evidencia",json!([
                {"question":"Activo coincide con la posición","answer":same},
                {"question":"Evaluación recibida en los últimos 5 segundos","answer":fresh},
                {"question":"Datos suficientes para gestionar la posición","answer":r["management_ready"]==true||r["ready"]==true},
                {"question":"Bloqueos de la lectura","answer":r["blockers"]},
                {"question":"Segundos continuos sin datos de gestión","answer":now.saturating_sub(since)/1000}
            ]));
        }
        self.missing_since=None;
        let sign=if side=="CALL"{1.}else{-1.};let spot=positive(&r["spot"]).unwrap_or(0.);
        if matches!(side,"CALL"|"PUT")&&spot>0.&&self.target.is_some_and(|target|(spot-target)*sign>=0.) {
            return self.report("EXIT","Objetivo estructural de entrada alcanzado",json!([
                {"question":"Objetivo fijado al entrar","answer":self.target},
                {"question":"Precio vigente del activo","answer":spot}
            ]));
        }
        if let Some(bar)=r["swing_bars"].as_array().and_then(|rows|rows.last()){
            if let Some(t)=bar["t"].as_u64().filter(|t|*t>=self.entered_ms&&*t>self.last_bar&&*t+1800000<=now&&now-(*t+1800000)<=1800000&&snipers_core::calendar::date_at(*t)==snipers_core::calendar::date_at(now)){
                let lost=self.anchor.zip(positive(&bar["close"])).is_some_and(|(anchor,close)|(close-anchor)*sign<0.&&(spot-anchor)*sign<0.);
                self.opposing_bars=if lost {if self.last_bar.checked_add(1800000)==Some(t){self.opposing_bars+1}else{1}}else{0};self.last_bar=t;
            }
        }
        let thesis=crate::bot_thesis::assess(v,side,&self.underlying,self.anchor,self.target,now);
        let smc=massive_feed::smc::advisory(&r["quick_context"]["smc"]["context"],r,self.anchor,Some(side),now);
        let strong_opposition=thesis["status"]=="level_lost"&&thesis["flowOpposes"]==true&&(thesis["cvd"]=="opposes"||smc["exitReviewSuggested"]==true);
        if let Some(t)=r["closed_bars"].as_array().and_then(|rows|rows.last()).and_then(|bar|bar["t"].as_u64()).filter(|t|*t>=self.entered_ms&&*t>self.fast_last_bar&&t.checked_add(120000).is_some_and(|end|end<=now&&now-end<=180000)&&snipers_core::calendar::date_at(*t)==snipers_core::calendar::date_at(now)){
            let bar=r["closed_bars"].as_array().unwrap().last().unwrap();
            let lost=self.anchor.zip(positive(&bar["close"])).is_some_and(|(a,p)|(p-a)*sign<0.);
            self.fast_opposing_bars=if strong_opposition&&lost{if self.fast_last_bar.checked_add(120000)==Some(t){self.fast_opposing_bars+1}else{1}}else{0};self.fast_last_bar=t;
        }else if !strong_opposition{self.fast_opposing_bars=0;}
        if self.fast_opposing_bars>=2{return self.report("EXIT","Invalidación reforzada: nivel perdido, flujo contrario y confirmación CVD/SMC en dos cierres de 2 minutos",json!([thesis]));}
        let checks=json!([{"question":"¿Se perdió el nivel estructural de entrada en dos cierres de 30 minutos?","answer":format!("{} / 2",self.opposing_bars.min(2))},{"question":"Respaldo actual de la lectura rápida","answer":thesis},{"question":"Pérdida máxima autorizada","answer":"Toda la prima pagada; no ampliar posición para recuperar"}]);
        if self.opposing_bars>=2 {return self.report("EXIT","Tesis invalidada: dos cierres de 30 minutos pierden el nivel estructural",checks);}
        self.report("HOLD","Mantener hacia el objetivo fijado; puede continuar al próximo día. Retroceso o pérdida de prima no invalidan por sí solos; nivel perdido exige dos cierres de 30 minutos",checks)
    }
    fn evaluate_session_end(&mut self, v: &Value, side: &str, now: u64) -> Option<String> {
        let r = &v["reading"];
        let h = &v["holding"];
        let sign = if side == "CALL" { 1. } else { -1. };
        let recent = |t: &Value| t.as_u64().is_some_and(|t| t <= now && now - t <= 5000);
        let premium = positive(&h["entry_premium"]);
        let bid = positive(&h["bid"]);
        let loss = recent(&h["quote_ms"]) && premium.zip(bid).is_some_and(|(entry, bid)| bid < entry);
        let bar = r["closed_bars"].as_array().and_then(|bars| bars.last());
        let bar_current = bar.and_then(|b| b["t"].as_u64()).is_some_and(|t|
            t.checked_add(120000).is_some_and(|end| end <= now && now - end <= 180000)
            && snipers_core::calendar::date_at(t) == snipers_core::calendar::date_at(now));
        let spot = positive(&r["spot"]);
        let level_held = self.anchor.zip(spot).is_some_and(|(anchor, spot)|
            (spot - anchor) * sign > 0.
            && bar.and_then(|b| positive(&b["close"])).is_some_and(|close| (close - anchor) * sign > 0.));
        let target_ahead = positive(&h["target"]).zip(spot).is_some_and(|(target, spot)| (target - spot) * sign > 0.)
            && positive(&r["next"]).zip(spot).is_some_and(|(target, spot)| (target - spot) * sign > 0.);
        let supported = matches!(side, "CALL" | "PUT")
            && r["symbol"] == if self.underlying.is_empty() { "QQQ" } else { &self.underlying }
            && recent(&v["reviewedMs"]) && r["ready"] == true && bar_current && level_held && target_ahead
            && r["net"].as_f64().is_some_and(|n| n.is_finite() && n * sign >= 0.15)
            && ["flow_one", "flow_five"].iter().all(|k| r[k].as_f64().is_some_and(|n| n.is_finite() && n * sign > 0.));
        let checks = json!([
            {"question":"¿Está en pérdida con bid vigente frente a la prima ejecutada?", "answer":loss},
            {"question":"¿Se mantiene el nivel defendido?", "answer":level_held},
            {"question":"¿El objetivo de entrada y el actual siguen pendientes?", "answer":target_ahead},
            {"question":"¿La lectura propia, precio y flujo apoyan continuar?", "answer":supported}
        ]);
        if loss && supported {
            self.report("HOLD", "Excepción al cierre intradía: pérdida con nivel defendido, objetivo pendiente y flujo favorable; no garantiza recuperación", checks)
        } else {
            self.report("EXIT", "Prioridad de cierre el mismo día: no se cumple pérdida con respaldo estructural vigente", checks)
        }
    }
    fn report(&mut self, action: &str, reason: &str, checks: Value) -> Option<String> {
        self.assessment = json!({"action":action,"reason":reason,"checks":checks});
        if action == "EXIT" {
            self.pending_exit = Some(reason.into());
            Some(reason.into())
        } else {
            None
        }
    }
    pub fn evaluate(
        &mut self,
        v: &Value,
        side: &str,
        now: u64,
        close: u64,
        market_open: bool,
    ) -> Option<String> {
        if let Some(reason) = self.pending_exit.clone() { return Some(reason); }
        if !self.swing && market_open && now >= close.saturating_sub(CLOSE_BUFFER_MS) {
            return self.evaluate_session_end(v, side, now);
        }
        if self.swing {return self.evaluate_swing(v,side,now,market_open);}
        if let Some(reason) = self.pending_exit.clone() {
            return Some(reason);
        }
        if !market_open {
            self.missing_since = None;
            self.opposing_bars = 0;
            return self.report(
                "WAIT",
                "Mercado cerrado: posición conservada; reevaluar al abrir",
                json!([]),
            );
        }
        let date = snipers_core::calendar::date_at(now)
            .map(|d| d.to_string())
            .unwrap_or_default();
        if date != self.market_date {
            self.market_date = date;
            self.last_bar = 0;
            self.opposing_bars = 0;
            self.missing_since = None;
        }
        let r = &v["reading"];
        let bars = r["closed_bars"].as_array();
        let last = bars.and_then(|b| b.last());
        let fresh = v["reviewedMs"]
            .as_u64()
            .is_some_and(|t| t <= now && now - t <= 5000);
        let bar_ms = last.and_then(|b| b["t"].as_u64()).unwrap_or(0);
        let bar_valid = bar_ms
            .checked_add(120_000)
            .is_some_and(|end| end <= now && now - end <= 180_000)
            && snipers_core::calendar::date_at(bar_ms) == snipers_core::calendar::date_at(now);
        let complete = fresh
            && r["ready"] == true
            && r["symbol"] == if self.underlying.is_empty(){"QQQ"}else{&self.underlying}
            && matches!(side, "CALL" | "PUT")
            && positive(&r["vwap"]).is_some()
            && positive(&r["spot"]).is_some()
            && bar_valid
            && last.and_then(|b| positive(&b["close"])).is_some()
            && ["net", "flow_one", "flow_five"]
                .iter()
                .all(|k| r[k].as_f64().is_some_and(|n| n.is_finite()));
        if !complete {
            self.opposing_bars = 0;
            let warming_up = snipers_core::calendar::date_at(now).is_some_and(|date| {
                matches!(snipers_core::calendar::schedule(date), snipers_core::calendar::Day::Open(day)
                    if self.entered_ms < day.open_ms && now < day.open_ms + 180_000)
            });
            if warming_up {
                self.missing_since = None;
                return self.report(
                    "WAIT",
                    "Apertura: hasta 3 minutos para recibir velas y evidencia de la nueva sesión",
                    json!([]),
                );
            }
            let start = self.missing_since.get_or_insert(now);
            if now < *start {
                *start = now;
            }
            return if now.saturating_sub(*start) >= GAP_MS {
                self.report("EXIT", "Contingencia: 120 s continuos sin evidencia suficiente durante mercado abierto", json!([]))
            } else {
                self.report(
                    "WAIT",
                    "Evidencia incompleta: esperar recuperación, sin asumir cambio de tendencia",
                    json!([]),
                )
            };
        }
        self.missing_since = None;
        let sign = if side == "CALL" { 1. } else { -1. };
        let bar = last.unwrap();
        let price = positive(&bar["close"]).unwrap();
        let spot = positive(&r["spot"]).unwrap();
        let vwap = positive(&r["vwap"]).unwrap();
        let anchor_lost = self
            .anchor
            .is_some_and(|a| (price - a) * sign < 0. && (spot - a) * sign < 0.);
        let vwap_lost = (price - vwap) * sign < 0. && (spot - vwap) * sign < 0.;
        let flow_against = r["flow_one"].as_f64().unwrap() * sign <= -0.15
            && r["flow_five"].as_f64().unwrap() * sign < 0.;
        let opposite = flow_against
            && r["net"].as_f64().unwrap() * sign <= -0.15
            && (anchor_lost || vwap_lost);
        // Count distinct, consecutive, fully post-entry closed candles, never quote ticks.
        if bar_ms > self.last_bar && bar_ms >= self.entered_ms {
            self.opposing_bars = if opposite {
                if self.last_bar.checked_add(120_000) == Some(bar_ms) {
                    self.opposing_bars + 1
                } else {
                    1
                }
            } else {
                0
            };
            self.last_bar = bar_ms;
        } else if !opposite {
            self.opposing_bars = 0;
        }
        let checks = json!([
            {"question":"¿Se conserva el nivel defendido al entrar?", "answer":if self.anchor.is_none(){"Sin nivel registrado"}else if anchor_lost{"Nivel perdido"}else{"Sí"}},
            {"question":"¿Precio cerrado y actual perdieron VWAP?", "answer":if vwap_lost{"Sí"}else{"No"}},
            {"question":"¿El flujo de 1 y 5 minutos confirma fuerza contraria?", "answer":if flow_against{"Sí"}else{"No"}},
            {"question":"¿Hay dos velas cerradas consecutivas de invalidación?", "answer":format!("{} / 2",self.opposing_bars.min(2))}
        ]);
        if self.opposing_bars >= 2 {
            return self.report("EXIT", "Invalidación confirmada: nivel o VWAP perdido, flujo contrario y dos velas de 2 minutos", checks);
        }
        self.report(
            "HOLD",
            if opposite {
                "Posible invalidación: esperar confirmación en otra vela cerrada"
            } else {
                "Mantener: el retroceso no confirma invalidación de la operación"
            },
            checks,
        )
    }
}

pub fn expires_today_or_earlier(symbol: &str, now: u64) -> bool {
    let Some(tail) = symbol.get(symbol.len().saturating_sub(15)..) else {
        return true;
    };
    let Some(date) = tail
        .get(..6)
        .filter(|d| d.bytes().all(|b| b.is_ascii_digit()))
    else {
        return true;
    };
    let expiry = format!("20{}-{}-{}", &date[..2], &date[2..4], &date[4..6]);
    let today = snipers_core::calendar::date_at(now)
        .map(|d| d.to_string())
        .unwrap_or_default();
    snipers_core::calendar::day_bounds(&expiry).is_none() || expiry <= today
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smc_exit_requires_own_fresh_context_and_two_closed_bars() {
        let at=now();
        for valid in [true,false] {
            let mut initial=reading(at,"CALL",false);
            initial["reading"]["next"]=json!(510.);
            let mut state=State::capture(&initial,"CALL",at); state.swing=true;
            for elapsed in [120000,240000] {
                let t=at+elapsed;
                let mut v=reading(t,"CALL",true);
                v["reading"]["at"]=json!(t);
                v["reading"]["quick_context"]=json!({"symbol":"QQQ","at":t,"smc":{"context":{"version":massive_feed::smc::VERSION,"symbol":if valid{"QQQ"}else{"SPY"},"state":"observing","at":t,"layers":[{"timeframe":2,"segmentStart":at,"events":[{"at":t,"kind":"CHOCH","side":-1}]}]}}});
                assert_eq!(state.evaluate(&v,"CALL",t,u64::MAX,true).is_some(),valid && elapsed==240000);
                if elapsed==120000 { assert!(state.evaluate(&v,"CALL",t,u64::MAX,true).is_none()); }
            }
        }
    }
    #[test]fn overnight_is_independent_of_premium_profit_and_exits_at_frozen_target(){
        let close=snipers_core::calendar::rfc3339_ms("2026-09-28T20:00:00Z").unwrap();let at=close-600000;
        for side in ["CALL","PUT"]{let sign=if side=="CALL"{1.}else{-1.};for bid in [1.,3.]{
            let mut v=reading(at,side,false);v["reading"]["next"]=json!(500.+sign*10.);v["holding"]=json!({"entry_premium":2.,"bid":bid,"quote_ms":at});
            let mut s=State::capture(&v,side,at-3600000);s.swing=true;assert!(s.evaluate(&v,side,at,close,true).is_none());
            assert!(s.evaluate(&v,side,close+1,close,false).is_none());
            let next=snipers_core::calendar::rfc3339_ms("2026-09-29T15:00:00Z").unwrap();v=reading(next,side,false);v["reading"]["spot"]=json!(500.+sign*10.);
            assert!(s.evaluate(&v,side,next,u64::MAX,true).is_some());
        }}
    }
    #[test]fn reinforced_invalidation_requires_two_distinct_closed_bars_and_own_cvd(){
        let at=now();for side in ["CALL","PUT"]{let mut initial=reading(at,side,false);initial["reading"]["next"]=json!(if side=="CALL"{510.}else{490.});let mut s=State::capture(&initial,side,at);s.swing=true;
            for (elapsed,exit) in [(120000,false),(240000,true)]{let mut v=reading(at+elapsed,side,true);v["reading"]["quick_context"]=json!({"symbol":"QQQ","at":at+elapsed,"cvd":{"side":if side=="CALL"{"PUT"}else{"CALL"}}});
                assert_eq!(s.evaluate(&v,side,at+elapsed,u64::MAX,true).is_some(),exit);
                if !exit{assert!(s.evaluate(&v,side,at+elapsed+1,u64::MAX,true).is_none());}
            }
        }
    }
    #[test]
    fn frozen_target_survives_loss_retargeting_and_restart_for_calls_and_puts(){
        let at=now();
        for side in ["CALL","PUT"]{
            let sign=if side=="CALL"{1.}else{-1.};
            let mut initial=reading(at,side,false);
            initial["reading"]["spot"]=json!(500.);
            initial["reading"]["next"]=json!(500.+sign*10.);
            let mut state=State::capture(&initial,side,at);state.swing=true;
            let mut v=reading(at+1000,side,false);
            v["reading"]["spot"]=json!(500.-sign*2.);
            v["reading"]["next"]=json!(500.+sign*30.);
            v["holding"]=json!({"entry_premium":3.,"bid":1.});
            assert!(state.evaluate(&v,side,at+1000,u64::MAX,true).is_none());
            let mut restored:State=serde_json::from_value(json!(state)).unwrap();
            v["reviewedMs"]=json!(at+2000);v["reading"]["spot"]=json!(500.+sign*10.);
            assert_eq!(restored.evaluate(&v,side,at+2000,u64::MAX,true).as_deref(),Some("Objetivo estructural de entrada alcanzado"));
        }
    }
    #[test] fn swing_does_not_treat_missing_entry_flow_as_loss_of_management_data(){
        let at=now();
        let mut state=State::capture(&reading(at,"PUT",false),"PUT",at);state.swing=true;
        for elapsed in [0,60000,120001,240000] {
            let mut v=reading(at+elapsed,"PUT",false);
            v["reading"]["ready"]=json!(false);
            v["reading"]["management_ready"]=json!(true);
            assert!(state.evaluate(&v,"PUT",at+elapsed,u64::MAX,true).is_none());
            assert!(state.missing_since.is_none());
        }
        let mut v=reading(at+250000,"PUT",false);
        v["reading"]["ready"]=json!(false);v["reading"]["management_ready"]=json!(false);
        assert!(state.evaluate(&v,"PUT",at+250000,u64::MAX,true).is_none());
        v["reviewedMs"]=json!(at+370001);
        assert!(state.evaluate(&v,"PUT",at+370001,u64::MAX,true).is_some());
    }
    #[test] fn swing_ignores_intraday_noise_but_exits_on_two_thirty_minute_closes(){
        let at=now();
        for side in ["CALL","PUT"] {
            let mut state=State::capture(&reading(at,side,false),side,at);state.swing=true;
            let mut v=reading(at+120000,side,true);
            assert!(state.evaluate(&v,side,at+120000,u64::MAX,true).is_none());
            for (offset,exit) in [(1800000,false),(3600000,true)] {
                v=reading(at+offset,side,true);v["reading"]["swing_bars"]=json!([{"t":at+offset-1800000,"close":if side=="CALL"{499.}else{501.}}]);
                assert_eq!(state.evaluate(&v,side,at+offset,u64::MAX,true).is_some(),exit);
                if !exit {assert!(state.evaluate(&v,side,at+offset,u64::MAX,true).is_none());}
            }
            let mut restored:State=serde_json::from_value(json!(state)).unwrap();
            assert!(restored.evaluate(&reading(at+3600000,side,false),side,at+3600000,u64::MAX,true).is_some());
        }
    }
    fn now() -> u64 {
        snipers_core::calendar::rfc3339_ms("2026-09-28T15:00:00Z").unwrap()
    }
    fn reading(at: u64, side: &str, opposing: bool) -> Value {
        let sign = if side == "CALL" { 1. } else { -1. };
        let d = sign * if opposing { -1. } else { 1. };
        json!({"reviewedMs":at,"reading":{"ready":true,"symbol":"QQQ","spot":500.+d,"vwap":500.,"levels":[500.],"net":0.7*d,"flow_one":0.4*d,"flow_five":0.3*d,"closed_bars":[{"t":at-120000,"close":500.+d}]}})
    }
    #[test]
    fn pullback_and_duplicate_ticks_never_confirm_exit_but_two_closed_bars_do() {
        for side in ["CALL", "PUT"] {
            let at = now();
            let mut s = State::capture(&reading(at, side, false), side, at - 240000);
            let mut pullback = reading(at, side, true);
            pullback["reading"]["flow_five"] = json!(if side == "CALL" { 0.2 } else { -0.2 });
            assert!(s.evaluate(&pullback, side, at, u64::MAX, true).is_none());
            let a = reading(at + 120000, side, true);
            assert!(s.evaluate(&a, side, at + 120000, u64::MAX, true).is_none());
            assert!(s.evaluate(&a, side, at + 120001, u64::MAX, true).is_none());
            assert!(s
                .evaluate(
                    &reading(at + 240000, side, true),
                    side,
                    at + 240000,
                    u64::MAX,
                    true
                )
                .is_some());
            let mut restored: State =
                serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
            assert!(restored
                .evaluate(
                    &reading(at + 240000, side, false),
                    side,
                    at + 240000,
                    u64::MAX,
                    true
                )
                .is_some());
        }
    }
    #[test]
    fn missing_data_recovers_or_exits_after_continuous_timeout_and_survives_restart() {
        let at = now();
        let mut s = State::default();
        assert!(s
            .evaluate(&Value::Null, "CALL", at, u64::MAX, true)
            .is_none());
        assert!(s
            .evaluate(
                &reading(at + 60000, "CALL", false),
                "CALL",
                at + 60000,
                u64::MAX,
                true
            )
            .is_none());
        assert!(s.missing_since.is_none());
        assert!(s
            .evaluate(&Value::Null, "CALL", at + 70000, u64::MAX, true)
            .is_none());
        let mut restored: State =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert!(restored
            .evaluate(&Value::Null, "CALL", at + 189999, u64::MAX, true)
            .is_none());
        assert!(restored
            .evaluate(&Value::Null, "CALL", at + 190000, u64::MAX, true)
            .is_some());
    }
    #[test]
    fn session_end_only_allows_a_verified_loss_with_supported_pending_move() {
        for close in ["2026-09-28T20:00:00Z", "2026-11-27T18:00:00Z"] {
            let close = snipers_core::calendar::rfc3339_ms(close).unwrap();
            let at = close - CLOSE_BUFFER_MS;
            for swing in [false] {
                for side in ["CALL", "PUT"] {
                    let sign = if side == "CALL" { 1. } else { -1. };
                    let mut v = reading(at, side, false);
                    v["reading"]["next"] = json!(500. + sign * 10.);
                    v["holding"] = json!({"entry_premium":2.,"bid":1.5,"quote_ms":at,"target":500.+sign*10.});
                    let mut initial = State::capture(&v, side, at - 3600000);
                    initial.swing = swing;
                    let mut state = initial.clone();
                    assert!(state.evaluate(&v, side, at, close, true).is_none());
                    assert!(state.assessment["reason"].as_str().unwrap().contains("Excepción"));
                    for (field, value) in [("bid",json!(2.)),("bid",json!(2.5)),("quote_ms",json!(at-5001)),("entry_premium",Value::Null),("target",json!(500.))] {
                        let mut bad = v.clone(); bad["holding"][field] = value;
                        assert!(initial.clone().evaluate(&bad, side, at, close, true).is_some());
                    }
                    for (field, value) in [("ready",json!(false)),("flow_five",json!(-sign)),("spot",json!(500.-sign)),("next",json!(500.)),("symbol",json!("SPY"))] {
                        let mut bad = v.clone(); bad["reading"][field] = value;
                        assert!(initial.clone().evaluate(&bad, side, at, close, true).is_some());
                    }
                    assert!(state.evaluate(&Value::Null, side, at, close, true).is_some());
                    let mut restored: State = serde_json::from_value(json!(state)).unwrap();
                    assert!(restored.evaluate(&v, side, at, close, true).is_some());
                }
            }
        }
    }
    #[test]
    fn opening_warmup_is_bounded_and_expiry_prevents_overnight() {
        let open = snipers_core::calendar::rfc3339_ms("2026-09-28T13:30:00Z").unwrap();
        let mut s = State {
            entered_ms: open - 86400000,
            ..Default::default()
        };
        assert!(s
            .evaluate(&Value::Null, "CALL", open, u64::MAX, true)
            .is_none());
        assert!(s
            .evaluate(&Value::Null, "CALL", open + 179999, u64::MAX, true)
            .is_none());
        assert!(s.missing_since.is_none());
        assert!(s
            .evaluate(&Value::Null, "CALL", open + 180000, u64::MAX, true)
            .is_none());
        assert!(s
            .evaluate(&Value::Null, "CALL", open + 300000, u64::MAX, true)
            .is_some());
        assert!(expires_today_or_earlier("QQQ260928C00500000", open));
        assert!(!expires_today_or_earlier("QQQ261005C00500000", open));
        assert!(expires_today_or_earlier("bad", open));
    }
    #[test]
    fn preentry_candles_gaps_and_invalid_data_cannot_confirm_a_reversal() {
        let at = now();
        let mut s = State {
            entered_ms: at,
            ..Default::default()
        };
        assert!(s
            .evaluate(&reading(at, "PUT", true), "PUT", at, u64::MAX, true)
            .is_none());
        assert_eq!(s.opposing_bars, 0);
        assert!(s
            .evaluate(
                &reading(at + 120000, "PUT", true),
                "PUT",
                at + 120000,
                u64::MAX,
                true
            )
            .is_none());
        assert!(s
            .evaluate(
                &reading(at + 480000, "PUT", true),
                "PUT",
                at + 480000,
                u64::MAX,
                true
            )
            .is_none());
        assert_eq!(s.opposing_bars, 1);
        let mut bad = reading(at + 480000, "PUT", true);
        bad["reading"]["vwap"] = Value::Null;
        assert!(s
            .evaluate(&bad, "PUT", at + 480000, u64::MAX, true)
            .is_none());
        assert_eq!(s.opposing_bars, 0);
    }
}
