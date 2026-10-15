//! Informational option-contract pace; never historical RVOL or entry permission.
use serde::Serialize;
use serde_json::Value;
#[derive(Clone, Debug, Serialize)]
pub struct Read {
    pub symbol: String,
    pub state: String,
    pub usable: bool,
    pub rvol_local: Option<f64>,
    pub contracts_1m: Option<f64>,
    pub baseline_contracts_per_minute: Option<f64>,
    pub call_contracts_1m: Option<f64>,
    pub put_contracts_1m: Option<f64>,
    pub last_trade_age_seconds: Option<f64>,
    pub source: String,
    pub reason: String,
}
impl Default for Read {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            state: "ESPERANDO".into(),
            usable: false,
            rvol_local: None,
            contracts_1m: None,
            baseline_contracts_per_minute: None,
            call_contracts_1m: None,
            put_contracts_1m: None,
            last_trade_age_seconds: None,
            source: "MASSIVE_OPTIONS_SELECTED_CONTRACTS".into(),
            reason: "SIN FLUJO LIVE VERIFICADO".into(),
        }
    }
}
fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|n| n.is_finite())
}
fn rounded(value: f64, digits: usize) -> f64 {
    format!("{value:.digits$}").parse().unwrap_or(value)
}
pub fn evaluate(symbol: &str, horizon: &str, snapshot: &Value, now: u64) -> Read {
    let mut r = Read {
        symbol: symbol.trim().to_uppercase(),
        ..Default::default()
    };
    macro_rules! wait {
        ($reason:expr) => {{
            r.reason = $reason.into();
            return r;
        }};
    }
    if horizon.to_uppercase() != "DAY" {
        wait!("PROXIMA SESION: SIN VOLUMEN 0DTE LIVE");
    }
    if !snapshot.is_object() {
        return r;
    }
    if snapshot["symbol"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_uppercase()
        != r.symbol
    {
        wait!("EL FLUJO NO ES DEL ACTIVO SELECCIONADO");
    }
    if !["connected", "authenticated", "subscribed"]
        .iter()
        .all(|key| snapshot[key].as_bool() == Some(true))
    {
        wait!("WEBSOCKET SIN SUSCRIPCION LIVE VERIFICADA");
    }
    let windows = &snapshot["windows"];
    if !windows.is_object() {
        return r;
    }
    if !["1m", "5m", "15m"]
        .iter()
        .all(|key| windows[key].is_object())
    {
        wait!("FALTAN VENTANAS DE VOLUMEN");
    }
    let Some(timestamp) = number(&snapshot["last_trade"]["timestamp"]) else {
        wait!("SIN TIMESTAMP DE OPERACION LIVE");
    };
    let age = now as f64 / 1000. - timestamp;
    r.last_trade_age_seconds = Some(rounded(age, 1));
    if timestamp < 0. || timestamp * 1000. > u64::MAX as f64 {
        wait!("FECHA DE OPERACION INVALIDA");
    }
    let trade_ms = (timestamp * 1000.) as u64;
    if crate::calendar::date_at(now).is_none()
        || crate::calendar::date_at(trade_ms) != crate::calendar::date_at(now)
        || !(0.0..=75.0).contains(&age)
    {
        wait!("SIN OPERACIONES LIVE RECIENTES");
    }
    let one = &windows["1m"];
    let five = &windows["5m"];
    let fifteen = &windows["15m"];
    let (
        Some(one_events),
        Some(five_events),
        Some(fifteen_events),
        Some(current),
        Some(total_five),
    ) = (
        number(&one["event_count"]),
        number(&five["event_count"]),
        number(&fifteen["event_count"]),
        number(&one["contracts"]),
        number(&five["contracts"]),
    )
    else {
        wait!("CONTEOS DE CONTRATOS INCOMPLETOS");
    };
    if one_events < 1. || current <= 0. || total_five < current {
        wait!("SIN VOLUMEN VALIDO EN EL ULTIMO MINUTO");
    }
    r.contracts_1m = Some(rounded(current, 2));
    if one["buckets"].is_object() {
        let amount = |key: &str| {
            number(&one["buckets"][key]["contracts"])
                .filter(|n| *n >= 0.)
                .unwrap_or(0.)
        };
        r.call_contracts_1m = Some(rounded(amount("CALL BUY") + amount("CALL SELL"), 2));
        r.put_contracts_1m = Some(rounded(amount("PUT BUY") + amount("PUT SELL"), 2));
    }
    let previous = total_five - current;
    if fifteen_events <= five_events || five_events - one_events < 3. || previous <= 0. {
        wait!("ACUMULANDO BASE LOCAL DE VOLUMEN");
    }
    let baseline = previous / 4.;
    let ratio = current / baseline;
    if !ratio.is_finite() {
        wait!("CONTEOS DE CONTRATOS INCOMPLETOS");
    }
    r.state = if ratio < 0.7 {
        "BAJO"
    } else if ratio < 1. {
        "MEDIO"
    } else if ratio < 1.5 {
        "BUENO"
    } else {
        "FUERTE"
    }
    .into();
    r.usable = true;
    r.rvol_local = Some(rounded(ratio, 3));
    r.baseline_contracts_per_minute = Some(rounded(baseline, 2));
    r.reason = "RITMO LOCAL 1M VS 4 MINUTOS PREVIOS; NO RVOL HISTORICO".into();
    r
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sixty_legacy_local_volume_cases_preserve_thresholds_and_failures() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-local-volume.json"))
                .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 60);
        for (i, c) in cases.iter().enumerate() {
            let result = evaluate(
                c["symbol"].as_str().unwrap(),
                c["horizon"].as_str().unwrap(),
                &c["snapshot"],
                c["nowMs"].as_u64().unwrap(),
            );
            assert_eq!(
                serde_json::to_value(result).unwrap(),
                c["expected"],
                "case {i}"
            );
        }
    }
}
