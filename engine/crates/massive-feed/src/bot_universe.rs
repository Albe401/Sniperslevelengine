//! Frozen premarket universe for background Paper analysis, independent of UI tabs.
use serde::{Serialize,Deserialize};
use crate::scanner::VolumeWindow;
#[derive(Clone,Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Selection { pub session:String, pub symbols:Vec<String>, pub verified_ms:Option<u64>, pub detail:String }
impl Selection {
    pub fn restore(&mut self,saved:Self,now:u64)->bool{
        let Some(date)=snipers_core::calendar::date_at(now) else{return false};
        let snipers_core::calendar::Day::Open(day)=snipers_core::calendar::schedule(date) else{return false};
        let unique:std::collections::BTreeSet<_>=saved.symbols.iter().collect();
        if saved.session!=date.to_string()||saved.symbols.len()!=5||unique.len()!=5||saved.symbols.iter().any(|s|snipers_core::universe::stock_asset(s).is_none())||saved.verified_ms.is_none_or(|t|t<day.open_ms||t>now){return false;}
        *self=saved;true
    }
    pub fn update(&mut self, rows:&[VolumeWindow], now:u64, recovered:bool) {
        use snipers_core::calendar::{self,Day};
        let Some(date)=calendar::date_at(now) else {return};
        if self.session!=date.to_string(){*self=Self{session:date.to_string(),..Default::default()};}
        let Day::Open(day)=calendar::schedule(date) else {self.detail="Sesión cerrada".into();return};
        if self.verified_ms.is_some(){return;}
        if now<day.open_ms {self.detail="Verificación a las 09:30 ET · acumulando premarket".into();return;}
        if !recovered {self.detail="Esperando recuperación completa del universo".into();return;}
        let mut candidates:Vec<_>=rows.iter().filter(|r|r.period=="premarket"&&r.finalized&&r.end_ms==day.open_ms&&r.volume.is_finite()&&r.volume>0.&&snipers_core::universe::stock_asset(&r.symbol).is_some()).collect();
        candidates.sort_by(|a,b|b.volume.total_cmp(&a.volume).then(a.symbol.cmp(&b.symbol)));
        let mut seen=std::collections::BTreeSet::new();
        self.symbols=candidates.into_iter().filter(|r|seen.insert(r.symbol.clone())).take(5).map(|r|r.symbol.clone()).collect();
        if self.symbols.len()==5 {self.verified_ms=Some(now);self.detail="Cinco activos fijados por volumen premarket hasta 09:30 ET".into();}
        else {self.symbols.clear();self.detail="Faltan cinco candidatos con volumen premarket verificado".into();}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn freezes_five_only_after_open_and_full_recovery_then_resets_next_day(){
        let open=snipers_core::calendar::legacy_eastern_ms("2026-09-29 09:30:00").unwrap();
        let mut rows:Vec<_>=["QQQ","SPY","AAPL","NVDA","TSLA","BBY"].into_iter().enumerate().map(|(i,s)|VolumeWindow{symbol:s.into(),period:"premarket".into(),end_ms:open,finalized:true,volume:1000.-i as f64,..Default::default()}).collect();
        let mut selection=Selection::default();selection.update(&rows,open-1,true);assert!(selection.symbols.is_empty());
        selection.update(&rows,open,false);assert!(selection.symbols.is_empty());
        selection.update(&rows,open,true);assert_eq!(selection.symbols,["QQQ","SPY","AAPL","NVDA","TSLA"]);
        rows[5].volume=99999.;selection.update(&rows,open+60000,true);assert!(!selection.symbols.contains(&"BBY".into()));
        let saved=selection.clone();let mut restored=Selection::default();assert!(restored.restore(saved.clone(),open+60000));assert_eq!(restored.symbols,selection.symbols);
        assert!(!restored.restore(saved,open-1));
        selection.update(&rows,open+86400000,true);assert!(selection.symbols.is_empty());
    }
}
