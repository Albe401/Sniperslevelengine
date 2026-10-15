//! Exact structural levels and observed volume; never entry permission.
use crate::{flow::ContractWindow, structure::Structure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Default, Deserialize)]
pub struct Targets {
    pub call_wall: Option<f64>,
    pub put_wall: Option<f64>,
    pub magnet: Option<f64>,
    pub call_below: Option<f64>,
    pub call_above: Option<f64>,
    pub put_below: Option<f64>,
    pub put_above: Option<f64>,
    pub call_pull: Option<f64>,
    pub put_pull: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Spec {
    pub name: String,
    pub aliases: Vec<String>,
    pub kind: String,
    pub strike: f64,
}
pub fn specs(t: &Targets) -> Vec<Spec> {
    let mut out: Vec<Spec> = vec![];
    for (name, kind, strike) in [
        ("CALL_WALL", "call", t.call_wall),
        ("PUT_WALL", "put", t.put_wall),
        ("MAIN_MAGNET_CALL", "call", t.magnet),
        ("MAIN_MAGNET_PUT", "put", t.magnet),
        ("CALL_BELOW_WALL", "call", t.call_below),
        ("CALL_ABOVE_WALL", "call", t.call_above),
        ("PUT_BELOW_WALL", "put", t.put_below),
        ("PUT_ABOVE_WALL", "put", t.put_above),
        ("CALL_PULL_TARGET", "call", t.call_pull),
        ("PUT_PULL_TARGET", "put", t.put_pull),
    ] {
        let Some(strike) = strike.filter(|v| v.is_finite() && *v > 0.0) else {
            continue;
        };
        // Listed strikes use thousandths. Do not merge distinct raw strikes by rounding.
        if let Some(existing) = out
            .iter_mut()
            .find(|s| s.kind == kind && s.strike == strike)
        {
            existing.aliases.push(name.into());
        } else {
            out.push(Spec {
                name: name.into(),
                aliases: vec![name.into()],
                kind: kind.into(),
                strike,
            });
        }
    }
    out
}
pub fn structural_specs(s: &Structure) -> Vec<Spec> {
    if !s.complete || !s.liquidity_complete {
        return vec![];
    }
    specs(&Targets {
        call_wall: s.call_wall.as_ref().map(|w| w.strike),
        put_wall: s.put_wall.as_ref().map(|w| w.strike),
        magnet: s.selected.get("M1").copied().flatten(),
        call_below: s.call_wall.as_ref().and_then(|w| w.below),
        call_above: s.call_wall.as_ref().and_then(|w| w.above),
        put_below: s.put_wall.as_ref().and_then(|w| w.below),
        put_above: s.put_wall.as_ref().and_then(|w| w.above),
        call_pull: s
            .routes
            .iter()
            .find(|r| r.direction == "CALL")
            .and_then(|r| r.pull_target),
        put_pull: s
            .routes
            .iter()
            .find(|r| r.direction == "PUT")
            .and_then(|r| r.pull_target),
    })
}
#[derive(Clone, Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub levels_requested: usize,
    pub levels_covered: usize,
    pub coverage_pct: f64,
    /// Totals of fully covered levels only; unknown premiums stay unknown.
    pub contracts: Option<f64>,
    pub premium: Option<f64>,
    pub trades: Option<usize>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub minutes: u64,
    pub call_zone: Coverage,
    pub put_zone: Coverage,
    pub call_pull: Coverage,
    pub put_pull: Coverage,
}
fn nonnegative(n: f64) -> bool {
    n.is_finite() && n >= 0.0
}
fn coverage(
    specs: &[&Spec],
    expirations: &[String],
    rows: &[ContractWindow],
    minutes: u64,
    valid: bool,
) -> Coverage {
    let mut out = Coverage {
        levels_requested: specs.len(),
        ..Default::default()
    };
    let dates: BTreeSet<_> = expirations.iter().collect();
    if !valid || dates.is_empty() {
        return out;
    }
    for spec in specs {
        let mut matched = vec![];
        for expiry in &dates {
            let candidates: Vec<_> = rows
                .iter()
                .filter(|r| {
                    r.minutes == minutes
                        && r.kind == spec.kind
                        && r.strike == spec.strike
                        && &r.expiration == *expiry
                })
                .collect();
            // Ambiguous contracts or duplicate rows must not certify a level.
            if candidates.len() != 1 {
                break;
            }
            matched.push(candidates[0]);
        }
        if matched.len() != dates.len()
            || matched
                .iter()
                .any(|r| !r.covered || !r.contracts.is_some_and(nonnegative) || r.trades.is_none())
        {
            continue;
        }
        let contracts: f64 = matched.iter().map(|r| r.contracts.unwrap()).sum();
        let trades = matched
            .iter()
            .try_fold(0usize, |sum, r| sum.checked_add(r.trades.unwrap()));
        if !contracts.is_finite() || trades.is_none() {
            continue;
        }
        let premium = matched
            .iter()
            .try_fold(0.0, |sum, r| {
                r.premium.filter(|v| nonnegative(*v)).map(|v| sum + v)
            })
            .filter(|v| v.is_finite());
        if out.levels_covered == 0 {
            out.contracts = Some(contracts);
            out.trades = trades;
            out.premium = premium;
        } else {
            out.contracts = out.contracts.map(|v| v + contracts);
            out.trades = out.trades.and_then(|v| v.checked_add(trades.unwrap()));
            out.premium = out
                .premium
                .zip(premium)
                .map(|(a, b)| a + b)
                .filter(|v| v.is_finite());
        }
        out.levels_covered += 1;
    }
    if out.contracts.is_some_and(|v| !v.is_finite())
        || (out.levels_covered > 0 && out.trades.is_none())
    {
        return Coverage {
            levels_requested: specs.len(),
            ..Default::default()
        };
    }
    if !specs.is_empty() {
        out.coverage_pct = (out.levels_covered as f64 / specs.len() as f64 * 1000.0).round() / 10.0;
    }
    out
}
pub fn windows(
    specs: &[Spec],
    expirations: &[String],
    rows: &[ContractWindow],
    valid: bool,
) -> Vec<Window> {
    [1, 5]
        .into_iter()
        .map(|minutes| {
            let group = |aliases: &[&str]| {
                let selected: Vec<_> = specs
                    .iter()
                    .filter(|s| s.aliases.iter().any(|a| aliases.contains(&a.as_str())))
                    .collect();
                coverage(&selected, expirations, rows, minutes, valid)
            };
            Window {
                minutes,
                call_zone: group(&["CALL_BELOW_WALL", "CALL_WALL", "CALL_ABOVE_WALL"]),
                put_zone: group(&["PUT_BELOW_WALL", "PUT_WALL", "PUT_ABOVE_WALL"]),
                call_pull: group(&["CALL_PULL_TARGET"]),
                put_pull: group(&["PUT_PULL_TARGET"]),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_targets_preserve_aliases_without_duplicate_contracts() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-wall-targets.json"))
                .unwrap();
        for case in cases["cases"].as_array().unwrap() {
            let t: Targets = serde_json::from_value(case["input"].clone()).unwrap();
            assert_eq!(serde_json::to_value(specs(&t)).unwrap(), case["expected"]);
        }
        assert!(specs(&Targets {
            call_wall: Some(f64::NAN),
            put_wall: Some(f64::INFINITY),
            ..Default::default()
        })
        .is_empty());
        assert_eq!(
            specs(&Targets {
                call_wall: Some(500.0000001),
                magnet: Some(500.0000002),
                ..Default::default()
            })
            .len(),
            3
        );
    }
    fn row(strike: f64) -> ContractWindow {
        ContractWindow {
            ticker: format!("O:QQQ-{strike}"),
            kind: "call".into(),
            strike,
            expiration: "2026-09-24".into(),
            minutes: 1,
            observed_since_ms: Some(1),
            covered: true,
            contracts: Some(10.0),
            premium: Some(1000.0),
            trades: Some(1),
            reason: String::new(),
        }
    }
    #[test]
    fn exact_zones_require_expirations_and_do_not_double_count_aliases() {
        // Magnet is added before adjacent levels in Python; zone must find its alias.
        let s = specs(&Targets {
            call_wall: Some(510.0),
            magnet: Some(500.0),
            call_below: Some(500.0),
            call_above: Some(520.0),
            call_pull: Some(520.0),
            ..Default::default()
        });
        let dates = vec!["2026-09-24".into()];
        let rows = vec![row(500.0), row(510.0), row(520.0)];
        let w = &windows(&s, &dates, &rows, true)[0];
        assert_eq!(w.call_zone.levels_requested, 3);
        assert_eq!(w.call_zone.coverage_pct, 100.0);
        assert_eq!(w.call_zone.contracts, Some(30.0));
        assert_eq!(w.call_pull.contracts, Some(10.0));
        assert_eq!(w.put_zone.contracts, None);
        assert_eq!(
            windows(&s, &dates, &rows, true)[1].call_zone.contracts,
            None
        );
        let partial = &windows(&s, &dates, &rows[..2], true)[0];
        assert_eq!(partial.call_zone.coverage_pct, 66.7);
        assert_eq!(partial.call_zone.contracts, Some(20.0));
        assert_eq!(partial.call_pull.contracts, None);
        assert_eq!(
            windows(&s, &[dates[0].clone(), "2026-09-25".into()], &rows, true)[0]
                .call_zone
                .contracts,
            None
        );
        assert_eq!(
            windows(&s, &dates, &rows, false)[0].call_zone.contracts,
            None
        );
        assert_eq!(windows(&s, &[], &rows, true)[0].call_zone.contracts, None);
        let mut duplicate = rows.clone();
        duplicate.push(rows[0].clone());
        assert_eq!(
            windows(&s, &dates, &duplicate, true)[0]
                .call_zone
                .levels_covered,
            2
        );
        let mut missing_premium = rows.clone();
        missing_premium[0].premium = None;
        let w = &windows(&s, &dates, &missing_premium, true)[0];
        assert_eq!(w.call_zone.contracts, Some(30.0));
        assert_eq!(w.call_zone.premium, None);
        let mut zero = rows.clone();
        for r in &mut zero {
            r.contracts = Some(0.0);
            r.premium = Some(0.0);
            r.trades = Some(0);
        }
        assert_eq!(
            windows(&s, &dates, &zero, true)[0].call_zone.contracts,
            Some(0.0)
        );
        let mut overflow = rows.clone();
        for r in &mut overflow {
            r.contracts = Some(f64::MAX);
        }
        let w = &windows(&s, &dates, &overflow, true)[0];
        assert_eq!(w.call_zone.levels_covered, 0);
        assert_eq!(w.call_zone.contracts, None);
        let mut wrong = rows.clone();
        wrong[0].kind = "put".into();
        wrong[1].strike += 0.001;
        wrong[2].covered = false;
        assert_eq!(
            windows(&s, &dates, &wrong, true)[0]
                .call_zone
                .levels_covered,
            0
        );
    }
}
