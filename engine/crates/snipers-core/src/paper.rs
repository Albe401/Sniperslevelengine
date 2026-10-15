//! Deterministic simulation in underlying-price points, never option execution.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Side {
    CALL,
    PUT,
}
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Stage {
    Initial,
    MagnetLock,
    VacuumLock,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Structure {
    pub magnet: f64,
    pub vacuum_low: f64,
    pub vacuum_high: f64,
    pub target_wall: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    pub side: Side,
    pub entry: f64,
    pub current: f64,
    pub stop: f64,
    pub target_1: f64,
    pub target_2: Option<f64>,
    pub highest: f64,
    pub lowest: f64,
    pub mfe: f64,
    pub mae: f64,
    pub pnl_points: f64,
    pub pnl_percent: f64,
    pub structure: Option<Structure>,
    pub stage: Stage,
    pub clear_count: u64,
    pub target_1_hit: bool,
    pub target_2_hit: bool,
    pub stop_hit: bool,
    pub opened_ms: u64,
    pub last_ms: Option<u64>,
    pub closed_ms: Option<u64>,
    pub exit_reason: Option<String>,
}
#[derive(Debug, PartialEq)]
pub enum Error {
    InvalidGeometry,
    InvalidData,
    OutOfOrder,
    AlreadyClosed,
}
impl Trade {
    pub fn new(
        side: Side,
        entry: f64,
        stop: f64,
        target_1: f64,
        target_2: Option<f64>,
        structure: Option<Structure>,
        opened_ms: u64,
    ) -> Result<Self, Error> {
        let t = Self {
            side,
            entry,
            current: entry,
            stop,
            target_1,
            target_2,
            highest: entry,
            lowest: entry,
            mfe: 0.0,
            mae: 0.0,
            pnl_points: 0.0,
            pnl_percent: 0.0,
            structure,
            stage: Stage::Initial,
            clear_count: 0,
            target_1_hit: false,
            target_2_hit: false,
            stop_hit: false,
            opened_ms,
            last_ms: None,
            closed_ms: None,
            exit_reason: None,
        };
        let call = side == Side::CALL;
        if !t.valid()
            || if call {
                stop >= entry || target_1 <= entry || target_2.is_some_and(|p| p < target_1)
            } else {
                stop <= entry || target_1 >= entry || target_2.is_some_and(|p| p > target_1)
            }
        {
            return Err(Error::InvalidGeometry);
        }
        Ok(t)
    }
    pub(crate) fn valid(&self) -> bool {
        let positive = |p: f64| p.is_finite() && p > 0.0;
        [
            self.entry,
            self.current,
            self.stop,
            self.target_1,
            self.highest,
            self.lowest,
        ]
        .into_iter()
        .all(positive)
            && self.highest >= self.entry
            && self.lowest <= self.entry
            && self.current >= self.lowest
            && self.current <= self.highest
            && self.target_2.is_none_or(positive)
            && self.structure.as_ref().is_none_or(|s| {
                [s.magnet, s.vacuum_low, s.vacuum_high, s.target_wall]
                    .into_iter()
                    .all(positive)
                    && if self.side == Side::CALL {
                        s.magnet < s.vacuum_low
                            && s.vacuum_low < s.vacuum_high
                            && s.vacuum_high < s.target_wall
                            && s.target_wall > self.entry
                    } else {
                        s.target_wall < s.vacuum_low
                            && s.vacuum_low < s.vacuum_high
                            && s.vacuum_high < s.magnet
                            && s.target_wall < self.entry
                    }
            })
    }
    /// Ignore replayed observations rather than counting them as sustained vacuum
    /// clearance. A price must be verified, from today's feed and at most 15s old.
    /// An existing position survives the session boundary, as in the legacy runtime.
    pub fn observe(
        &mut self,
        price: f64,
        event_ms: u64,
        now: u64,
        verified: bool,
    ) -> Result<(), Error> {
        if self.closed_ms.is_some() {
            return Err(Error::AlreadyClosed);
        }
        if !self.valid() {
            return Err(Error::InvalidGeometry);
        }
        if !verified
            || !price.is_finite()
            || price <= 0.0
            || event_ms > now
            || now - event_ms > 15_000
            || crate::calendar::date_at(event_ms).is_none()
            || crate::calendar::date_at(event_ms) != crate::calendar::date_at(now)
        {
            return Err(Error::InvalidData);
        }
        if event_ms < self.opened_ms || self.last_ms.is_some_and(|t| event_ms <= t) {
            return Err(Error::OutOfOrder);
        }
        let mut next = self.clone();
        if event_ms.saturating_sub(self.last_ms.unwrap_or(self.opened_ms)) > 15_000 {
            // A disconnected/overnight gap cannot establish sustained clearance.
            next.clear_count = 0;
        }
        next.current = price;
        next.last_ms = Some(event_ms);
        next.highest = next.highest.max(price);
        next.lowest = next.lowest.min(price);
        let call = next.side == Side::CALL;
        next.mfe = round4(if call {
            next.highest - next.entry
        } else {
            next.entry - next.lowest
        });
        next.mae = round4(if call {
            next.lowest - next.entry
        } else {
            next.entry - next.highest
        });
        let mut wall_hit = false;
        if let Some(s) = &next.structure {
            let cleared = if call {
                price >= s.vacuum_high
            } else {
                price <= s.vacuum_low
            };
            next.clear_count = if cleared {
                next.clear_count.saturating_add(1)
            } else {
                0
            };
            wall_hit = if call {
                price >= s.target_wall
            } else {
                price <= s.target_wall
            };
            if next.clear_count >= 1 {
                next.stop = if call {
                    next.stop.max(s.magnet)
                } else {
                    next.stop.min(s.magnet)
                };
                if next.stage < Stage::MagnetLock {
                    next.stage = Stage::MagnetLock;
                }
            }
            if next.clear_count >= 2 {
                next.stop = if call {
                    next.stop.max(s.vacuum_low)
                } else {
                    next.stop.min(s.vacuum_high)
                };
                next.stage = Stage::VacuumLock;
            }
        }
        next.target_1_hit |= if call {
            price >= next.target_1
        } else {
            price <= next.target_1
        };
        next.target_2_hit |= next
            .target_2
            .is_some_and(|t| if call { price >= t } else { price <= t });
        next.stop_hit = if call {
            price <= next.stop
        } else {
            price >= next.stop
        };
        let reason = if next.stop_hit {
            Some(if next.structure.is_some() {
                "STRUCTURAL TRAILING STOP HIT"
            } else {
                "STOP HIT"
            })
        } else if wall_hit {
            Some("STRUCTURAL TARGET WALL HIT")
        } else if next.target_2_hit {
            Some("TARGET 2 HIT")
        } else {
            None
        };
        let points = if call {
            price - next.entry
        } else {
            next.entry - price
        };
        let percent = points / next.entry * 100.0;
        if !percent.is_finite() {
            return Err(Error::InvalidData);
        }
        next.pnl_points = round4(points);
        next.pnl_percent = round4(percent);
        if let Some(reason) = reason {
            next.closed_ms = Some(event_ms);
            next.exit_reason = Some(reason.into());
        }
        *self = next;
        Ok(())
    }
}
fn round4(v: f64) -> f64 {
    format!("{v:.4}").parse().unwrap_or(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overnight_position_is_preserved_and_resumes_only_on_a_current_verified_price() {
        let opened = crate::calendar::legacy_eastern_ms("2026-09-24 15:59:00").unwrap();
        let next_day = crate::calendar::legacy_eastern_ms("2026-09-25 09:30:01").unwrap();
        let mut trade = Trade::new(
            Side::CALL,
            100.0,
            98.0,
            109.0,
            None,
            Some(Structure {
                magnet: 101.0,
                vacuum_low: 102.0,
                vacuum_high: 104.0,
                target_wall: 109.0,
            }),
            opened,
        )
        .unwrap();
        trade.observe(104.0, opened + 1, opened + 1, true).unwrap();
        assert_eq!(trade.clear_count, 1);
        assert!(trade.observe(104.0, opened + 1, next_day, true).is_err());
        trade.observe(104.0, next_day, next_day, true).unwrap();
        assert_eq!(trade.clear_count, 1);
        assert_eq!(trade.stop, 101.0);
        assert!(trade.closed_ms.is_none());
        trade
            .observe(100.5, next_day + 1, next_day + 1, true)
            .unwrap();
        assert_eq!(trade.closed_ms, Some(next_day + 1));
        assert_eq!(trade.pnl_points, 0.5);
    }
    #[test]
    fn twelve_legacy_trailing_trajectories_match_both_sides() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-paper-trailing.json"))
                .unwrap();
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        for case in fixtures["cases"].as_array().unwrap() {
            let side: Side = serde_json::from_value(case["side"].clone()).unwrap();
            let structure: Structure = serde_json::from_value(case["structure"].clone()).unwrap();
            let mut trade = Trade::new(
                side,
                case["entry"].as_f64().unwrap(),
                case["stop"].as_f64().unwrap(),
                case["target1"].as_f64().unwrap(),
                case["target2"].as_f64(),
                Some(structure),
                now,
            )
            .unwrap();
            for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
                trade
                    .observe(
                        step["price"].as_f64().unwrap(),
                        now + index as u64,
                        now + index as u64,
                        true,
                    )
                    .unwrap();
                assert_eq!(trade.stop, step["stop"].as_f64().unwrap());
                assert_eq!(serde_json::to_value(trade.stage).unwrap(), step["stage"]);
                assert_eq!(trade.clear_count, step["clearCount"].as_u64().unwrap());
                if step["wallHit"] == true {
                    assert!(trade.closed_ms.is_some());
                }
            }
        }
    }
    #[test]
    fn replay_cannot_advance_trailing_and_stop_never_retreats() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut t = Trade::new(
            Side::CALL,
            100.0,
            98.0,
            105.0,
            Some(110.0),
            Some(Structure {
                magnet: 101.0,
                vacuum_low: 102.0,
                vacuum_high: 104.0,
                target_wall: 109.0,
            }),
            now,
        )
        .unwrap();
        t.observe(104.0, now, now, true).unwrap();
        assert_eq!(t.stop, 101.0);
        assert_eq!(t.stage, Stage::MagnetLock);
        assert_eq!(t.observe(104.0, now, now, true), Err(Error::OutOfOrder));
        assert_eq!(t.clear_count, 1);
        t.observe(104.0, now + 1, now + 1, true).unwrap();
        assert_eq!(t.stop, 102.0);
        assert_eq!(t.stage, Stage::VacuumLock);
        t.observe(103.0, now + 2, now + 2, true).unwrap();
        assert_eq!(t.stop, 102.0);
        assert_eq!(t.clear_count, 0);
        t.observe(101.5, now + 3, now + 3, true).unwrap();
        assert!(t.stop_hit);
        assert_eq!(t.pnl_points, 1.5);
        assert_eq!(t.mfe, 4.0);
        assert_eq!(
            t.observe(109.0, now + 4, now + 4, true),
            Err(Error::AlreadyClosed)
        );
    }
    #[test]
    fn put_targets_and_invalid_data_leave_state_consistent() {
        let now = crate::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        assert!(Trade::new(
            Side::CALL,
            100.0,
            98.0,
            105.0,
            None,
            Some(Structure {
                magnet: 103.0,
                vacuum_low: 102.0,
                vacuum_high: 104.0,
                target_wall: 109.0
            }),
            now
        )
        .is_err());
        let mut t = Trade::new(Side::PUT, 100.0, 102.0, 98.0, Some(95.0), None, now).unwrap();
        for (p, event, current, verified) in [
            (f64::NAN, now, now, true),
            (99.0, now + 1, now, true),
            (99.0, now, now + 15001, true),
            (99.0, now, now, false),
        ] {
            assert_eq!(
                t.observe(p, event, current, verified),
                Err(Error::InvalidData)
            );
            assert_eq!(t.current, 100.0);
        }
        t.observe(98.0, now, now, true).unwrap();
        assert!(t.target_1_hit);
        assert!(t.closed_ms.is_none());
        t.observe(94.0, now + 1, now + 1, true).unwrap();
        assert_eq!(t.exit_reason.as_deref(), Some("TARGET 2 HIT"));
        assert_eq!(t.pnl_points, 6.0);
        assert_eq!(t.pnl_percent, 6.0);
    }
}
