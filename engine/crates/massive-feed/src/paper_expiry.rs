//! Explicit 7–10 calendar-day expiry, verified against actual Paper contracts.
use crate::alpaca::PaperClient;
use snipers_core::calendar::{date_at, day_bounds};

pub fn dates(session: &str) -> Result<Vec<String>, String> {
    let (ms, _) = day_bounds(session).ok_or("Fecha de sesión inválida")?;
    let mut date = date_at(ms).ok_or("Fecha no disponible")?;
    let mut out = vec![];
    for day in 1..=10 {
        date = date.succ_opt().ok_or("Vencimiento fuera de rango")?;
        if day >= 7 {
            out.push(date.to_string());
        }
    }
    Ok(out)
}
pub fn valid(session: &str, expiry: &str) -> bool {
    dates(session).is_ok_and(|dates| dates.iter().any(|d| d == expiry))
}
pub async fn select(client: &PaperClient, session: &str) -> Result<String, String> {
    for expiry in dates(session)? {
        let contracts = client.contracts("QQQ", &expiry, None).await?;
        if contracts["option_contracts"]
            .as_array()
            .is_some_and(|rows| {
                rows.iter().any(|r| {
                    r["expiration_date"] == expiry
                        && r["underlying_symbol"] == "QQQ"
                        && r["tradable"] == true
                        && r["status"] == "active"
                })
            })
        {
            return Ok(expiry);
        }
    }
    Err(
        "No se encontró vencimiento QQQ negociable entre 7 y 10 días; selección manual requerida"
            .into(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn week_uses_calendar_days_and_never_falls_back_to_zero_dte() {
        assert_eq!(
            dates("2026-09-28").unwrap(),
            ["2026-10-05", "2026-10-06", "2026-10-07", "2026-10-08"]
        );
        assert!(!valid("2026-09-28", "2026-09-28"));
        assert!(!valid("2026-09-28", "2026-10-02"));
        assert!(valid("2026-09-28", "2026-10-05"));
        assert_eq!(dates("2026-12-28").unwrap()[0], "2027-01-04");
    }
}
