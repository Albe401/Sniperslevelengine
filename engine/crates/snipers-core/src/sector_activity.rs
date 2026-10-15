//! Observed trading activity, not net fund flows or institutional attribution.
use crate::{scanner, universe};
use serde::Serialize;
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sector {
    pub name: String,
    pub expected: usize,
    pub observed: usize,
    pub notional: Option<f64>,
    pub share_pct: Option<f64>,
    pub above_vwap_pct: Option<f64>,
    pub vwap_observed: usize,
    pub mean_rvol: Option<f64>,
    pub rvol_observed: usize,
    pub leaders: Vec<String>,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub bar_end_ms: Option<u64>,
    pub sectors: Vec<Sector>,
    pub rotation: Vec<Rotation>,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rotation {
    pub minutes: usize,
    pub ready: bool,
    pub reason: String,
    pub coverage_pct: f64,
    pub sectors: Vec<Change>,
    pub leaders: Vec<RotationLeader>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RotationLeader {
    pub symbol: String,
    pub sector: String,
    pub current_usd: f64,
    pub previous_usd: f64,
    pub change_usd: f64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub name: String,
    pub current_usd: f64,
    pub previous_usd: f64,
    pub change_usd: f64,
    pub share_pct: f64,
    pub change_pp: f64,
    pub observed: usize,
    pub expected: usize,
    pub state: String,
}
#[derive(Clone, Default)]
pub struct History {
    buckets: std::collections::BTreeMap<u64, Vec<scanner::Candidate>>,
    date: Option<String>,
}
impl History {
    pub fn update(&mut self, inputs: &[scanner::Input], now: u64, connected: bool) -> Report {
        let date = crate::calendar::date_at(now).map(|d| d.to_string());
        if self.date != date || !connected {
            self.buckets.clear();
            self.date = date;
        }
        let mut report = read(inputs, now, connected);
        if let Some(end) = report.bar_end_ms {
            let rows = scanner::rank_all(inputs, now)
                .into_iter()
                .filter(|c| {
                    c.bar_end_ms == end
                        && universe::assets().get(&c.symbol).is_some_and(|a| {
                            a.kind == universe::AssetKind::Stock && a.sector.is_some()
                        })
                })
                .collect();
            // Replaces the entire cut on corrections; repeated UI publications never add volume.
            self.buckets.insert(end, rows);
        }
        self.buckets
            .retain(|end, _| now.saturating_sub(*end) <= 7_200_000);
        report.rotation = [10, 30]
            .into_iter()
            .map(|minutes| self.compare(report.bar_end_ms, minutes))
            .collect();
        report
    }
    fn compare(&self, end: Option<u64>, minutes: usize) -> Rotation {
        let mut result = Rotation {
            minutes,
            reason: "Acumulando ventanas completas y comparables".into(),
            ..Default::default()
        };
        let Some(end) = end else {
            result.reason = "Sin datos vigentes".into();
            return result;
        };
        let count = minutes / 2;
        let Some(start) = end.checked_sub((count * 2 - 1) as u64 * 120_000) else {
            return result;
        };
        let mut cuts = Vec::new();
        for i in 0..count * 2 {
            let Some(rows) = self.buckets.get(&(start + i as u64 * 120_000)) else {
                return result;
            };
            cuts.push(rows);
        }
        // Fixed intersection across every candle in both windows: missing symbols cannot create rotation.
        let common: Vec<_> = cuts[0]
            .iter()
            .filter(|c| {
                cuts.iter()
                    .all(|rows| rows.iter().any(|r| r.symbol == c.symbol))
            })
            .collect();
        let expected = universe::assets()
            .values()
            .filter(|a| a.kind == universe::AssetKind::Stock && a.sector.is_some())
            .count();
        result.coverage_pct = common.len() as f64 / expected.max(1) as f64 * 100.0;
        if result.coverage_pct < 80.0 {
            result.reason = "Cobertura comparable inferior al 80% del universo".into();
            return result;
        }
        let mut totals = std::collections::BTreeMap::<String, (f64, f64, usize)>::new();
        let mut leaders = Vec::new();
        for asset in common {
            let mut previous = 0.0;
            let mut current = 0.0;
            let entry = totals.entry(asset.sector.clone().unwrap()).or_default();
            entry.2 += 1;
            for (i, rows) in cuts.iter().enumerate() {
                let v = rows
                    .iter()
                    .find(|r| r.symbol == asset.symbol)
                    .unwrap()
                    .notional_2m;
                if i < count {
                    entry.0 += v;
                    previous += v;
                } else {
                    entry.1 += v;
                    current += v;
                }
            }
            leaders.push(RotationLeader {
                symbol: asset.symbol.clone(),
                sector: asset.sector.clone().unwrap(),
                current_usd: current,
                previous_usd: previous,
                change_usd: current - previous,
            });
        }
        let old: f64 = totals.values().map(|v| v.0).sum();
        let new: f64 = totals.values().map(|v| v.1).sum();
        if !old.is_finite() || !new.is_finite() || old <= 0.0 || new <= 0.0 {
            return result;
        }
        for (name, (previous, current, observed)) in totals {
            let expected = universe::assets()
                .values()
                .filter(|a| {
                    a.kind == universe::AssetKind::Stock && a.sector.as_ref() == Some(&name)
                })
                .count();
            let delta = current / new * 100.0 - previous / old * 100.0;
            let state = if observed as f64 / (expected.max(1) as f64) < 0.8 {
                "COBERTURA PARCIAL"
            } else if delta >= 1.0 && current > previous {
                "GANANDO CONCENTRACIÓN"
            } else if delta <= -1.0 && current < previous {
                "PERDIENDO CONCENTRACIÓN"
            } else {
                "SIN ROTACIÓN CLARA"
            };
            result.sectors.push(Change {
                name,
                current_usd: current,
                previous_usd: previous,
                change_usd: current - previous,
                share_pct: current / new * 100.0,
                change_pp: delta,
                observed,
                expected,
                state: state.into(),
            });
        }
        result.sectors.sort_by(|a, b| {
            b.change_pp
                .total_cmp(&a.change_pp)
                .then(a.name.cmp(&b.name))
        });
        // A paired sector signal is required; individual volume growth alone is not rotation.
        if result
            .sectors
            .iter()
            .any(|s| s.state == "PERDIENDO CONCENTRACIÓN")
        {
            leaders.retain(|a| {
                a.change_usd.is_finite()
                    && a.change_usd > 0.0
                    && a.current_usd.is_finite()
                    && result
                        .sectors
                        .iter()
                        .any(|s| s.name == a.sector && s.state == "GANANDO CONCENTRACIÓN")
            });
            leaders.sort_by(|a, b| {
                b.change_usd
                    .total_cmp(&a.change_usd)
                    .then(b.current_usd.total_cmp(&a.current_usd))
                    .then(a.symbol.cmp(&b.symbol))
            });
            result.leaders = leaders;
        }
        result.ready = true;
        result.reason =
            "Ventanas consecutivas de igual duración · mismos activos · señal estimada".into();
        result
    }
}
pub fn read(inputs: &[scanner::Input], now: u64, connected: bool) -> Report {
    let candidates = if connected {
        scanner::rank_all(inputs, now)
    } else {
        vec![]
    };
    let stocks: Vec<_> = candidates
        .iter()
        .filter(|c| {
            universe::assets()
                .get(&c.symbol)
                .is_some_and(|a| a.kind == universe::AssetKind::Stock && a.sector.is_some())
        })
        .collect();
    // Compare the latest common time bucket only, never adjacent 2m windows.
    let end = stocks.iter().map(|c| c.bar_end_ms).max();
    let current: Vec<_> = stocks
        .into_iter()
        .filter(|c| Some(c.bar_end_ms) == end)
        .collect();
    let total: f64 = current.iter().map(|c| c.notional_2m).sum();
    let mut sectors = std::collections::BTreeMap::<String, Sector>::new();
    for asset in universe::assets()
        .values()
        .filter(|a| a.kind == universe::AssetKind::Stock)
    {
        if let Some(name) = &asset.sector {
            let sector = sectors.entry(name.clone()).or_insert_with(|| Sector {
                name: name.clone(),
                ..Default::default()
            });
            sector.expected += 1;
        }
    }
    for sector in sectors.values_mut() {
        let rows: Vec<_> = current
            .iter()
            .filter(|c| c.sector.as_ref() == Some(&sector.name))
            .collect();
        sector.observed = rows.len();
        if rows.is_empty() {
            continue;
        }
        let notional: f64 = rows.iter().map(|c| c.notional_2m).sum();
        sector.notional = notional.is_finite().then_some(notional);
        sector.share_pct = (total.is_finite() && total > 0.0)
            .then_some(notional / total * 100.0)
            .filter(|v| v.is_finite());
        sector.vwap_observed = rows.iter().filter(|c| c.vwap.is_some()).count();
        if sector.vwap_observed > 0 {
            sector.above_vwap_pct = Some(
                rows.iter()
                    .filter(|c| c.vwap.is_some_and(|v| c.price > v))
                    .count() as f64
                    / sector.vwap_observed as f64
                    * 100.0,
            );
        }
        let rvols: Vec<_> = rows.iter().filter_map(|c| c.rvol).collect();
        sector.rvol_observed = rvols.len();
        if !rvols.is_empty() {
            sector.mean_rvol = Some(rvols.iter().map(|v| v / rvols.len() as f64).sum::<f64>())
                .filter(|v| v.is_finite());
        }
        sector.leaders = rows.iter().take(3).map(|c| c.symbol.clone()).collect();
    }
    let mut sectors: Vec<_> = sectors.into_values().collect();
    sectors.sort_by(|a, b| {
        b.notional
            .unwrap_or(0.0)
            .total_cmp(&a.notional.unwrap_or(0.0))
            .then(a.name.cmp(&b.name))
    });
    Report {
        bar_end_ms: end,
        sectors,
        rotation: vec![],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn replay(
        factor: impl Fn(usize, &str) -> f64,
        skip: Option<usize>,
    ) -> (History, Report, Vec<scanner::Input>, u64) {
        let start = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut history = History::default();
        let mut result = Report::default();
        let mut rows = vec![];
        for n in 0..30 {
            if skip == Some(n) {
                continue;
            }
            let end = start + n as u64 * 120_000;
            rows = universe::assets()
                .iter()
                .filter(|(_, a)| a.kind == universe::AssetKind::Stock && a.sector.is_some())
                .map(|(symbol, a)| {
                    input(symbol, end, 100.0 * factor(n, a.sector.as_deref().unwrap()))
                })
                .collect();
            result = history.update(&rows, end, true);
        }
        (history, result, rows, start + 29 * 120_000)
    }
    #[test]
    fn uniform_market_expansion_or_contraction_is_not_sector_rotation() {
        for factor in [0.25, 2.0] {
            let (_, report, _, _) = replay(|n, _| if n < 15 { 1.0 } else { factor }, None);
            assert!(report.rotation.iter().all(|r| r.ready));
            assert!(report
                .rotation
                .iter()
                .flat_map(|r| &r.sectors)
                .all(|s| s.state == "SIN ROTACIÓN CLARA" && s.change_pp.abs() < 1e-8));
        }
    }
    #[test]
    fn relative_gain_without_dollar_growth_never_labels_inflow() {
        let tech = universe::assets()["AAPL"].sector.as_deref().unwrap();
        let (_, report, _, _) = replay(
            |n, sector| {
                if n < 15 {
                    1.0
                } else if sector == tech {
                    0.8
                } else {
                    0.2
                }
            },
            None,
        );
        let window = &report.rotation[1];
        assert!(window.ready);
        let sector = window.sectors.iter().find(|s| s.name == tech).unwrap();
        assert!(sector.change_pp > 1.0 && sector.change_usd < 0.0);
        assert_eq!(sector.state, "SIN ROTACIÓN CLARA");
        assert!(!window
            .sectors
            .iter()
            .any(|s| s.state == "GANANDO CONCENTRACIÓN"));
    }
    #[test]
    fn missing_bucket_and_disconnect_require_rebuilding_comparable_history() {
        let (_, report, _, _) = replay(|_, _| 1.0, Some(12));
        assert!(!report.rotation[1].ready);
        assert!(report.rotation[0].ready);
        let (mut history, _, rows, end) = replay(|_, _| 1.0, None);
        assert!(!history.update(&rows, end, false).rotation[0].ready);
        assert!(history.buckets.is_empty());
        assert!(!history.update(&rows, end, true).rotation[0].ready);
    }
    #[test]
    fn removing_one_symbol_does_not_create_rotation_in_the_remaining_cohort() {
        let (mut history, _, mut rows, end) = replay(|_, _| 1.0, None);
        rows.retain(|r| r.symbol != "AAPL");
        let report = history.update(&rows, end, true);
        for window in &report.rotation {
            assert!(window.ready && window.coverage_pct < 100.0);
            assert!(window
                .sectors
                .iter()
                .all(|s| s.change_pp.abs() < 1e-8 && s.change_usd.abs() < 1e-8));
        }
    }
    #[test]
    fn rotation_requires_equal_windows_and_stable_coverage() {
        let start = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut history = History::default();
        let mut last = Vec::new();
        for n in 0..10 {
            let end = start + n * 120_000;
            last = universe::assets()
                .iter()
                .filter(|(_, a)| a.kind == universe::AssetKind::Stock && a.sector.is_some())
                .map(|(s, a)| {
                    let growth = a.sector == universe::assets()["AAPL"].sector;
                    input(
                        s,
                        end,
                        if n < 5 {
                            100.0
                        } else if s == "AAPL" {
                            500.0
                        } else if s == "MSFT" {
                            20.0
                        } else if growth {
                            200.0
                        } else {
                            50.0
                        },
                    )
                })
                .collect();
            let report = history.update(&last, end, true);
            if n < 9 {
                assert!(!report.rotation[0].ready);
            }
        }
        let end = start + 9 * 120_000;
        let report = history.update(&last, end, true);
        let change = &report.rotation[0];
        assert!(change.ready);
        assert_eq!(change.leaders[0].symbol, "AAPL");
        assert_eq!(change.leaders[0].previous_usd, 50_000.0);
        assert_eq!(change.leaders[0].current_usd, 250_000.0);
        assert_eq!(change.leaders[0].change_usd, 200_000.0);
        assert!(change.leaders.iter().all(|a| a.symbol != "MSFT"
            && a.change_usd > 0.0
            && change
                .sectors
                .iter()
                .any(|s| s.name == a.sector && s.state == "GANANDO CONCENTRACIÓN")));
        assert_eq!(change.coverage_pct, 100.0);
        assert!(change
            .sectors
            .iter()
            .any(|s| s.state == "GANANDO CONCENTRACIÓN"));
        assert!(change
            .sectors
            .iter()
            .any(|s| s.state == "PERDIENDO CONCENTRACIÓN"));
        assert!(
            change
                .sectors
                .iter()
                .map(|s| s.change_pp)
                .sum::<f64>()
                .abs()
                < 0.000001
        );
        assert!(!report.rotation[1].ready);
        let repeated = history.update(&last, end, true);
        assert_eq!(repeated.rotation[0].leaders[0].change_usd, 200_000.0);
        assert_eq!(
            repeated.rotation[0].sectors[0].current_usd,
            change.sectors[0].current_usd
        );
        let disconnected = history.update(&last, end, false);
        assert!(!disconnected.rotation[0].ready);
        assert!(disconnected.rotation[0].leaders.is_empty());
        // A corrected latest cut with massive data loss must remove the signal.
        assert!(!history.update(&last[..1], end, true).rotation[0].ready);
        assert!(!history.update(&last, end + 240001, true).rotation[0].ready);
        let tomorrow = crate::calendar::legacy_eastern_ms("2026-09-25 10:00:00").unwrap();
        history.update(&[], tomorrow, true);
        assert!(history.buckets.is_empty());
    }
    fn input(symbol: &str, end: u64, volume: f64) -> scanner::Input {
        scanner::Input {
            symbol: symbol.into(),
            bar_end_ms: end,
            price: 100.0,
            volume_2m: volume,
            vwap: Some(99.0),
            rvol: Some(2.0),
            source_verified: true,
        }
    }
    #[test]
    fn coverage_etfs_duplicates_and_same_window() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let rows = vec![
            input("AAPL", now, 10.0),
            input("MSFT", now, 20.0),
            input("QQQ", now, 999999.0),
            input("AMD", now - 120000, 500.0),
        ];
        let result = read(&rows, now, true);
        assert_eq!(result.sectors.iter().map(|s| s.observed).sum::<usize>(), 2);
        let sector = result.sectors.iter().find(|s| s.observed > 0).unwrap();
        assert_eq!(sector.notional, Some(3000.0));
        assert_eq!(sector.share_pct, Some(100.0));
        assert_eq!(sector.above_vwap_pct, Some(100.0));
        assert!(sector.expected > sector.observed);
        assert_eq!(sector.leaders, vec!["MSFT", "AAPL"]);
        assert!(read(&[rows[0].clone(), rows[0].clone()], now, true)
            .bar_end_ms
            .is_none());
        for report in [
            read(&rows, now, false),
            read(&rows, now + 120001, true),
            read(&rows, now - 240001, true),
        ] {
            assert!(report
                .sectors
                .iter()
                .all(|s| s.notional.is_none() && s.observed == 0));
        }
    }
    #[test]
    fn missing_direction_and_baseline_remain_missing() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut row = input("AAPL", now, 10.0);
        row.vwap = None;
        row.rvol = None;
        let result = read(&[row], now, true);
        let sector = result.sectors.iter().find(|s| s.observed > 0).unwrap();
        assert_eq!(sector.above_vwap_pct, None);
        assert_eq!(sector.mean_rvol, None);
    }
}
