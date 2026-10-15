//! Shared SMA true range; magnitude only, never directional evidence.
pub fn sma14(bars: &[(f64, f64, f64)]) -> Option<f64> {
    let bars = bars.get(bars.len().checked_sub(15)?..)?;
    if bars.iter().any(|&(h,l,c)| ![h,l,c].iter().all(|v|v.is_finite()) || l<=0. || h<c || c<l) {return None;}
    let value=bars.windows(2).map(|w|(w[1].0-w[1].1).max((w[1].0-w[0].2).abs()).max((w[1].1-w[0].2).abs())).sum::<f64>()/14.;
    (value.is_finite() && value>0.).then_some(value)
}
/// Exact last fifteen completed exchange sessions, including holidays/early closes.
pub fn sessions(now:u64)->Option<Vec<crate::calendar::Session>> {
    let mut date=crate::calendar::date_at(now)?;
    let mut out=Vec::new();
    for _ in 0..60 {
        match crate::calendar::schedule(date) {
            crate::calendar::Day::Open(s) if s.close_ms<now => out.push(s),
            crate::calendar::Day::Unknown => return None,
            _=>{}
        }
        if out.len()==15 {out.reverse();return Some(out);}
        date=date.pred_opt()?;
    }
    None
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn true_range_and_validation(){let mut b=vec![(101.,99.,100.);15];assert_eq!(sma14(&b),Some(2.));b[14]=(111.,109.,110.);assert_eq!(sma14(&b),Some(37./14.));assert_eq!(sma14(&b[..14]),None);b[14].0=f64::NAN;assert_eq!(sma14(&b),None);}
    #[test] fn calendar_excludes_forming_and_weekend(){let now=crate::calendar::rfc3339_ms("2026-10-03T12:00:00-04:00").unwrap();assert_eq!(sessions(now).unwrap().last().unwrap().date,"2026-10-02");let pre=crate::calendar::rfc3339_ms("2026-10-02T15:00:00-04:00").unwrap();assert_eq!(sessions(pre).unwrap().last().unwrap().date,"2026-10-01");}
    #[test] fn holiday_and_early_close(){let now=crate::calendar::rfc3339_ms("2026-11-27T13:01:00-05:00").unwrap();let s=sessions(now).unwrap();assert!(s.last().unwrap().early_close);assert!(!s.iter().any(|v|v.date=="2026-11-26"));}
    #[test] fn consistent_split_adjustment_scales_atr(){let b=vec![(101.,99.,100.);15];let adjusted=b.iter().map(|&(h,l,c)|(h/10.,l/10.,c/10.)).collect::<Vec<_>>();assert!((sma14(&adjusted).unwrap()-sma14(&b).unwrap()/10.).abs()<1e-12);}
}
