//! Legacy heuristic confirmation with a separate, mandatory evidence gate.
//! Confirmation never changes the primary options magnet or grants an entry.
use crate::calendar;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct OptionsMagnet {
    pub price: f64,
    pub score: Option<f64>,
}
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct OffMagnet {
    pub price: f64,
    pub notional: f64,
    pub prints: usize,
    pub total_notional: f64,
    pub total_prints: usize,
    pub concentration_pct: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Confirmation {
    pub status: String,
    pub reason: String,
    pub confirmation_score: f64,
    pub alignment: String,
    pub options_score: Option<f64>,
    pub off_score: Option<f64>,
    pub primary_magnet: Option<f64>,
    pub off_magnet: Option<f64>,
    pub distance: Option<f64>,
    pub distance_pct: Option<f64>,
    pub entry_confirmed: bool,
}
fn waiting(primary: Option<f64>, status: &str, reason: &str) -> Confirmation {
    Confirmation {
        status: status.into(),
        reason: reason.into(),
        confirmation_score: 0.0,
        alignment: "N/A".into(),
        options_score: None,
        off_score: None,
        primary_magnet: primary,
        off_magnet: None,
        distance: None,
        distance_pct: None,
        entry_confirmed: false,
    }
}
fn round(value: f64, scale: f64) -> f64 {
    (value * scale).round_ties_even() / scale
}

/// Pure legacy formula. Live consumers must use `guarded`, not this calculation alone.
pub fn calculate(
    options: Option<OptionsMagnet>,
    off: Option<OffMagnet>,
    spot: Option<f64>,
) -> Result<Confirmation, &'static str> {
    let Some(options) = options else {
        return Ok(waiting(None, "NO_OPTIONS_MAGNET", "Sin imán de opciones"));
    };
    if !options.price.is_finite()
        || options.price <= 0.0
        || options.score.is_some_and(|v| !v.is_finite())
    {
        return Err("Imán de opciones inválido");
    }
    let Some(off) = off else {
        return Ok(waiting(
            Some(options.price),
            "NO_DARK_POOL_MAGNET",
            "Sin imán fuera de bolsa",
        ));
    };
    if [
        off.price,
        off.notional,
        off.total_notional,
        off.concentration_pct,
    ]
    .iter()
    .any(|v| !v.is_finite() || *v < 0.0)
        || off.price == 0.0
        || off.notional > off.total_notional
        || off.prints > off.total_prints
        || spot.is_some_and(|v| !v.is_finite() || v <= 0.0)
    {
        return Err("Evidencia de imán inválida");
    }
    let reference = spot.unwrap_or(options.price / 2.0 + off.price / 2.0);
    let distance = (options.price - off.price).abs();
    let fraction = distance / reference;
    let (alignment, score) = if fraction <= 0.0015 {
        ("ALINEACIÓN FUERTE", 100.0)
    } else if fraction <= 0.0035 {
        (
            "ALINEACIÓN MEDIA",
            100.0 - (fraction - 0.0015) / 0.002 * 30.0,
        )
    } else if fraction <= 0.006 {
        (
            "ALINEACIÓN DÉBIL",
            70.0 - (fraction - 0.0035) / 0.0025 * 40.0,
        )
    } else {
        ("DIVERGENCIA", 0.0)
    };
    let concentration = if off.concentration_pct <= 0.0 && off.total_notional > 0.0 {
        off.notional / off.total_notional * 100.0
    } else {
        off.concentration_pct
    };
    let print_share = if off.total_prints > 0 {
        off.prints as f64 / off.total_prints as f64 * 100.0
    } else {
        0.0
    };
    let dark_score = round(
        ((concentration / 20.0).clamp(0.0, 1.0) * 0.7 + (print_share / 2.0).clamp(0.0, 1.0) * 0.3)
            * 100.0,
        100.0,
    );
    let mut options_score = options.score.unwrap_or(50.0);
    if (0.0..=1.0).contains(&options_score) {
        options_score *= 100.0;
    }
    options_score = options_score.clamp(0.0, 100.0);
    let mut final_score =
        round(score.clamp(0.0, 100.0), 100.0) * 0.45 + options_score * 0.30 + dark_score * 0.25;
    let status = if alignment == "DIVERGENCIA" {
        final_score = final_score.min(49.0);
        "DIVERGENCIA"
    } else if alignment == "ALINEACIÓN DÉBIL" {
        final_score = final_score.min(64.9);
        if final_score >= 50.0 {
            "PARTIAL_CONFIRMATION"
        } else {
            "WEAK_CONFIRMATION"
        }
    } else if final_score >= 80.0 {
        "CONFIRMED_STRONG"
    } else if final_score >= 65.0 {
        "CONFIRMED"
    } else if final_score >= 50.0 {
        "PARTIAL_CONFIRMATION"
    } else {
        "WEAK_CONFIRMATION"
    };
    let distance = round(distance, 10_000.0);
    let distance_pct = round(fraction * 100.0, 10_000.0);
    if !distance.is_finite() || !distance_pct.is_finite() {
        return Err("Distancia fuera de rango");
    }
    Ok(Confirmation {
        status: status.into(),
        reason: "Confirmación heurística de ubicación; no permiso de entrada".into(),
        confirmation_score: round(final_score, 100.0),
        alignment: alignment.into(),
        options_score: Some(round(options_score, 100.0)),
        off_score: Some(dark_score),
        primary_magnet: Some(options.price),
        off_magnet: Some(off.price),
        distance: Some(distance),
        distance_pct: Some(distance_pct),
        entry_confirmed: false,
    })
}

pub struct Evidence<'a> {
    pub symbol: &'a str,
    pub off_symbol: &'a str,
    pub options_ms: Option<u64>,
    pub off_ms: Option<u64>,
    pub spot_ms: Option<u64>,
    pub options_complete: bool,
    pub off_complete: bool,
}
pub fn guarded(
    options: Option<OptionsMagnet>,
    off: Option<OffMagnet>,
    spot: Option<f64>,
    evidence: Evidence<'_>,
    now: u64,
) -> Confirmation {
    let fresh = |time: Option<u64>, age: u64| {
        time.is_some_and(|t| {
            t <= now
                && now - t <= age
                && calendar::date_at(t).is_some()
                && calendar::date_at(t) == calendar::date_at(now)
        })
    };
    let primary = options
        .map(|o| o.price)
        .filter(|p| p.is_finite() && *p > 0.0);
    if evidence.symbol != evidence.off_symbol
        || crate::universe::stock_asset(evidence.symbol).is_none()
        || !evidence.options_complete
        || !evidence.off_complete
        || !fresh(evidence.options_ms, crate::dex_change::MAX_GAP_MS)
        || !fresh(evidence.off_ms, 15_000)
        || !fresh(evidence.spot_ms, 15_000)
        || spot.is_none()
    {
        return waiting(
            primary,
            "WAITING",
            "Cobertura, activo o frescura insuficientes para confirmar",
        );
    }
    calculate(options, off, spot).unwrap_or_else(|reason| waiting(primary, "WAITING", reason))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fourteen_python_fixtures_preserve_alignment_caps_and_primary_magnet() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/legacy-magnet-confirmation.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let options: Option<OptionsMagnet> =
                serde_json::from_value(case["options"].clone()).unwrap();
            let off = serde_json::from_value(case["off"].clone()).unwrap();
            let result = calculate(options, off, case["spot"].as_f64()).unwrap();
            let e = &case["expected"];
            assert_eq!(
                result.status,
                e["status"].as_str().unwrap(),
                "{}",
                case["name"]
            );
            assert!(
                (result.confirmation_score - e["confirmation_score"].as_f64().unwrap()).abs()
                    < 1e-8
            );
            for (actual, key) in [
                (result.distance, "distance"),
                (result.distance_pct, "distance_pct"),
                (result.options_score, "options_score"),
                (result.off_score, "dark_pool_score"),
                (result.primary_magnet, "primary_magnet"),
            ] {
                assert_eq!(actual, e[key].as_f64(), "{} {key}", case["name"]);
            }
            assert_eq!(result.alignment, e["alignment"].as_str().unwrap());
            assert!(!result.entry_confirmed);
        }
    }
    #[test]
    fn invalid_incomplete_stale_foreign_and_future_evidence_never_confirm() {
        let now = crate::calendar::day_bounds("2026-09-24").unwrap().0 + 10 * 3_600_000;
        let options = Some(OptionsMagnet {
            price: 500.0,
            score: Some(90.0),
        });
        let off = Some(OffMagnet {
            price: 500.0,
            notional: 200_000.0,
            prints: 20,
            total_notional: 1_000_000.0,
            total_prints: 1000,
            concentration_pct: 20.0,
        });
        for variant in 0..7 {
            let mut evidence = Evidence {
                symbol: "QQQ",
                off_symbol: "QQQ",
                options_ms: Some(now),
                off_ms: Some(now),
                spot_ms: Some(now),
                options_complete: true,
                off_complete: true,
            };
            match variant {
                0 => evidence.off_complete = false,
                1 => evidence.options_complete = false,
                2 => evidence.off_ms = Some(now - 15_001),
                3 => evidence.options_ms = Some(now - 900_001),
                4 => evidence.spot_ms = Some(now + 1),
                5 => evidence.off_symbol = "SPY",
                _ => {}
            }
            let actual = guarded(options, off, Some(500.0), evidence, now);
            assert_eq!(
                actual.status,
                if variant == 6 {
                    "CONFIRMED_STRONG"
                } else {
                    "WAITING"
                }
            );
            assert_eq!(actual.primary_magnet, Some(500.0));
            assert!(!actual.entry_confirmed);
        }
        assert!(calculate(
            Some(OptionsMagnet {
                price: f64::NAN,
                score: None
            }),
            off,
            None
        )
        .is_err());
    }
}
