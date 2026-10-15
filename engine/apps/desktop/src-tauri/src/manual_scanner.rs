use massive_feed::{
    credentials::Credentials,
    manual_scanner::{self, Report},
};
use std::sync::atomic::{AtomicBool, Ordering};
fn valid_native_request(symbols:&[String],days:u32)->bool{
    !symbols.is_empty()&&symbols.len()<=20&&matches!(days,0|7|28)&&symbols.iter().all(|s|snipers_core::universe::stock_asset(s).is_some())
}
#[tauri::command]
pub async fn get_manual_scanner_profiles(app:tauri::AppHandle,task:tauri::State<'_,crate::LiveTask>,symbols:Vec<String>,max_dte:Option<u32>)->Result<NativeProfiles,String>{
    use futures_util::{stream,StreamExt};
    use tauri::Manager;
    let days=max_dte.unwrap_or(7);
    if !valid_native_request(&symbols,days) {
        return Err("Selecciona hasta veinte candidatos válidos y un horizonte admitido".into());
    }
    static BUSY:AtomicBool=AtomicBool::new(false);
    if BUSY.swap(true,Ordering::SeqCst){return Err("Consulta de opciones en curso".into());}
    struct Guard;
    impl Drop for Guard{fn drop(&mut self){BUSY.store(false,Ordering::SeqCst);}}
    let _guard=Guard;
    let live=task.state.lock().map_err(|_|"Sesión no disponible")?.clone();
    let directory=app.path().app_local_data_dir().ok().map(|p|p.join("top-five-oi-observations").join(days.to_string()));
    let symbols:std::collections::BTreeSet<_>=symbols.into_iter().collect();
    let mut pending=stream::iter(symbols.into_iter().map(|symbol|{
        let directory=directory.clone();
        async move{let result=tokio::time::timeout(std::time::Duration::from_secs(25),massive_feed::etf_scanner::fetch_stock(&symbol,days,directory)).await.unwrap_or(Err("Tiempo de consulta agotado"));(symbol,result)}
    })).buffer_unordered(2);
    let deadline=tokio::time::Instant::now()+std::time::Duration::from_secs(120);
    let mut out=NativeProfiles::default();
    loop{match tokio::time::timeout_at(deadline,pending.next()).await{
        Ok(Some((symbol,Ok(mut report))))=>{
            let now=massive_feed::live::now_ms();
            let minutes=live.as_ref().and_then(|state|state.lock().ok()?.scanner_store.bot_open(&symbol,now)).map(|bar|serde_json::json!([bar])).unwrap_or(serde_json::json!([]));
            report.amplitude=Some(massive_feed::movement_room::calculate(&symbol,&serde_json::json!(report.daily_range),Some(report.at),report.spot,report.price_at,&minutes,now));
            out.reports.insert(symbol,report);
        },
        Ok(Some((symbol,Err(error))))=>{out.errors.insert(symbol,error.into());},
        Ok(None)=>break,
        Err(_)=>{out.errors.insert("consulta".into(),"Consulta parcial: límite de dos minutos".into());break;}
    }}
    Ok(out)
}
#[derive(Default,serde::Serialize)]
pub struct NativeProfiles{
    reports:std::collections::BTreeMap<String,massive_feed::etf_scanner::Report>,
    errors:std::collections::BTreeMap<String,String>,
}
#[cfg(test)]
mod native_tests{
    use super::*;
    #[test]
    fn limits_native_option_requests_to_configured_candidates(){
        assert!(valid_native_request(&["AAPL".into(),"QQQ".into()],7));
        assert!(!valid_native_request(&[],7));
        assert!(!valid_native_request(&vec!["AAPL".into();21],7));
        assert!(!valid_native_request(&["AAPL".into()],8));
        assert!(!valid_native_request(&["INVALID-ASSET".into()],7));
    }
}

#[tauri::command]
pub async fn search_top_five_now(
    on_message: tauri::ipc::Channel<Report>,
) -> Result<Report, String> {
    static BUSY: AtomicBool = AtomicBool::new(false);
    if BUSY.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una búsqueda manual en curso".into());
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::SeqCst);
        }
    }
    let _guard = Guard;
    let key = Credentials::load().map_err(str::to_owned)?;
    manual_scanner::run(&key, |r| on_message.send(r.clone()).is_ok())
        .await
        .map_err(str::to_owned)
}
