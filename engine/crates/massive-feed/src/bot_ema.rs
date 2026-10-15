//! Closed one-minute RTH EMAs for paper-bot context. Never use an unfinished bar.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snipers_core::calendar::{self, Day};
use std::collections::BTreeMap;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmaRead {
    pub available: bool,
    pub timeframe_minutes: u8,
    pub last_closed_ms: u64,
    pub bars: usize,
    pub ema9: Option<f64>,
    pub ema20: Option<f64>,
    pub ema50: Option<f64>,
    pub ema200: Option<f64>,
}

fn ema(values: &[f64], period: usize) -> Option<f64> {
    if values.len() < period { return None; }
    let mut value = values[..period].iter().sum::<f64>() / period as f64;
    let alpha = 2.0 / (period as f64 + 1.0);
    for close in &values[period..] { value += alpha * (*close - value); }
    value.is_finite().then_some(value)
}

pub fn read(history: &Value, now: u64) -> EmaRead {
    let mut result = EmaRead { timeframe_minutes: 1, ..Default::default() };
    let mut rows = BTreeMap::new();
    for bar in history["minutes"].as_array().into_iter().flatten() {
        let Some(t) = bar["timestampMs"].as_u64() else { continue };
        if t.checked_add(60_000).is_none_or(|end| end > now) { continue; }
        let Some(date) = calendar::date_at(t) else { continue };
        let Day::Open(session) = calendar::schedule(date) else { continue };
        if t < session.open_ms || t >= session.close_ms || (t-session.open_ms)%60_000 != 0 { continue; }
        let (Some(o),Some(h),Some(l),Some(c)) = (bar["open"].as_f64(),bar["high"].as_f64(),bar["low"].as_f64(),bar["close"].as_f64()) else {continue};
        if ![o,h,l,c].iter().all(|x|x.is_finite()&&*x>0.) || l>o.min(c) || h<o.max(c) {continue;}
        rows.insert(t,c);
    }
    let Some((&last,_))=rows.last_key_value() else {return result};
    result.last_closed_ms=last+60_000;
    let Some(today)=calendar::date_at(now) else {return result};
    if calendar::date_at(last)!=Some(today) || now.saturating_sub(result.last_closed_ms)>180_000 {return result;}
    // A missing minute within a session invalidates the EMA seed and current reading.
    let recent=rows.iter().rev().take(200).map(|(&t,&c)|(t,c)).collect::<Vec<_>>();
    if recent.len()<200 {return result;}
    for pair in recent.windows(2) {
        let newer=pair[0].0;let older=pair[1].0;
        if calendar::date_at(newer)==calendar::date_at(older) && newer-older!=60_000 {return result;}
    }
    let closes=rows.values().copied().collect::<Vec<_>>();
    result.bars=closes.len();
    result.ema9=ema(&closes,9);
    result.ema20=ema(&closes,20);
    result.ema50=ema(&closes,50);
    result.ema200=ema(&closes,200);
    result.available=[result.ema9,result.ema20,result.ema50,result.ema200].iter().all(Option::is_some);
    result
}

/// Only a strong short/medium-term contradiction blocks a new entry.
pub fn opposes(read: &EmaRead, side: &str, spot: f64) -> bool {
    if !read.available || !spot.is_finite() {return false;}
    let (Some(e9),Some(e20),Some(e50))=(read.ema9,read.ema20,read.ema50) else {return false};
    match side {
        "CALL" => e9<e20 && e20<e50 && spot<e20,
        "PUT" => e9>e20 && e20>e50 && spot>e20,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn closed_bars_seed_all_four_and_ignore_live_bar() {
        let open=calendar::rfc3339_ms("2026-10-07T09:30:00-04:00").unwrap();
        let minutes=(0..201).map(|i|json!({"timestampMs":open+i*60_000,"open":100.,"high":101.,"low":99.,"close":100.})).collect::<Vec<_>>();
        let r=read(&json!({"minutes":minutes}),open+201*60_000);
        assert!(r.available);assert_eq!(r.bars,201);assert_eq!(r.ema200,Some(100.));
        assert_eq!(r.last_closed_ms,open+201*60_000);
    }
    #[test]
    fn missing_recent_minute_or_stale_reading_cannot_authorize_ema() {
        let open=calendar::rfc3339_ms("2026-10-07T09:30:00-04:00").unwrap();
        let minutes=(0..202).filter(|i|*i!=150).map(|i|json!({"timestampMs":open+i*60_000,"open":100.,"high":101.,"low":99.,"close":100.})).collect::<Vec<_>>();
        assert!(!read(&json!({"minutes":minutes}),open+202*60_000).available);
        let stale=(0..200).map(|i|json!({"timestampMs":open+i*60_000,"open":100.,"high":101.,"low":99.,"close":100.})).collect::<Vec<_>>();
        assert!(!read(&json!({"minutes":stale}),open+210*60_000).available);
    }
    #[test]
    fn opposition_is_side_specific_and_does_not_infer_from_absence() {
        let r=EmaRead{available:true,ema9:Some(98.),ema20:Some(99.),ema50:Some(100.),ema200:Some(101.),..Default::default()};
        assert!(opposes(&r,"CALL",97.));assert!(!opposes(&r,"PUT",97.));
        assert!(!opposes(&EmaRead::default(),"CALL",97.));
    }
}
