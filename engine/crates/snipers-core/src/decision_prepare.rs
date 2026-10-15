//! Recompute a structural proposal at admission time. No ledger or broker effects.
use crate::{bars::SessionBar, calendar, magnet_trigger, paper, structural_path};

#[derive(Clone)]
pub struct Evidence {
    pub symbol: String,
    pub source_symbol: String,
    pub horizon: String,
    pub published_ms: u64,
    pub verified: bool,
    pub bars: Vec<SessionBar>,
    pub vwap: Option<f64>,
    pub path: structural_path::Input,
    pub price: f64,
    pub price_ms: u64,
}

/// The supplied ready/entry/tolerance flags are never used as permission.
/// The caller must still obtain transactional session/ledger admission.
pub fn prepare(e: &Evidence, now: u64) -> Result<paper::Trade, &'static str> {
    if !e.verified || e.symbol != e.source_symbol {
        return Err("UNVERIFIED_SOURCE");
    }
    if e.symbol != "QQQ" {
        return Err("SYMBOL_NOT_ENABLED");
    }
    if e.horizon != "DAY" {
        return Err("NOT_CURRENT_SESSION_HORIZON");
    }
    let current_date = calendar::date_at(now).ok_or("CALENDAR_UNAVAILABLE")?;
    for (timestamp, max_age, reason) in [
        (e.published_ms, 3_000, "STALE_DECISION_EVIDENCE"),
        (e.price_ms, 15_000, "STALE_ENTRY_PRICE"),
    ] {
        if timestamp > now
            || now - timestamp > max_age
            || calendar::date_at(timestamp) != Some(current_date)
        {
            return Err(reason);
        }
    }
    if !e.price.is_finite() || e.price <= 0.0 {
        return Err("INVALID_ENTRY_PRICE");
    }
    let magnet = e.path.magnet.ok_or("MISSING_GEOMETRY")?;
    let trigger = magnet_trigger::evaluate(&e.path.side, magnet, &e.bars, e.vwap, now, e.verified);
    if !trigger.ready
        || trigger
            .bar_end_ms
            .is_none_or(|end| now.saturating_sub(end) < 2_000)
    {
        return Err("MAGNET_2M_TRIGGER");
    }
    let call = e.path.side == "CALL";
    if (e.price - magnet).abs() / magnet * 100.0 > 0.35
        || if call {
            e.price <= magnet
        } else {
            e.price >= magnet
        }
    {
        return Err("ENTRY_MOVED_FROM_MAGNET");
    }
    let mut input = e.path.clone();
    input.entry = Some(e.price);
    input.trigger_ready = trigger.ready;
    input.tolerance = trigger.magnet_tolerance;
    input.volume_ratio = trigger.volume_ratio;
    let candidate = structural_path::evaluate(&input, true);
    if !candidate.ready {
        return Err("STRUCTURAL_PATH_NOT_READY");
    }
    let structure = paper::Structure {
        magnet,
        vacuum_low: input.vacuum_low.ok_or("MISSING_GEOMETRY")?,
        vacuum_high: input.vacuum_high.ok_or("MISSING_GEOMETRY")?,
        target_wall: input.wall.ok_or("MISSING_GEOMETRY")?,
    };
    // Legacy structural trailing requires the whole vacuum inside the corridor.
    // Reject explicitly rather than silently disabling structural management.
    paper::Trade::new(
        if call {
            paper::Side::CALL
        } else {
            paper::Side::PUT
        },
        e.price,
        candidate.invalidation_level.ok_or("INVALID_STOP")?,
        structure.target_wall,
        None, // The destination wall closes this structural route.
        Some(structure),
        now,
    )
    .map_err(|_| "STRUCTURAL_PAPER_GEOMETRY")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn evidence(side: &str, now: u64) -> Evidence {
        let call = side == "CALL";
        let (open, close) = if call { (500.1, 500.2) } else { (499.9, 499.8) };
        Evidence {
            symbol: "QQQ".into(),
            source_symbol: "QQQ".into(),
            horizon: "DAY".into(),
            published_ms: now,
            verified: true,
            price: close,
            price_ms: now,
            vwap: Some(500.0),
            bars: (1..=6)
                .rev()
                .map(|n| SessionBar {
                    timestamp: now - 2_000 - n * 120_000,
                    open,
                    high: 500.25,
                    low: 499.75,
                    close,
                    volume: 200.0,
                    minutes_received: 2,
                    vwap: Some(500.0),
                })
                .collect(),
            path: structural_path::Input {
                side: side.into(),
                magnet: Some(500.0),
                wall: Some(if call { 505.0 } else { 495.0 }),
                wall_score: Some(60.0),
                vacuum_low: Some(if call { 501.0 } else { 497.0 }),
                vacuum_high: Some(if call { 503.0 } else { 499.0 }),
                vacuum_confirmed: true,
                flow_transport_ready: true,
                flow_side: side.into(),
                buy_contracts: 5.0,
                rvol_ready: true,
                option_liquidity_ready: true,
                ..Default::default()
            },
        }
    }
    #[test]
    fn recomputes_trigger_and_price_for_both_sides_without_trusting_ready_flags() {
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:02").unwrap();
        for side in ["CALL", "PUT"] {
            let mut e = evidence(side, now);
            let t = prepare(&e, now).unwrap();
            assert_eq!(t.entry, e.price);
            assert_eq!(t.target_1, e.path.wall.unwrap());
            assert!(t.structure.is_some());
            assert!(t.target_2.is_none());
            let mut unsettled = e.clone();
            unsettled.published_ms -= 1;
            unsettled.price_ms -= 1;
            assert_eq!(
                prepare(&unsettled, now - 1).unwrap_err(),
                "MAGNET_2M_TRIGGER"
            );
            e.path.trigger_ready = true;
            e.bars.last_mut().unwrap().minutes_received = 1;
            assert_eq!(prepare(&e, now).unwrap_err(), "MAGNET_2M_TRIGGER");
        }
    }
    #[test]
    fn stale_foreign_future_and_after_hours_evidence_never_prepares() {
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:02").unwrap();
        let original = evidence("CALL", now);
        for kind in 0..8 {
            let mut e = original.clone();
            match kind {
                0 => e.source_symbol = "SPY".into(),
                1 => e.verified = false,
                2 => e.published_ms -= 3_001,
                3 => e.published_ms += 1,
                4 => e.price_ms -= 15_001,
                5 => e.price_ms += 1,
                6 => e.horizon = "NEXT_SESSION".into(),
                _ => e.price = f64::NAN,
            }
            assert!(prepare(&e, now).is_err(), "case {kind}");
        }
        let closed = calendar::legacy_eastern_ms("2026-09-24 16:00:00").unwrap();
        assert!(prepare(&evidence("CALL", closed), closed).is_err());
    }
    #[test]
    fn incompatible_vacuum_and_entry_movement_block_before_storage() {
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:02").unwrap();
        for side in ["CALL", "PUT"] {
            let mut e = evidence(side, now);
            if side == "CALL" {
                e.path.vacuum_low = Some(499.9);
            } else {
                e.path.vacuum_high = Some(500.1);
            }
            assert_eq!(prepare(&e, now).unwrap_err(), "STRUCTURAL_PAPER_GEOMETRY");
            let mut e = evidence(side, now);
            e.price = if side == "CALL" { 502.0 } else { 498.0 };
            assert_eq!(prepare(&e, now).unwrap_err(), "ENTRY_MOVED_FROM_MAGNET");
            e.price = 500.0;
            assert!(prepare(&e, now).is_err());
            let mut e = evidence(side, now);
            e.path.wall = Some(if side == "CALL" { 501.1 } else { 498.9 });
            e.path.vacuum_low = Some(if side == "CALL" { 500.4 } else { 499.2 });
            e.path.vacuum_high = Some(if side == "CALL" { 500.8 } else { 499.6 });
            assert_eq!(prepare(&e, now).unwrap_err(), "STRUCTURAL_PATH_NOT_READY");
        }
    }
}
