//! Read-only Snipers evidence and passive Neuro evaluation. No broker or strategy writes.
use massive_feed::live;
use serde::Deserialize;
use serde_json::{json, Value};
use snipers_store::learning::{LearningStore, Observation};
use tauri::Manager;

#[derive(Clone, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Reading {
    symbol: String, generated_ms: u64, bias: f64, max_dte: u16,
    include_zero: bool, convention: String, ready: bool,
}
impl Reading {
    fn valid(&self, now:u64)->bool {
        !self.symbol.is_empty() && self.symbol.len()<=16
        && self.symbol.chars().all(|c|c.is_ascii_uppercase()||c=='.')
        && self.bias.is_finite() && (0.0..=100.0).contains(&self.bias)
        && self.max_dte<=365 && ["std","short","inv"].contains(&self.convention.as_str())
        && self.generated_ms<=now && now-self.generated_ms<=45_000
    }
}

#[tauri::command]
pub async fn neuro_confirmation(app:tauri::AppHandle, task:tauri::State<'_,crate::LiveTask>, reading:Reading)->Result<Value,String> {
    let now=live::now_ms();
    if !reading.valid(now) {return Err("Lectura Neuro inválida o antigua".into());}
    let shared=task.state.lock().map_err(|_|"Estado ocupado")?.clone();
    let connected=task.handle.lock().map_err(|_|"Estado ocupado")?.is_some();
    let directory=app.path().app_local_data_dir().map_err(|_|"Carpeta no disponible")?;
    tauri::async_runtime::spawn_blocking(move || {
        let frame=shared.map(|s|live::snapshot(&s));
        let variant=format!("neuro-v1:{}:{}:{}",reading.max_dte,reading.include_zero,reading.convention);
        let side=if reading.bias>=60.0 {"CALL"}else if reading.bias<=40.0 {"PUT"}else{"NEUTRAL"};
        let mut out=json!({"at":now,"symbol":frame.as_ref().map(|s|&s.symbol),"ready":false,
            "reason":"Conecta Snipers al mismo activo","horizons":[],"flow":null,"paths":[]});
        let mut price=None; let mut price_ms=None; let mut snipers_side="N/A".to_owned();
        if let Some(s)=frame.as_ref().filter(|s|connected && s.symbol==reading.symbol) {
            let tick=s.ticks.get(&s.symbol).filter(|t|t.symbol==s.symbol);
            let fresh=s.stocks.phase=="streaming" && tick.is_some_and(|t|t.trade_ms.is_some_and(|ms|ms<=now&&now-ms<=15_000));
            if fresh {price=tick.and_then(|t|t.price);price_ms=tick.and_then(|t|t.trade_ms);}
            let hs:Vec<_>=s.option_snapshot_status.horizons.iter().map(|h| {
                let r=s.adjusted_readings.get(&h.mode).map(|a|&a.reading).unwrap_or(&h.structure.reading);
                let current=fresh && s.option_snapshot_status.phase=="ready" && h.structure.complete && h.structure.liquidity_complete
                    && r.available && h.observed_ms<=now && now-h.observed_ms<=300_000
                    && !h.expirations.is_empty() && h.expirations.iter().all(|e|snipers_core::calendar::day_bounds(e).is_some_and(|(_,end)|end>now));
                if h.mode=="DAY" && current {snipers_side=r.side.clone();}
                json!({"mode":h.mode,"label":h.label,"expirations":h.expirations,"ready":current,
                    "side":if current {r.side.as_str()}else{"N/A"},"callScore":if current {r.call_score}else{None},
                    "putScore":if current {r.put_score}else{None},"missing":h.structure.missing_contracts,
                    "callWall":if current {h.structure.call_wall.as_ref().map(|w|w.strike)}else{None},
                    "putWall":if current {h.structure.put_wall.as_ref().map(|w|w.strike)}else{None},
                    "routes":if current {json!(h.structure.routes)}else{json!([])}})
            }).collect();
            out["ready"]=json!(fresh);
            out["reason"]=json!(if fresh {"Lecturas independientes; horizontes distintos no confirman una misma señal"}else{"Falta precio vigente de Snipers"});
            out["horizons"]=json!(hs);
            let flow_fresh=fresh && s.options.phase=="streaming" && s.options.last_event_ms.is_some_and(|ms|ms<=now&&now-ms<=15000);
            out["flow"]=if flow_fresh {json!(s.flow.windows)}else{Value::Null};
            out["paths"]=if fresh {json!(s.structural_paths)}else{json!([])};
        }
        let in_session=snipers_core::calendar::date_at(now).is_some_and(|d|matches!(snipers_core::calendar::schedule(d),snipers_core::calendar::Day::Open(s) if now>=s.open_ms&&now<s.close_ms));
        let eligible=in_session && reading.ready && price.is_some();
        let quality=if !in_session {"Fuera de sesión regular"}else if price.is_none(){"Falta precio fresco del mismo activo en Snipers"}else if !reading.ready{"Neuro sin perfil válido"}else{"Registrando lectura"};
        std::fs::create_dir_all(&directory).map_err(|_|"No se pudo abrir aprendizaje")?;
        let path=directory.join("neuro-learning-v1.sqlite");
        LearningStore::open(&path).and_then(|mut db|db.record(&Observation {
            symbol:reading.symbol.clone(),observed_ms:now,price,price_ms,eligible,quality:quality.into(),
            context:json!({"engine_build":"neuro-reading-v1","variant":variant,
                "neuro":{"side":side,"bias":reading.bias,"generatedMs":reading.generated_ms},
                "snipers":{"side":snipers_side,"horizons":out["horizons"]},
                "analyst":{"quick_read":{"quick_read_side":side}},"automatic_model_updates":false})
        })).map_err(|_|"No se pudo guardar el aprendizaje de Neuro")?;
        out["learning"]=snipers_store::neuro_report::read(&path,&reading.symbol,&variant,now).map_err(|_|"No se pudo leer aprendizaje")?;
        out["learningStatus"]=json!(quality);
        Ok(out)
    }).await.map_err(|_|"No se pudo comparar lecturas".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn rejects_old_future_or_invalid_readings() {
        let mut r=Reading{symbol:"QQQ".into(),generated_ms:100_000,bias:70.0,max_dte:45,include_zero:true,convention:"std".into(),ready:true};
        assert!(r.valid(100_000)); assert!(!r.valid(99_999)); assert!(!r.valid(145_001));
        r.bias=f64::NAN; assert!(!r.valid(100_000));
    }
}
