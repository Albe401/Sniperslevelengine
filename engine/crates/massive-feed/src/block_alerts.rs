//! Adapter from the existing bounded block search to the alert decision engine.
//! No polling or notification is started until the Radar UI is installed.
use crate::investment::DailyPurchases;
use snipers_core::block_alerts::{Alert, Config, Engine};

pub fn observe_report(
    engine: &mut Engine,
    config: &Config,
    watchlist: &[String],
    report: &DailyPurchases,
    now_ms: u64,
) -> Result<Vec<Alert>, String> {
    let today = snipers_core::calendar::date_at(now_ms)
        .ok_or("Fecha ET no disponible")?
        .to_string();
    if report.date != today || report.through_ms > now_ms {
        return Err("La búsqueda no corresponde a la fecha/hora ET actual".into());
    }
    let rows = report
        .rows
        .iter()
        .map(|row| (row.ticker.clone(), row.block.clone()))
        .collect::<Vec<_>>();
    engine.observe(config, watchlist, &rows, now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use snipers_core::block_alerts::Scope;

    #[test]
    fn rejects_old_or_future_report_before_generating_alerts() {
        let now = snipers_core::calendar::legacy_eastern_ms("2026-10-08 11:00:00").unwrap();
        let config = Config {
            scope: Scope::Selected(vec!["QQQ".into()]),
            minimum_notional: 100_000.,
            alert_buy: true,
            alert_sell: true,
        };
        let mut engine = Engine::default();
        let mut report = DailyPurchases {
            date: "2026-10-07".into(), through_ms: now - 1000,
            rows: vec![], coverage: vec![],
        };
        assert!(observe_report(&mut engine, &config, &[], &report, now).is_err());
        report.date = "2026-10-08".into();
        report.through_ms = now + 1000;
        assert!(observe_report(&mut engine, &config, &[], &report, now).is_err());
        report.through_ms = now;
        assert!(observe_report(&mut engine, &config, &[], &report, now).unwrap().is_empty());
    }
}
