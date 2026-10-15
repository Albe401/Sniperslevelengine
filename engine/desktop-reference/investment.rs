use massive_feed::investment::{Config, Report};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::Manager;
fn save_opportunity_history(c:&mut snipers_core::investment::Company,dir:&std::path::Path)->Result<(),String>{
    let Some(o)=c.opportunity.as_mut() else{return Ok(())};
    if c.ticker.is_empty()||!c.ticker.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit()||b==b'.'||b==b'-'){return Err("Símbolo de historial inválido".into());}
    let dir=dir.join("investment-opportunity-history");std::fs::create_dir_all(&dir).map_err(|_|"No se pudo crear historial")?;
    let path=dir.join(format!("{}.json",c.ticker));
    let mut history:Vec<snipers_core::opportunity::History>=if path.is_file(){let bytes=std::fs::read(&path).map_err(|_|"No se pudo leer historial")?;if bytes.len()>256000{return Err("Historial excede límite".into());}serde_json::from_slice(&bytes).map_err(|_|"Historial inválido; se preservó el original")?}else{vec![]};
    if !history.iter().any(|h|h.at==o.at&&h.version==o.version){history.push(snipers_core::opportunity::History{at:o.at,price_date:c.price_date.clone(),score:o.score,coverage:o.coverage,fair_value:o.fair_value,version:o.version.clone()});}
    history.sort_by_key(|h|h.at);if history.len()>50{history.drain(..history.len()-50);}
    let bytes=serde_json::to_vec(&history).map_err(|_|"No se pudo serializar historial")?;
    let temp=path.with_extension("tmp");std::fs::write(&temp,bytes).and_then(|_|std::fs::rename(&temp,&path)).map_err(|_|"No se pudo guardar historial")?;o.history=history;Ok(())
}
#[derive(Default)]
pub struct Task {
    busy: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    report: Arc<Mutex<Option<Report>>>,
    individual: Mutex<Option<snipers_core::investment::Company>>,
}
#[tauri::command]
pub async fn research_investment_company(
    app: tauri::AppHandle,
    task: tauri::State<'_, Task>,
    ticker: String,
    date: String,
    session: String,
    minimum: f64,
    structure_only: Option<bool>,
    individual: Option<bool>,
) -> Result<(), String> {
    if task.busy.swap(true, Ordering::SeqCst) {
        return Err("Espera a que termine la consulta actual".into());
    }
    struct Release(Arc<AtomicBool>);
    impl Drop for Release {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let _release = Release(task.busy.clone());
    let individual = individual.unwrap_or(false);
    let (price, price_date) = if individual {
        let guard = task.individual.lock().map_err(|_| "Ficha no disponible")?;
        let c = guard
            .as_ref()
            .filter(|c| c.ticker == ticker)
            .ok_or("Primero analiza esta compañía")?;
        let price = c
            .metrics
            .iter()
            .find(|m| m.id == "last_close")
            .and_then(|m| m.value)
            .ok_or("Empresa sin cierre verificado")?;
        (price, c.price_date.clone())
    } else {
        let guard = task.report.lock().map_err(|_| "Radar no disponible")?;
        let c = guard
            .as_ref()
            .and_then(|r| r.rows.iter().find(|c| c.ticker == ticker))
            .ok_or("Primero explora esta empresa")?;
        let price = c
            .metrics
            .iter()
            .find(|m| m.id == "last_close")
            .and_then(|m| m.value)
            .ok_or("Empresa sin cierre verificado")?;
        (price, c.price_date.clone())
    };
    let r = massive_feed::investment::research_mode(
        &ticker,
        price,
        &price_date,
        &date,
        &session,
        minimum,
        structure_only.unwrap_or(false),
    )
    .await?;
    if individual {
        let mut guard = task.individual.lock().map_err(|_| "Ficha no disponible")?;
        if let Some(c) = guard.as_mut().filter(|c| c.ticker == ticker) {
            c.research = Some(r);
        }
        return Ok(());
    }
    let mut guard = task.report.lock().map_err(|_| "Radar no disponible")?;
    if let Some(report) = guard.as_mut() {
        if let Some(c) = report.rows.iter_mut().find(|c| c.ticker == ticker) {
            c.research = Some(r);
        }
        let dir = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "Carpeta no disponible")?;
        let bytes =
            serde_json::to_vec(report).map_err(|_| "No se pudo guardar la investigación")?;
        std::fs::write(dir.join("investment-radar-v2.tmp"), bytes)
            .and_then(|_| {
                std::fs::rename(
                    dir.join("investment-radar-v2.tmp"),
                    dir.join("investment-radar-v2.json"),
                )
            })
            .map_err(|_| "Lectura disponible en memoria; no se pudo guardar")?;
    }
    Ok(())
}
#[tauri::command]
pub fn get_investment_radar(
    app: tauri::AppHandle,
    task: tauri::State<'_, Task>,
) -> Result<Option<Report>, String> {
    let mut current = task.report.lock().map_err(|_| "Radar no disponible")?;
    if current.is_none() {
        let dir = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "Carpeta local no disponible")?;
        if let Ok(raw) = std::fs::read(dir.join("investment-radar-v2.json")) {
            if raw.len() < 8_000_000 {
                if let Ok(mut r) = serde_json::from_slice::<Report>(&raw) {
                    if r.version == 2 {
                        if r.running {
                            r.running = false;
                            r.canceled = true;
                            r.current =
                                "Exploración interrumpida al cerrar; resultados parciales".into();
                        }
                        *current = Some(r);
                    }
                }
            }
        }
    }
    Ok(current.clone())
}
#[tauri::command]
pub fn stop_investment_radar(task: tauri::State<'_, Task>) {
    task.cancel.store(true, Ordering::SeqCst);
}
#[tauri::command]
pub fn start_investment_radar(
    app: tauri::AppHandle,
    task: tauri::State<'_, Task>,
    config: Config,
) -> Result<(), String> {
    config.validate()?;
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta local no disponible")?;
    std::fs::create_dir_all(&dir).map_err(|_| "No se pudo preparar caché del radar")?;
    if task.busy.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una exploración en curso".into());
    }
    task.cancel.store(false, Ordering::SeqCst);
    let busy = task.busy.clone();
    let cancel = task.cancel.clone();
    let output = task.report.clone();
    let previous = output.lock().ok().and_then(|r| r.clone());
    if let Ok(mut r) = output.lock() {
        *r = Some(Report {
            version: 2,
            running: true,
            started_ms: massive_feed::live::now_ms(),
            config: config.clone(),
            current: "Preparando análisis".into(),
            ..Default::default()
        });
    }
    tauri::async_runtime::spawn(async move {
        struct Release(Arc<AtomicBool>);
        impl Drop for Release {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let _release = Release(busy);
        let result = massive_feed::investment::scan(config, cancel, |mut r| {
            if !r.running {for c in &mut r.rows {if let Err(e)=save_opportunity_history(c,&dir){c.source_errors.push(e);}}}
            if let Some(old) = &previous {
                preserve_research(&mut r, old, massive_feed::live::now_ms());
            }
            if let Ok(mut v) = output.lock() {
                *v = Some(r);
            }
        })
        .await;
        if let Ok(mut guard) = output.lock() {
            if let Err(e) = result {
                if let Some(r) = guard.as_mut() {
                    r.running = false;
                    r.error = Some(e);
                    r.finished_ms = massive_feed::live::now_ms();
                    r.current = "No se pudo completar la exploración".into();
                }
            }
            if let Some(r) = guard.as_mut() {
                let persisted = serde_json::to_vec(r)
                    .map_err(|_| ())
                    .and_then(|bytes| {
                        std::fs::write(dir.join("investment-radar-v2.tmp"), bytes).map_err(|_| ())
                    })
                    .and_then(|_| {
                        std::fs::rename(
                            dir.join("investment-radar-v2.tmp"),
                            dir.join("investment-radar-v2.json"),
                        )
                        .map_err(|_| ())
                    });
                if persisted.is_err() {
                    r.error = Some(
                        "Resultados disponibles en memoria; no se pudo guardar la caché local"
                            .into(),
                    );
                }
            }
        }
    });
    Ok(())
}

fn preserve_research(next: &mut Report, old: &Report, now: u64) {
    for c in &mut next.rows {
        if c.research.is_some() {
            continue;
        }
        let Some(prior) = old
            .rows
            .iter()
            .find(|p| p.ticker == c.ticker && p.price_date == c.price_date)
        else {
            continue;
        };
        let Some(r) = &prior.research else {
            continue;
        };
        let price = c
            .metrics
            .iter()
            .find(|m| m.id == "last_close")
            .and_then(|m| m.value);
        if r.fetched_ms <= now
            && now - r.fetched_ms <= 86_400_000
            && price == Some(r.reference_price)
            && r.reference_date == c.price_date
        {
            c.research = Some(r.clone());
        }
    }
}

#[cfg(test)]
mod preservation_tests {
    use super::*;
    fn company() -> snipers_core::investment::Company {
        serde_json::from_value(serde_json::json!({"ticker":"TEST","name":"Test","industry":"","sic":"","cik":"","asOf":"2026-09-25","period":"","filing":"","priceDate":"2026-09-25","score":null,"coverage":0.,"status":"","metrics":[{"id":"last_close","label":"close","group":"price","value":100.,"unit":"usd","source":"bars"}],"checks":[],"warnings":[],"annual":[],"closes":[],"sourceErrors":[],"eligible":false})).unwrap()
    }
    #[test]
    fn opportunity_history_is_bounded_deduplicated_and_preserves_corruption(){
        let dir=tempfile::tempdir().unwrap();let mut c=company();
        c.opportunity=Some(snipers_core::opportunity::calculate(&c,1));
        save_opportunity_history(&mut c,dir.path()).unwrap();save_opportunity_history(&mut c,dir.path()).unwrap();
        assert_eq!(c.opportunity.as_ref().unwrap().history.len(),1);
        for at in 2..=60 {c.opportunity.as_mut().unwrap().at=at;save_opportunity_history(&mut c,dir.path()).unwrap();}
        assert_eq!(c.opportunity.as_ref().unwrap().history.len(),50);
        let path=dir.path().join("investment-opportunity-history/TEST.json");std::fs::write(&path,b"invalid").unwrap();
        assert!(save_opportunity_history(&mut c,dir.path()).is_err());assert_eq!(std::fs::read(&path).unwrap(),b"invalid");
    }
    fn old() -> Report {
        let mut c = company();
        c.research = Some(snipers_core::investment_structure::Research {
            ticker: "TEST".into(),
            reference_price: 100.,
            reference_date: c.price_date.clone(),
            fetched_ms: 1000,
            ..Default::default()
        });
        Report {
            rows: vec![c],
            ..Default::default()
        }
    }
    #[test]
    fn rescan_keeps_current_levels() {
        let mut next = Report {
            rows: vec![company()],
            ..Default::default()
        };
        preserve_research(&mut next, &old(), 2000);
        assert!(next.rows[0].research.is_some());
    }
    #[test]
    fn changed_close_or_expired_levels_are_not_reused() {
        for mode in 0..3 {
            let mut next = Report {
                rows: vec![company()],
                ..Default::default()
            };
            if mode == 0 {
                next.rows[0].price_date = "2026-09-24".into();
            }
            if mode == 1 {
                next.rows[0].metrics[0].value = Some(101.);
            }
            preserve_research(&mut next, &old(), if mode == 2 { 90_000_000 } else { 2000 });
            assert!(next.rows[0].research.is_none());
        }
    }
}

#[tauri::command]
pub async fn analyze_investment_individual(
    app: tauri::AppHandle,
    task: tauri::State<'_, Task>,
    ticker: String,
) -> Result<snipers_core::investment::Company, String> {
    if task.busy.swap(true, Ordering::SeqCst) {
        return Err("Espera a que termine la consulta actual".into());
    }
    struct Release(Arc<AtomicBool>);
    impl Drop for Release {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let _release = Release(task.busy.clone());
    let mut company = massive_feed::investment::analyze_individual(&ticker).await?;
    let dir=app.path().app_local_data_dir().map_err(|_|"Carpeta local no disponible")?;
    if let Err(e)=save_opportunity_history(&mut company,&dir){company.source_errors.push(e);}
    *task
        .individual
        .lock()
        .map_err(|_| "Ficha individual no disponible")? = Some(company.clone());
    Ok(company)
}
#[tauri::command]
pub fn get_investment_individual(
    task: tauri::State<'_, Task>,
) -> Result<Option<snipers_core::investment::Company>, String> {
    Ok(task
        .individual
        .lock()
        .map_err(|_| "Ficha individual no disponible")?
        .clone())
}

#[derive(serde::Serialize,serde::Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SavedDailyPurchases { pub minimum:f64, pub report:massive_feed::investment::DailyPurchases }
fn purchases_window(date:&str,at:u64,morning_allowed:bool)->Result<(u64,u64),String>{
 let switch=snipers_core::calendar::legacy_eastern_ms(&format!("{date} 16:00:00")).ok_or("Hora de bloques inválida")?;
 let end=snipers_core::calendar::legacy_eastern_ms(&format!("{date} 20:00:00")).ok_or("Hora de bloques inválida")?;
 if morning_allowed&&at<switch {Ok((snipers_core::calendar::legacy_eastern_ms(&format!("{date} 04:00:00")).ok_or("Hora de bloques inválida")?,switch))}else{Ok((switch,end))}
}
fn daily_purchases_dir(app:&tauri::AppHandle,date:&str)->Result<std::path::PathBuf,String>{
 if date.len()!=10||!date.bytes().enumerate().all(|(i,b)|if i==4||i==7{b==b'-'}else{b.is_ascii_digit()}){return Err("Fecha de bloques inválida".into());}
 Ok(app.path().app_local_data_dir().map_err(|_|"Carpeta local no disponible")?.join("daily-purchases").join(date))
}
fn save_daily_purchases(app:&tauri::AppHandle,symbol:&str,minimum:f64,report:massive_feed::investment::DailyPurchases)->Result<(),String>{
 let today=snipers_core::calendar::date_at(massive_feed::live::now_ms()).ok_or("Fecha no disponible")?.to_string();
 if report.date!=today{return Err("Cambió el día de mercado; se conserva la consulta en memoria".into());}
 let dir=daily_purchases_dir(app,&today)?;save_daily_purchases_file(&dir,symbol,minimum,report)
}
fn save_daily_purchases_file(dir:&std::path::Path,symbol:&str,minimum:f64,mut report:massive_feed::investment::DailyPurchases)->Result<(),String>{
 if symbol.is_empty()||symbol.len()>12||!symbol.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit()||b==b'.'||b==b'-'){return Err("Símbolo de bloques inválido".into());}
 let (from,to)=purchases_window(&report.date,report.through_ms,true)?;
 report.rows.retain(|row|row.block.execution_ms.is_some_and(|ms|ms>=from&&ms<to));
 std::fs::create_dir_all(dir).map_err(|_|"No se pudo crear la carpeta de bloques")?;
 let window=if report.through_ms<snipers_core::calendar::legacy_eastern_ms(&format!("{} 16:00:00",report.date)).ok_or("Hora de bloques inválida")?{"morning"}else{"afternoon"};
 let bytes=serde_json::to_vec(&SavedDailyPurchases{minimum,report}).map_err(|_|"No se pudieron serializar los bloques")?;
 if bytes.len()>64_000_000{return Err("La búsqueda supera el límite de almacenamiento local por activo".into());}
 std::fs::write(dir.join(format!("{symbol}-{window}.json")),bytes).map_err(|_|"No se pudieron guardar los bloques".to_string())
}
#[tauri::command]
pub async fn get_saved_daily_purchases(app:tauri::AppHandle,date:String)->Result<Option<SavedDailyPurchases>,String>{
 let now=massive_feed::live::now_ms();let today=snipers_core::calendar::date_at(now).ok_or("Fecha no disponible")?.to_string();
 if date>today{return Err("La fecha de bloques aún no ha ocurrido".into());}
 let (from,to)=purchases_window(&date,now,date==today)?;
 let dir=daily_purchases_dir(&app,&date)?;
 tauri::async_runtime::spawn_blocking(move||load_daily_purchases_dir(&dir,date,from,to)).await.map_err(|_|"No se pudo recuperar el historial de bloques".to_string())?
}
#[tauri::command]
pub async fn get_saved_overnight_purchases(app:tauri::AppHandle,date:String)->Result<Option<SavedDailyPurchases>,String>{
 let today=snipers_core::calendar::date_at(massive_feed::live::now_ms()).ok_or("Fecha no disponible")?.to_string();
 if date>today{return Err("La fecha de bloques aún no ha ocurrido".into());}
 let from=snipers_core::calendar::legacy_eastern_ms(&format!("{date} 16:00:00")).ok_or("Hora de bloques inválida")?;
 let to=snipers_core::calendar::legacy_eastern_ms(&format!("{date} 20:00:00")).ok_or("Hora de bloques inválida")?;
 let next=next_overnight_session(&date)?;
 let morning_from=snipers_core::calendar::legacy_eastern_ms(&format!("{next} 04:00:00")).ok_or("Hora de bloques inválida")?;
 let morning_to=snipers_core::calendar::legacy_eastern_ms(&format!("{next} 09:30:00")).ok_or("Hora de bloques inválida")?;
 let dir=daily_purchases_dir(&app,&date)?;
 let morning_dir=daily_purchases_dir(&app,&next)?;
 tauri::async_runtime::spawn_blocking(move||{
  let mut post=load_overnight_purchases_dir(&dir,date.clone(),from,to)?;
  if next<=today {if let Some(morning)=load_overnight_purchases_dir(&morning_dir,next,morning_from,morning_to)?{
   if let Some(overnight)=post.as_mut(){overnight.minimum=overnight.minimum.min(morning.minimum);overnight.report.through_ms=overnight.report.through_ms.max(morning.report.through_ms);overnight.report.rows.extend(morning.report.rows);overnight.report.coverage.extend(morning.report.coverage);}
   else {post=Some(SavedDailyPurchases{minimum:morning.minimum,report:massive_feed::investment::DailyPurchases{date,through_ms:morning.report.through_ms,rows:morning.report.rows,coverage:morning.report.coverage}});}
  }}
  Ok(post)
 }).await.map_err(|_|"No se pudo recuperar el historial de bloques".to_string())?
}
fn next_overnight_session(date:&str)->Result<String,String>{
 let noon=snipers_core::calendar::legacy_eastern_ms(&format!("{date} 12:00:00")).ok_or("Fecha de bloques inválida")?;
 for offset in 1..=7 {let day=snipers_core::calendar::date_at(noon+offset*86_400_000).ok_or("Fecha fuera de rango")?;match snipers_core::calendar::schedule(day){snipers_core::calendar::Day::Open(_)=>return Ok(day.to_string()),snipers_core::calendar::Day::Closed=>continue,snipers_core::calendar::Day::Unknown=>return Err("Calendario de mercado no disponible".into())}}
 Err("Siguiente sesión no disponible".into())
}
#[tauri::command]
pub async fn get_overnight_closing_bar(symbol:String,date:String)->Result<massive_feed::overnight_data::ClosingBar,String>{
 massive_feed::overnight_data::closing_bar(&symbol,&date).await
}
#[tauri::command]
pub async fn get_overnight_qqq_holdings()->Result<massive_feed::qqq_components::Holdings,String>{
 massive_feed::qqq_components::fetch().await
}
fn overnight_records_dir(app:&tauri::AppHandle)->Result<std::path::PathBuf,String>{Ok(app.path().app_local_data_dir().map_err(|_|"Carpeta local no disponible")?.join("overnight-observations"))}
#[tauri::command]
pub async fn save_overnight_observation(app:tauri::AppHandle,record:serde_json::Value)->Result<(),String>{
 let date=record["date"].as_str().ok_or("Fecha de observación ausente")?;
 let symbol=record["symbol"].as_str().ok_or("Símbolo de observación ausente")?;
 if date.len()!=10||!date.bytes().enumerate().all(|(i,b)|if i==4||i==7{b==b'-'}else{b.is_ascii_digit()})||record["version"]!="overnight-v1"||symbol.is_empty()||symbol.len()>12||!symbol.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit()||b==b'.'||b==b'-'){return Err("Observación overnight inválida".into());}
 let today=snipers_core::calendar::date_at(massive_feed::live::now_ms()).ok_or("Fecha no disponible")?.to_string();if date>today.as_str(){return Err("No se guardan observaciones futuras".into());}
 let bytes=serde_json::to_vec(&record).map_err(|_|"No se pudo serializar la observación")?;if bytes.len()>32_000{return Err("Observación demasiado grande".into());}
 let dir=overnight_records_dir(&app)?;std::fs::create_dir_all(&dir).map_err(|_|"No se pudo crear el historial overnight")?;
 let path=dir.join(format!("{date}-{symbol}.json"));
 std::fs::write(&path,bytes).map_err(|_|"No se pudo guardar la observación".into())
}
#[tauri::command]
pub async fn get_overnight_observations(app:tauri::AppHandle)->Result<Vec<serde_json::Value>,String>{
 let dir=overnight_records_dir(&app)?;if !dir.is_dir(){return Ok(Vec::new())}
 tauri::async_runtime::spawn_blocking(move||{let mut files=std::fs::read_dir(&dir).map_err(|_|"No se pudo leer el historial")?.filter_map(Result::ok).map(|e|e.path()).filter(|p|p.extension().is_some_and(|x|x=="json")).collect::<Vec<_>>();files.sort();files.reverse();let mut out=Vec::new();for path in files.into_iter().take(500){let Ok(bytes)=std::fs::read(&path)else{continue};if bytes.len()>32_000{continue}if let Ok(value)=serde_json::from_slice::<serde_json::Value>(&bytes){if value["version"]=="overnight-v1"&&value["symbol"].as_str().is_some_and(|s|!s.is_empty()&&s.len()<=12&&s.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit()||b==b'.'||b==b'-')){out.push(value);}}}Ok(out)}).await.map_err(|_|"No se pudo leer el historial")?
}
fn load_daily_purchases_dir(dir:&std::path::Path,date:String,from:u64,to:u64)->Result<Option<SavedDailyPurchases>,String>{
 load_daily_purchases_dir_impl(dir,date,from,to,false)
}
fn load_overnight_purchases_dir(dir:&std::path::Path,date:String,from:u64,to:u64)->Result<Option<SavedDailyPurchases>,String>{
 load_daily_purchases_dir_impl(dir,date,from,to,true)
}
fn load_daily_purchases_dir_impl(dir:&std::path::Path,date:String,from:u64,to:u64,include_later_snapshots:bool)->Result<Option<SavedDailyPurchases>,String>{
 if !dir.is_dir(){return Ok(None)}
 let switch=snipers_core::calendar::legacy_eastern_ms(&format!("{date} 16:00:00")).ok_or("Hora de bloques inválida")?;
 let mut merged=massive_feed::investment::DailyPurchases{date,through_ms:0,rows:Vec::new(),coverage:Vec::new()};let mut minimum=f64::INFINITY;let mut legacy_rows=0;
 for entry in std::fs::read_dir(dir).map_err(|_|"No se pudieron leer los bloques guardados")? {
  let entry=entry.map_err(|_|"No se pudieron leer los bloques guardados")?;let path=entry.path();
  if path.extension().is_none_or(|ext|ext!="json")||!entry.metadata().is_ok_and(|m|m.is_file()&&m.len()<=64_000_000){continue;}
  if let Some(stem)=path.file_stem().and_then(|s|s.to_str()){if !stem.ends_with("-morning")&&!stem.ends_with("-afternoon")&&dir.join(format!("{stem}-{}.json",if from<switch{"morning"}else{"afternoon"})).is_file(){continue;}}
  let Ok(bytes)=std::fs::read(&path) else{continue};let Ok(saved)=serde_json::from_slice::<SavedDailyPurchases>(&bytes) else{continue};
  if saved.report.date!=merged.date||!saved.minimum.is_finite()||saved.minimum<100_000. {continue;}
  if saved.report.through_ms<from||(!include_later_snapshots&&to==switch&&saved.report.through_ms>=switch){continue;}
  merged.through_ms=merged.through_ms.max(saved.report.through_ms);minimum=minimum.min(saved.minimum);
  legacy_rows+=saved.report.rows.iter().filter(|row|row.block.execution_ms.is_none_or(|ms|ms<from||ms>=to)).count();
  merged.rows.extend(saved.report.rows.into_iter().filter(|row|row.block.execution_ms.is_some_and(|ms|ms>=from&&ms<to)));merged.coverage.extend(saved.report.coverage);
 }
 if !minimum.is_finite()||(include_later_snapshots&&merged.rows.is_empty()){return Ok(None)}
 if legacy_rows>0{merged.coverage.push(format!("Caché anterior: {legacy_rows} operaciones fuera de la ventana seleccionada excluidas; sus contadores históricos pueden incluirlas."));}
 merged.rows.sort_by(|a,b|b.block.notional.total_cmp(&a.block.notional));
 Ok(Some(SavedDailyPurchases{minimum,report:merged}))
}
#[tauri::command]
pub async fn search_daily_purchases(app:tauri::AppHandle,task:tauri::State<'_,Task>,symbols:String,minimum:f64)->Result<massive_feed::investment::DailyPurchases,String>{
    if task.busy.swap(true,Ordering::SeqCst){return Err("Espera a que termine la consulta actual".into());}
    struct Release(Arc<AtomicBool>);impl Drop for Release{fn drop(&mut self){self.0.store(false,Ordering::SeqCst);}}
    let _release=Release(task.busy.clone());let mut result=massive_feed::investment::daily_purchases(&symbols,minimum).await?;
    if let Err(error)=save_daily_purchases(&app,&symbols.trim().to_ascii_uppercase(),minimum,result.clone()){result.coverage.push(format!("Guardado local: {error}"));}
    Ok(result)
}

#[tauri::command]
pub async fn get_daily_purchases_universe()->Result<Vec<String>,String>{massive_feed::investment::daily_purchases_universe().await}
#[cfg(test)]
mod daily_purchases_cache_tests {
 use super::*;
 #[test]
 fn saves_each_completed_symbol_and_loads_only_the_selected_day(){
  let root=tempfile::tempdir().unwrap();let current=root.path().join("2026-10-07");
  let afternoon=snipers_core::calendar::legacy_eastern_ms("2026-10-07 17:00:00").unwrap();
  let (from,to)=purchases_window("2026-10-07",afternoon,true).unwrap();
  for (ticker,count) in [("SMH",1),("AAPL",2)]{
   let report=massive_feed::investment::DailyPurchases{date:"2026-10-07".into(),through_ms:afternoon+count,rows:Vec::new(),coverage:vec![format!("{ticker}: {count} bloques")]};
   save_daily_purchases_file(&current,ticker,10_000_000.,report).unwrap();
  }
  let old=root.path().join("2026-10-06");
  let prior=snipers_core::calendar::legacy_eastern_ms("2026-10-06 17:00:00").unwrap();
  save_daily_purchases_file(&old,"SMH",5_000_000.,massive_feed::investment::DailyPurchases{date:"2026-10-06".into(),through_ms:prior,rows:Vec::new(),coverage:vec!["SMH: día anterior".into()]}).unwrap();
  let today=load_daily_purchases_dir(&current,"2026-10-07".into(),from,to).unwrap().unwrap();
  assert_eq!(today.report.coverage.len(),2);assert_eq!(today.report.through_ms,afternoon+2);
  let (prior_from,prior_to)=purchases_window("2026-10-06",prior,true).unwrap();
  let yesterday=load_daily_purchases_dir(&old,"2026-10-06".into(),prior_from,prior_to).unwrap().unwrap();
  assert_eq!(yesterday.report.coverage,vec!["SMH: día anterior"]);
  assert!(load_daily_purchases_dir(&root.path().join("2026-10-05"),"2026-10-05".into(),from,to).unwrap().is_none());
 }
 #[test]
 fn morning_cache_expires_at_four_and_afternoon_cache_is_available_next_day(){
  let root=tempfile::tempdir().unwrap();let dir=root.path().join("2026-10-07");
  let morning=snipers_core::calendar::legacy_eastern_ms("2026-10-07 10:00:00").unwrap();
  let afternoon=snipers_core::calendar::legacy_eastern_ms("2026-10-07 17:00:00").unwrap();
  let report=|through_ms,source:&str|massive_feed::investment::DailyPurchases{date:"2026-10-07".into(),through_ms,rows:Vec::new(),coverage:vec![source.into()]};
  save_daily_purchases_file(&dir,"GOOGL",10_000_000.,report(morning,"mañana")).unwrap();
  let (morning_from,morning_to)=purchases_window("2026-10-07",morning,true).unwrap();
  let (afternoon_from,afternoon_to)=purchases_window("2026-10-07",afternoon,true).unwrap();
  assert_eq!(load_daily_purchases_dir(&dir,"2026-10-07".into(),morning_from,morning_to).unwrap().unwrap().report.coverage,vec!["mañana"]);
  assert!(load_daily_purchases_dir(&dir,"2026-10-07".into(),afternoon_from,afternoon_to).unwrap().is_none());
  save_daily_purchases_file(&dir,"GOOGL",10_000_000.,report(afternoon,"tarde")).unwrap();
  assert_eq!(load_daily_purchases_dir(&dir,"2026-10-07".into(),morning_from,morning_to).unwrap().unwrap().report.coverage,vec!["mañana"]);
  assert_eq!(load_daily_purchases_dir(&dir,"2026-10-07".into(),afternoon_from,afternoon_to).unwrap().unwrap().report.coverage,vec!["tarde"]);
 }
 #[test]
 fn overnight_reads_postclose_trades_from_a_legacy_after_close_snapshot(){
  let root=tempfile::tempdir().unwrap();let dir=root.path().join("2026-10-07");std::fs::create_dir_all(&dir).unwrap();
  let rth=snipers_core::calendar::legacy_eastern_ms("2026-10-07 15:15:00").unwrap();
  let after=snipers_core::calendar::legacy_eastern_ms("2026-10-07 17:15:00").unwrap();
  let row=|execution_ms|massive_feed::investment::DailyPurchase{ticker:"AMD".into(),block:snipers_core::investment_structure::Block{id:"test".into(),price:100.,shares:1000.,notional:100_000.,execution_ms:Some(execution_ms),sip_ms:execution_ms,trf_ms:None,trf_id:1,conditions:vec![0],nbbo:"En ask".into(),bid:Some(99.99),ask:Some(100.),quote_ms:Some(execution_ms-100)},execution:serde_json::Value::Null};
  let saved=SavedDailyPurchases{minimum:100_000.,report:massive_feed::investment::DailyPurchases{date:"2026-10-07".into(),through_ms:after,rows:vec![row(rth),row(after)],coverage:vec![]}};
  std::fs::write(dir.join("AMD.json"),serde_json::to_vec(&saved).unwrap()).unwrap();
  let from=snipers_core::calendar::legacy_eastern_ms("2026-10-07 16:00:00").unwrap();let to=snipers_core::calendar::legacy_eastern_ms("2026-10-07 20:00:00").unwrap();
  let result=load_overnight_purchases_dir(&dir,"2026-10-07".into(),from,to).unwrap().unwrap();
  assert_eq!(result.report.rows.len(),1);assert_eq!(result.report.rows[0].block.execution_ms,Some(after));
 }
 #[test]
 fn overnight_next_session_skips_weekends_and_market_holidays(){
  assert_eq!(next_overnight_session("2026-10-08").unwrap(),"2026-10-09");
  assert_eq!(next_overnight_session("2026-10-09").unwrap(),"2026-10-12");
  assert_eq!(next_overnight_session("2026-11-25").unwrap(),"2026-11-27");
 }
}

#[tauri::command]
pub async fn get_benzinga_news(app:tauri::AppHandle,ticker:Option<String>,force:Option<bool>)->Result<massive_feed::investment::NewsReport,String>{
 use crate::news_coordinator::{NewsCoordinator,batch,merge};
 type Shared=Arc<tokio::sync::Mutex<NewsCoordinator>>;
 static CACHE:std::sync::OnceLock<Shared>=std::sync::OnceLock::new();
 static FETCH:std::sync::OnceLock<tokio::sync::Mutex<std::collections::BTreeMap<String,Arc<tokio::sync::Mutex<()>>>>>=std::sync::OnceLock::new();
 let symbol=ticker.unwrap_or_default().trim().to_ascii_uppercase();
 if symbol.len()>16||!symbol.chars().all(|c|c.is_ascii_uppercase()||c=='.'||c=='-'){return Err("Símbolo de noticias inválido".into());}
 let cache=CACHE.get_or_init(||Arc::new(tokio::sync::Mutex::new(NewsCoordinator::default()))).clone();
 let gate={let mut gates=FETCH.get_or_init(||tokio::sync::Mutex::new(Default::default())).lock().await;gates.entry(symbol.clone()).or_insert_with(||Arc::new(tokio::sync::Mutex::new(()))).clone()};
 let _fetch_guard=gate.lock().await;
 let now=massive_feed::live::now_ms();
 let cached={let c=cache.lock().await;c.reports.get(&symbol).filter(|r|force!=Some(true)&&r.fetched_ms<=now&&now-r.fetched_ms<60000).cloned()};
 let directory=app.path().app_data_dir().map_err(|_|"Directorio local no disponible")?;
 std::fs::create_dir_all(&directory).map_err(|_|"Directorio de noticias no disponible")?;
 let mut report=if let Some(r)=cached{r}else{
  let mut r=massive_feed::investment::benzinga_intraday_news(&symbol).await?;
  let existing={cache.lock().await.reports.values().flat_map(|r|r.items.clone()).collect::<Vec<_>>()};
  for item in &mut r.items{if let Some(old)=existing.iter().find(|n|n["translated"]==true&&n["id"]==item["id"]&&n["originalTitle"]==item["title"]&&n["originalSummary"]==item["summary"]&&n["providerUpdatedAt"]==item["providerUpdatedAt"]){*item=old.clone();}}
  r
 };
 let ledger=directory.join("news-evidence-v2.sqlite");let record_path=ledger.clone();
 report=tauri::async_runtime::spawn_blocking(move||{let mut r=report;if snipers_store::news::record(&record_path,&mut r.items,now).is_err(){for n in &mut r.items{n["persistenceState"]=serde_json::json!("PENDING_STORAGE_ERROR");}}r}).await.map_err(|_|"Registro noticias no disponible")?;
 let launch={let mut c=cache.lock().await;if c.reports.len()>=20&&!c.reports.contains_key(&symbol){let first=c.reports.keys().find(|s|!s.is_empty()).cloned();if let Some(first)=first{c.reports.remove(&first);}}c.reports.insert(symbol,report.clone());if !c.running&&!batch(&c).is_empty(){c.running=true;true}else{false}};
 if launch{tauri::async_runtime::spawn(async move{
  let path=directory.join("benzinga-spanish-local-v2.json");
  loop{
   let items={let mut c=cache.lock().await;let items=batch(&c);if items.is_empty(){c.running=false;return;}items};
   let mut translated=massive_feed::investment::NewsReport{items,fetched_ms:massive_feed::live::now_ms(),truncated:false};
   if massive_feed::news_translation::translate_news(&mut translated,&path).await.is_err(){let mut c=cache.lock().await;for n in c.reports.values_mut().flat_map(|r|r.items.iter_mut()).filter(|n|n["translated"]!=true){n["processingState"]=serde_json::json!("TRANSLATION_RETRY_PENDING");}c.running=false;return;}
   for n in &mut translated.items{n["processingState"]=serde_json::json!("ENRICHED");}
   let record_path=ledger.clone();translated=match tauri::async_runtime::spawn_blocking(move||{let mut r=translated;if snipers_store::news::record(&record_path,&mut r.items,massive_feed::live::now_ms()).is_err(){for n in &mut r.items{n["persistenceState"]=serde_json::json!("PENDING_STORAGE_ERROR");}}r}).await{Ok(r)=>r,Err(_)=>{cache.lock().await.running=false;return;}};
   merge(&mut *cache.lock().await,&translated.items);
  }
 });}
 Ok(report)
}


#[tauri::command]
pub async fn record_news_alerts(app:tauri::AppHandle,items:Vec<serde_json::Value>)->Result<(),String>{
 let path=app.path().app_data_dir().map_err(|_|"Directorio local no disponible")?.join("news-evidence-v2.sqlite");
 tauri::async_runtime::spawn_blocking(move||snipers_store::news::alerts(&path,&items,massive_feed::live::now_ms()).map_err(|_|"No se pudo guardar el aviso de noticias".to_owned())).await.map_err(|_|"Registro de avisos no disponible".to_owned())?
}
