//! Gross reported traded premium. Never infer dollars from option-bar volume or VWAP.
use crate::{live::Bar, option_activity_history::Report};
use serde::{Deserialize, Serialize};
use snipers_core::{
    calendar::{self, Day},
    option_activity_baseline::Sample,
    options,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PremiumQuality {
    pub version: String,
    pub status: String,
    pub reasons: Vec<String>,
    pub unique_trades: usize,
    pub certified_premium: Option<f64>,
    pub measurement: String,
    pub scope: String,
    pub execution_enabled: bool,
}
fn equal(a: f64, b: f64) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= 1e-8 + 1e-10 * a.abs().max(b.abs())
}
fn identity_valid(value: &str) -> bool {
    if let Some(s) = value.strip_prefix("seq:") {
        return s.parse::<u64>().is_ok_and(|n| n > 0);
    }
    value
        .strip_prefix("id:")
        .and_then(|s| s.split_once(':'))
        .is_some_and(|(exchange, id)| {
            exchange.parse::<u64>().is_ok() && !id.is_empty() && id != "0" && id.len() <= 256
        })
}
/// Independently check the exported trade evidence and every certified window.
pub fn certify(report: &Report) -> PremiumQuality {
    certify_bounds(report, None)
}
pub fn certify_window(window: &crate::option_activity_history::WindowReport) -> PremiumQuality {
    let mut quality = certify_bounds(&window.report, Some((window.start_ms, window.end_ms)));
    if window.version != "option-premium-window-evidence-v1"
        || window.capture_started_ms < window.end_ms
        || window.capture_started_ms.saturating_sub(window.end_ms) > 120_000
        || window.report.collected_ms < window.capture_started_ms
    {
        quality
            .reasons
            .push("invalid-prospective-capture-clock".into());
        quality.status = "PENDING_PREMIUM_DATA".into();
        quality.certified_premium = None;
    }
    quality
}
fn certify_bounds(report: &Report, bounds: Option<(u64, u64)>) -> PremiumQuality {
    let mut reasons = BTreeSet::<String>::new();
    let (start, end) = bounds.unwrap_or((report.session.open_ms, report.session.close_ms));
    let expected_version = if bounds.is_some() {
        "option-activity-window-v1"
    } else {
        "option-activity-history-v1"
    };
    let expected_scope = if bounds.is_some() {
        "reported-prints-single-requested-contract-equity-RTH-window"
    } else {
        "reported-prints-single-requested-contract-equity-RTH"
    };
    if report.version != expected_version
        || report.window_clock != "sip_timestamp"
        || report.scope != expected_scope
    {
        reasons.insert("unsupported-report-definition".into());
    }
    if !report.retrieval_complete {
        reasons.insert("incomplete-pagination".into());
    }
    if !report.aggregation_complete {
        reasons.insert("uncertified-aggregation".into());
    }
    if report.rejected + report.corrected + report.conflicts > 0 {
        reasons.insert("rejected-corrected-or-conflicting-trades".into());
    }
    if report.unidentified > 0 {
        reasons.insert("unverified-trade-identity".into());
    }
    if report.contract.premium_multiplier != Some(100.) {
        reasons.insert("unverified-standard-multiplier".into());
    }
    if report.trades.is_empty() {
        reasons.insert("empty-history-not-certified-zero".into());
    }
    if report.rows_received != report.trades.len().saturating_add(report.duplicates)
        || report.pages == 0
        || report.pages > report.request.max_pages
    {
        reasons.insert("inconsistent-retrieval-counts".into());
    }
    let calendar_session = calendar::day_bounds(&report.request.date)
        .and_then(|(start, _)| calendar::date_at(start))
        .and_then(|d| match calendar::schedule(d) {
            Day::Open(s) => Some(s),
            _ => None,
        });
    if calendar_session.as_ref().is_none_or(|s| {
        s.open_ms != report.session.open_ms
            || s.close_ms != report.session.close_ms
            || s.early_close != report.session.early_close
            || s.date != report.session.date
            || report.collected_ms < end
    }) {
        reasons.insert("invalid-session-or-collection-clock".into());
    }
    if options::underlying(&report.request.ticker) != Some(report.request.underlying.as_str())
        || report.contract.ticker != report.request.ticker
    {
        reasons.insert("contract-identity-mismatch".into());
    } else {
        let raw = report.request.ticker.strip_prefix("O:").unwrap_or("");
        let tail = &raw[raw.len() - 15..];
        let expiry = format!("20{}-{}-{}", &tail[..2], &tail[2..4], &tail[4..6]);
        let kind = if &tail[6..7] == "C" { "call" } else { "put" };
        if report.contract.expiration != expiry
            || report.contract.expiration < report.request.date
            || report.contract.kind != kind
            || tail[7..].parse::<u64>().ok().map(|v| v as f64 / 1000.)
                != Some(report.contract.strike)
        {
            reasons.insert("contract-metadata-mismatch".into());
        }
    }
    let valid_window = [1, 5, 15, 30].contains(&report.request.window_minutes);
    let step = if valid_window {
        report.request.window_minutes * 60_000
    } else {
        300_000
    };
    let session_valid = start < end
        && start >= report.session.open_ms
        && end <= report.session.close_ms
        && report.session.open_ms.checked_mul(1_000_000).is_some()
        && report.session.close_ms.checked_mul(1_000_000).is_some();
    let n = if session_valid {
        (end - start).div_ceil(step) as usize
    } else {
        0
    };
    if !valid_window || n > 390 || report.windows.len() != n {
        reasons.insert("invalid-window-layout".into());
    }
    if bounds.is_some()
        && (!(1..=20).contains(&report.request.max_pages)
            || report.request.window_minutes != 5
            || end.checked_sub(start) != Some(step)
            || start < report.session.open_ms
            || !(start.saturating_sub(report.session.open_ms)).is_multiple_of(step))
    {
        reasons.insert("invalid-prospective-window-layout".into());
    }
    let mut totals = BTreeMap::<usize, (usize, f64, f64)>::new();
    let mut identities = BTreeSet::new();
    for trade in &report.trades {
        if !trade.identity_verified
            || !identity_valid(&trade.identity)
            || !identities.insert(trade.identity.clone())
        {
            reasons.insert("unverified-or-duplicate-trade-identity".into());
        }
        if !trade.price.is_finite()
            || trade.price <= 0.
            || !trade.contracts.is_finite()
            || trade.contracts <= 0.
            || !session_valid
            || trade.sip_ns < start.saturating_mul(1_000_000)
            || trade.sip_ns >= end.saturating_mul(1_000_000)
        {
            reasons.insert("invalid-trade-metrics-or-clock".into());
            continue;
        }
        let dollars = trade.price * trade.contracts * 100.;
        if trade.premium.is_none_or(|p| !equal(p, dollars)) {
            reasons.insert("trade-premium-does-not-match-price-size-multiplier".into());
        }
        let i = ((trade.sip_ns / 1_000_000 - start) / step) as usize;
        let row = totals.entry(i).or_default();
        row.0 += 1;
        row.1 += trade.contracts;
        row.2 += dollars;
    }
    let mut premium = 0.;
    for (i, w) in report.windows.iter().enumerate() {
        let (trades, contracts, dollars) = totals.get(&i).copied().unwrap_or_default();
        if w.start_ms != start.saturating_add(i as u64 * step)
            || w.end_ms != w.start_ms.saturating_add(step).min(end)
            || w.trades != trades
            || w.participant_clock_trades + w.sip_only_trades != trades
            || !equal(w.observed_contracts, contracts)
            || !equal(w.observed_premium, dollars)
            || w.contracts.is_none_or(|v| !equal(v, contracts))
            || w.premium.is_none_or(|v| !equal(v, dollars))
        {
            reasons.insert("window-totals-do-not-match-trade-evidence".into());
        }
        premium += dollars;
    }
    if !premium.is_finite() {
        reasons.insert("nonfinite-total-premium".into());
    }
    let certified = reasons.is_empty();
    PremiumQuality {
        version: "option-premium-quality-v1".into(),
        status: if certified {
            "CERTIFIED_REPORTED_PREMIUM"
        } else {
            "PENDING_PREMIUM_DATA"
        }
        .into(),
        reasons: reasons.into_iter().collect(),
        unique_trades: report.trades.len(),
        certified_premium: certified.then_some(premium),
        measurement: "reported-trades".into(),
        scope: "gross-price-times-size-times-verified-100; SIP-RTH; no-buy-sell-inference".into(),
        execution_enabled: false,
    }
}
/// Only independently certified reports can enter a premium reference.
pub fn samples(
    report: &Report,
    stock_bars: &[Bar],
    selected_after: u64,
) -> (PremiumQuality, Vec<Sample>, BTreeMap<String, usize>) {
    let quality = certify(report);
    let mut samples = vec![];
    let mut skipped = BTreeMap::new();
    if quality.certified_premium.is_none() {
        return (quality, samples, skipped);
    }
    let prices: BTreeMap<_, _> = stock_bars
        .iter()
        .map(|b| (b.timestamp_ms, b.close))
        .collect();
    for w in &report.windows {
        if w.start_ms < selected_after {
            *skipped
                .entry("selection-not-yet-available".into())
                .or_default() += 1;
            continue;
        }
        let price = w
            .start_ms
            .checked_sub(60_000)
            .and_then(|t| prices.get(&t))
            .copied();
        let Some(price) = price.filter(|p| p.is_finite() && *p > 0.) else {
            *skipped
                .entry("missing-prior-closed-stock-minute".into())
                .or_default() += 1;
            continue;
        };
        samples.push(Sample {
            underlying: report.request.underlying.clone(),
            ticker: report.request.ticker.clone(),
            kind: report.contract.kind.clone(),
            expiration: report.contract.expiration.clone(),
            strike: report.contract.strike,
            start_ms: w.start_ms,
            end_ms: w.end_ms,
            underlying_price: price,
            price_available_ms: w.start_ms,
            contracts: w.contracts.unwrap(),
            premium: w.premium,
            measurement: "reported-trades".into(),
            source_collected_ms: report.collected_ms,
            quality_complete: true,
        });
    }
    (quality, samples, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::option_activity_history::{Contract, Request, Session, Trade, Window};
    fn window_report() -> crate::option_activity_history::WindowReport {
        let mut r = report();
        let w = r.windows[1].clone();
        r.request.max_pages = 20;
        r.version = "option-activity-window-v1".into();
        r.scope = "reported-prints-single-requested-contract-equity-RTH-window".into();
        r.collected_ms = w.end_ms + 10_000;
        r.windows = vec![w.clone()];
        crate::option_activity_history::WindowReport {
            version: "option-premium-window-evidence-v1".into(),
            start_ms: w.start_ms,
            end_ms: w.end_ms,
            capture_started_ms: w.end_ms + 5000,
            report: r,
        }
    }
    #[test]
    fn certifies_closed_window_without_claiming_a_complete_day() {
        let w = window_report();
        assert!(w.report.collected_ms < w.report.session.close_ms);
        assert_eq!(certify_window(&w).certified_premium, Some(75.));
        assert!(certify(&w.report).certified_premium.is_none());
    }
    #[test]
    fn partial_empty_or_out_of_range_window_is_pending() {
        let mut w = window_report();
        w.report.retrieval_complete = false;
        assert!(certify_window(&w).certified_premium.is_none());
        w = window_report();
        w.report.trades.clear();
        w.report.rows_received = 0;
        assert!(certify_window(&w).certified_premium.is_none());
        w = window_report();
        w.report.trades[0].sip_ns = w.end_ms * 1_000_000;
        assert!(certify_window(&w).certified_premium.is_none());
    }
    #[test]
    fn prospective_evidence_cannot_have_open_or_delayed_dispatch() {
        let mut w = window_report();
        w.capture_started_ms = w.end_ms - 1;
        assert!(certify_window(&w).certified_premium.is_none());
        w = window_report();
        w.capture_started_ms = w.end_ms + 120_001;
        assert!(certify_window(&w).certified_premium.is_none());
        w = window_report();
        w.report.collected_ms = w.capture_started_ms - 1;
        assert!(certify_window(&w).certified_premium.is_none());
    }
    fn report() -> Report {
        let open = calendar::legacy_eastern_ms("2026-10-09 09:30:00").unwrap();
        let close = open + 23_400_000;
        let windows = (0..78)
            .map(|i| {
                let premium = if i == 1 { 75. } else { 0. };
                Window {
                    start_ms: open + i * 300_000,
                    end_ms: open + (i + 1) * 300_000,
                    trades: usize::from(i == 1),
                    observed_contracts: if i == 1 { 0.5 } else { 0. },
                    observed_premium: premium,
                    contracts: Some(if i == 1 { 0.5 } else { 0. }),
                    premium: Some(premium),
                    participant_clock_trades: 0,
                    sip_only_trades: usize::from(i == 1),
                }
            })
            .collect();
        Report {
            version: "option-activity-history-v1".into(),
            collected_ms: close + 86_400_000,
            request: Request {
                underlying: "QQQ".into(),
                ticker: "O:QQQ261009C00752000".into(),
                date: "2026-10-09".into(),
                window_minutes: 5,
                max_pages: 60,
            },
            contract: Contract {
                ticker: "O:QQQ261009C00752000".into(),
                kind: "call".into(),
                expiration: "2026-10-09".into(),
                strike: 752.,
                premium_multiplier: Some(100.),
            },
            session: Session {
                date: "2026-10-09".into(),
                open_ms: open,
                close_ms: close,
                early_close: false,
            },
            window_clock: "sip_timestamp".into(),
            scope: "reported-prints-single-requested-contract-equity-RTH".into(),
            pages: 1,
            rows_received: 1,
            duplicates: 0,
            rejected: 0,
            corrected: 0,
            conflicts: 0,
            unidentified: 0,
            retrieval_complete: true,
            aggregation_complete: true,
            warnings: vec![],
            windows,
            trades: vec![Trade {
                identity: "seq:1".into(),
                identity_verified: true,
                sip_ns: (open + 300_000) * 1_000_000,
                participant_ns: None,
                price: 1.5,
                contracts: 0.5,
                premium: Some(75.),
                conditions: vec![209],
            }],
        }
    }
    #[test]
    fn certifies_decimal_size_and_complete_quiet_windows() {
        let r = report();
        assert_eq!(certify(&r).certified_premium, Some(75.));
        assert_eq!(r.windows[0].premium, Some(0.));
    }
    #[test]
    fn partial_identity_corrections_and_unknown_multiplier_stay_pending() {
        let original = report();
        let mut r = original.clone();
        r.retrieval_complete = false;
        assert!(certify(&r).certified_premium.is_none());
        r = original.clone();
        r.unidentified = 1;
        assert!(certify(&r).certified_premium.is_none());
        r = original.clone();
        r.corrected = 1;
        assert!(certify(&r).certified_premium.is_none());
        r = original;
        r.contract.premium_multiplier = None;
        assert!(certify(&r).certified_premium.is_none());
    }
    #[test]
    fn forged_totals_and_duplicates_cannot_be_certified() {
        let mut r = report();
        r.trades[0].premium = Some(7500.);
        assert!(certify(&r).certified_premium.is_none());
        r = report();
        r.windows[1].premium = Some(7500.);
        assert!(certify(&r).certified_premium.is_none());
        r = report();
        r.trades.push(r.trades[0].clone());
        r.rows_received += 1;
        assert!(certify(&r).certified_premium.is_none());
    }
    #[test]
    fn identity_flags_cannot_certify_sequence_zero_or_empty_id() {
        let mut r = report();
        r.trades[0].identity = "seq:0".into();
        assert!(certify(&r).certified_premium.is_none());
        r.trades[0].identity = "id:1:".into();
        assert!(certify(&r).certified_premium.is_none());
    }
    #[test]
    fn adapter_requires_closed_stock_minute_and_certified_report() {
        let mut r = report();
        let open = r.session.open_ms;
        let b = Bar {
            timestamp_ms: open + 240_000,
            open: 752.,
            high: 752.,
            low: 752.,
            close: 752.,
            volume: 1.,
            vwap: None,
        };
        let (_, s, _) = samples(&r, &[b.clone()], open + 60_000);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].premium, Some(75.));
        assert_eq!(s[0].measurement, "reported-trades");
        assert!(samples(&r, &[], open + 60_000).1.is_empty());
        r.aggregation_complete = false;
        assert!(samples(&r, &[b], open + 60_000).1.is_empty());
    }
}
