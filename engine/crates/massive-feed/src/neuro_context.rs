//! Supplemental model evidence, never a score bonus or execution authorization.
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all="camelCase")]
pub struct Context {
    pub symbol:String, pub at:u64, pub spot:Option<f64>,
    pub realtime:bool, pub regular:bool, pub net_gamma:Option<f64>,
    pub charm_flow:Option<f64>, pub gamma_magnet:Option<f64>, pub gamma_flip:Option<f64>,
    pub contracts:usize, pub used:usize, pub available:bool, pub reason:String,
    #[serde(default)]
    pub model_payload:Option<serde_json::Value>,
}
impl Context {
    /// Display uses the same bounded provider-clock tolerance as the stock panel.
    /// Strategy validation continues to use `checked` with the original timestamp.
    pub fn checked_display(&self, symbol:&str, now:u64, price:Option<(f64,u64)>, connected:bool)->Self {
        let display_price=price.map(|(p,t)| (p, if t>now && t<=now.saturating_add(1000) {now} else {t}));
        let mut c=self.checked(symbol,now,display_price,connected);
        if c.reason=="Neuro pendiente de precio coincidente y vigente" {
            c.reason=match price {
                Some((_,t)) if t>now.saturating_add(1000)=>"Precio de Snipers adelantado más de 1 segundo".into(),
                Some((_,t)) if now.saturating_sub(t)>15_000=>"Precio de Snipers sin actualización en los últimos 15 segundos".into(),
                Some((p,_))=>format!("Precio Neuro {:?} frente a Snipers {:.4}; tolerancia de coincidencia 0,5 %",self.spot,p),
                None=>"Falta el último trade de Snipers para comparar con Neuro".into(),
            };
        }
        c
    }
    pub fn narrative(&self, side:&str)->String {
        if !self.available {return self.reason.clone();}
        let gamma=match self.net_gamma {Some(v) if v>0.0=>"Gamma del modelo positiva: posible amortiguación",Some(v) if v<0.0=>"Gamma del modelo negativa: posible amplificación",_=>"Gamma del modelo sin predominio"};
        let charm=match self.charm_flow {Some(v) if v>0.0=>"charm estima presión compradora",Some(v) if v<0.0=>"charm estima presión vendedora",_=>"charm sin dirección"};
        let conflict=matches!((side,self.charm_flow),("CALL",Some(v)) if v<0.0)||matches!((side,self.charm_flow),("PUT",Some(v)) if v>0.0);
        format!("{gamma}; {charm}. {}. Contexto de siete días; confirma con precio y flujo, no es una señal de entrada.",if conflict {"Diverge de la dirección de Snipers"}else{"No constituye confirmación independiente"})
    }
    pub fn checked(&self, symbol:&str, now:u64, price:Option<(f64,u64)>, connected:bool)->Self {
        let mut c=self.clone();
        c.available=false;
        c.reason=if c.symbol!=symbol {"Neuro pendiente para este activo"}
        else if c.at>now || now.saturating_sub(c.at)>45_000 {"Contexto Neuro antiguo; no aplicado"}
        else if !connected || !c.realtime || !c.regular {"Neuro sin evidencia de sesión regular en tiempo real"}
        else if c.used==0 || c.contracts==0 || c.used>c.contracts || c.net_gamma.is_none_or(|v|!v.is_finite()) || c.charm_flow.is_none_or(|v|!v.is_finite()) {"Perfil Neuro incompleto"}
        else if !price.is_some_and(|(p,t)| p.is_finite() && p>0.0 && t<=now && now-t<=15_000 && c.spot.is_some_and(|s|s.is_finite()&&s>0.0&&(s-p).abs()/p<=0.005)) {"Neuro pendiente de precio coincidente y vigente"}
        else {c.available=true; "Modelo Neuro de siete días, 0DTE incluido, convención +CALL/−PUT. Proyección, no flujo observado; OI sin fecha certificada. No modifica puntuaciones ni autoriza entradas"}.into();
        if !c.available {c.net_gamma=None;c.charm_flow=None;c.gamma_magnet=None;c.gamma_flip=None;c.model_payload=None;}
        c
    }
}

/// Runs with the market task, independent of the visible page; dropping it cancels polling.
pub async fn manual_profiles(symbols:Vec<String>)->Result<std::collections::BTreeMap<String,Context>, &'static str> {
    manual_profiles_horizon(symbols,7).await
}
pub async fn manual_profiles_horizon(symbols:Vec<String>,max_dte:u32)->Result<std::collections::BTreeMap<String,Context>, &'static str> {
    if !matches!(max_dte,0|7|28){return Err("Horizonte no admitido");}
    if symbols.len()>5 || symbols.iter().any(|s|snipers_core::universe::stock_asset(s).is_none()) {return Err("Selecciona hasta cinco activos válidos");}
    let key=crate::credentials::Credentials::load()?;
    let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(35)).build().map_err(|_|"Cliente no disponible")?;
    let results=futures_util::future::join_all(symbols.into_iter().map(|symbol| {
        let client=&client;let key=&key;
        async move {
            let mut c=match client.get(format!("http://127.0.0.1:8768/snipers-context/{symbol}")).query(&[("max_dte",max_dte)]).send().await {
                Ok(response)=>response.json::<Context>().await.unwrap_or_default(),
                Err(_)=>Context::default(),
            };
            let trade=async {
                let response=client.get(format!("https://api.massive.com/v2/last/trade/{symbol}")).query(&[("apiKey",key.0.as_str())]).send().await.ok()?.error_for_status().ok()?;
                let value=response.json::<serde_json::Value>().await.ok()?;
                if value["results"]["T"]!=symbol {return None;}
                Some((value["results"]["p"].as_f64()?,value["results"]["t"].as_u64()?/1_000_000))
            }.await;
            c=c.checked_display(&symbol,crate::live::now_ms(),trade,true);
            (symbol,c)
        }
    })).await;
    Ok(results.into_iter().collect())
}
pub async fn run(state:crate::live::SharedState) {
    let Ok(client)=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(35)).build() else {return};
    loop {
        let symbols={let mut s=state.lock().unwrap();let mut v=vec![s.symbol.clone(),"QQQ".into()];
            let connected=s.stocks.phase=="streaming";
            let report=s.scanner_store.snapshot(crate::live::now_ms(),connected);
            v.extend(report.bot_selection.symbols.clone());
            for period in ["premarket","opening","volume10","volume30"] {
                let mut rows=report.volume_windows.iter().filter(|r|r.period==period&&r.volume.is_finite()&&r.volume>0.).collect::<Vec<_>>();
                rows.sort_by(|a,b|b.volume.total_cmp(&a.volume));
                v.extend(rows.into_iter().take(5).map(|r|r.symbol.clone()));
            }
            v.extend(s.bot_held.clone());v.sort();v.dedup();v};
        let results=futures_util::future::join_all(symbols.iter().map(|symbol| {
            let client=&client;
            async move {
                let result=async {
                    if snipers_core::universe::stock_asset(symbol).is_none(){return None;}
                    client.get(format!("http://127.0.0.1:8768/snipers-context/{symbol}")).send().await.ok()?.error_for_status().ok()?.json::<Context>().await.ok()
                }.await;
                (symbol.clone(),result)
            }
        })).await;
        {let mut s=state.lock().unwrap();s.neuro_contexts.retain(|key,_|symbols.contains(key));
         for (symbol,result) in results {if let Some(c)=result.filter(|c|c.symbol==symbol){s.neuro_contexts.insert(symbol,c);}else{s.neuro_contexts.remove(&symbol);}}}
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}
#[cfg(test)] mod tests {
 use super::*;
 fn sample()->Context {Context{symbol:"QQQ".into(),at:100000,spot:Some(500.),realtime:true,regular:true,net_gamma:Some(10.),charm_flow:Some(-3.),contracts:100,used:80,..Default::default()}}
 #[test] fn validates_identity_age_price_session_and_coverage(){
  let c=sample(); let check=|c:&Context,s,n,p|c.checked(s,n,p,true).available;
  assert!(check(&c,"QQQ",100000,Some((500.,100000))));
  assert!(!check(&c,"SPY",100000,Some((500.,100000))));
  assert!(!check(&c,"QQQ",99999,Some((500.,99999))));
  assert!(!check(&c,"QQQ",145001,Some((500.,145001))));
  assert!(!check(&c,"QQQ",100000,Some((510.,100000))));
  assert!(!check(&c,"QQQ",100000,None));
  for bad in [Context{regular:false,..c.clone()},Context{realtime:false,..c.clone()},Context{used:101,..c.clone()},Context{net_gamma:Some(f64::NAN),..c.clone()}] {assert!(!check(&bad,"QQQ",100000,Some((500.,100000))));}
  assert!(c.checked("SPY",100000,None,true).net_gamma.is_none());
 }
 #[test] fn display_clock_tolerance_does_not_relax_strategy_validation(){
  let c=sample();
  assert!(c.checked_display("QQQ",100000,Some((500.,100650)),true).available);
  assert!(!c.checked("QQQ",100000,Some((500.,100650)),true).available);
  assert!(!c.checked_display("QQQ",100000,Some((500.,101001)),true).available);
  assert!(!c.checked_display("QQQ",100000,Some((510.,100000)),true).available);
 }
}
