//! Restore an immutable legacy OPEN record. Restoration itself never observes a
//! price, closes a position, or resets its extrema and trailing state.
use crate::paper::{Error, Side, Stage, Structure, Trade};
use serde_json::Value;

pub fn restore(record: &Value, transferred_ms: u64) -> Result<Trade, Error> {
    let bad = || Error::InvalidData;
    let number = |key: &str| {
        record[key]
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(bad)
    };
    let flag = |key: &str| match &record[key] {
        Value::Bool(b) => Ok(*b),
        v if v.as_i64() == Some(0) => Ok(false),
        v if v.as_i64() == Some(1) => Ok(true),
        _ => Err(bad()),
    };
    if record["status"] != "OPEN" || record["id"].as_i64().is_none_or(|id| id <= 0) {
        return Err(bad());
    }
    let opened = record["opened_at"]
        .as_str()
        .and_then(crate::calendar::legacy_eastern_ms)
        .ok_or_else(bad)?;
    if transferred_ms < opened || crate::calendar::date_at(transferred_ms).is_none() {
        return Err(bad());
    }
    let side = match record["side"].as_str() {
        Some("CALL") => Side::CALL,
        Some("PUT") => Side::PUT,
        _ => return Err(bad()),
    };
    let structural = flag("structural_trailing_enabled")?;
    let structure = if structural {
        Some(Structure {
            magnet: number("structural_main_magnet")?,
            vacuum_low: number("structural_vacuum_low")?,
            vacuum_high: number("structural_vacuum_high")?,
            target_wall: number("structural_target_wall")?,
        })
    } else {
        None
    };
    let stage = match (structural, record["trailing_stage"].as_str()) {
        (false, Some("OFF")) | (true, Some("INITIAL")) => Stage::Initial,
        (true, Some("MAGNET_LOCK")) => Stage::MagnetLock,
        (true, Some("VACUUM_LOCK")) => Stage::VacuumLock,
        _ => return Err(bad()),
    };
    let target_2 = if record["target_2"].is_null() {
        None
    } else {
        Some(number("target_2")?)
    };
    let initial_stop = if record["initial_stop_price"].is_null() {
        number("stop_price")?
    } else {
        number("initial_stop_price")?
    };
    let mut trade = Trade::new(
        side,
        number("entry_price")?,
        initial_stop,
        number("target_1")?,
        target_2,
        structure,
        opened,
    )?;
    trade.current = number("current_price")?;
    trade.stop = number("stop_price")?;
    trade.highest = number("highest_price")?;
    trade.lowest = number("lowest_price")?;
    trade.mfe = number("mfe")?;
    trade.mae = number("mae")?;
    trade.target_1_hit = flag("target_1_hit")?;
    trade.target_2_hit = flag("target_2_hit")?;
    trade.stage = stage;
    trade.clear_count = record["vacuum_clear_count"].as_u64().ok_or_else(bad)?;
    // A frozen snapshot has no trustworthy last-trade clock. Require the next
    // observation to be newer than this explicit transfer boundary.
    trade.last_ms = Some(transferred_ms);
    // OPEN legacy rows often store NULL realized PnL; original fields remain in
    // the archive. The resumed model uses marked underlying-price points.
    trade.pnl_points = if side == Side::CALL {
        trade.current - trade.entry
    } else {
        trade.entry - trade.current
    };
    trade.pnl_percent = trade.pnl_points / trade.entry * 100.0;
    if !trade.valid()
        || !trade.pnl_percent.is_finite()
        || trade.mfe < 0.0
        || trade.mae > 0.0
        || (side == Side::CALL && trade.stop < initial_stop)
        || (side == Side::PUT && trade.stop > initial_stop)
        || (!structural && trade.clear_count != 0)
    {
        return Err(Error::InvalidGeometry);
    }
    // The handoff breaks observation continuity even if the saved counter was
    // nonzero. Keep the achieved stop/stage, but rebuild sustained clearance.
    trade.clear_count = 0;
    Ok(trade)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record() -> Value {
        serde_json::json!({"id":12,"status":"OPEN","opened_at":"2026-09-18 11:16:23","symbol":"MSFT","side":"CALL","entry_price":493.5687,"current_price":500.5301,"stop_price":490.0,"initial_stop_price":490.0,"target_1":500.0,"target_2":510.0,"highest_price":502.0,"lowest_price":492.0,"mfe":8.4313,"mae":-1.5687,"target_1_hit":1,"target_2_hit":0,"structural_trailing_enabled":0,"trailing_stage":"OFF","vacuum_clear_count":0,"pnl_points":null})
    }
    #[test]
    fn restores_without_closing_or_erasing_history_then_requires_a_new_tick() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let old = record();
        let mut trade = restore(&old, now).unwrap();
        assert_eq!(trade.highest, 502.0);
        assert!(trade.target_1_hit);
        assert_eq!(trade.closed_ms, None);
        assert_eq!(trade.observe(511.0, now, now, true), Err(Error::OutOfOrder));
        assert_eq!(
            trade.observe(511.0, now + 1, now + 1, false),
            Err(Error::InvalidData)
        );
        trade.observe(511.0, now + 1, now + 1, true).unwrap();
        assert_eq!(trade.closed_ms, Some(now + 1));
        assert_eq!(trade.exit_reason.as_deref(), Some("TARGET 2 HIT"));
        assert!(old["pnl_points"].is_null());
    }
    #[test]
    fn refuses_unknown_flags_bad_extrema_and_incomplete_structural_state() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        for (key, value) in [
            ("highest_price", serde_json::json!(499)),
            ("target_1_hit", serde_json::json!(2)),
            ("structural_trailing_enabled", serde_json::json!(1)),
            ("trailing_stage", serde_json::json!("UNKNOWN")),
            ("status", serde_json::json!("CLOSED")),
        ] {
            let mut old = record();
            old[key] = value;
            assert!(restore(&old, now).is_err(), "{key}");
        }
    }
    #[test]
    fn structural_transfer_keeps_locked_stop_but_restarts_continuity() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let old = serde_json::json!({"id":1,"status":"OPEN","opened_at":"2026-09-23 10:00:00","side":"CALL","entry_price":100,"current_price":105,"stop_price":101,"initial_stop_price":98,"target_1":107,"target_2":110,"highest_price":106,"lowest_price":99,"mfe":6,"mae":-1,"target_1_hit":0,"target_2_hit":0,"structural_trailing_enabled":1,"trailing_stage":"MAGNET_LOCK","vacuum_clear_count":1,"structural_main_magnet":101,"structural_vacuum_low":102,"structural_vacuum_high":104,"structural_target_wall":110});
        let mut t = restore(&old, now).unwrap();
        assert_eq!(t.stop, 101.);
        assert_eq!(t.stage, Stage::MagnetLock);
        assert_eq!(t.clear_count, 0);
        t.observe(105., now + 1, now + 1, true).unwrap();
        assert_eq!(t.stop, 101.);
        t.observe(105., now + 2, now + 2, true).unwrap();
        assert_eq!(t.stop, 102.);
    }
}
