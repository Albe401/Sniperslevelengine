//! Closed simulated trades only; all results are underlying-price points.
use serde::{Deserialize, Serialize};
#[derive(Clone, Deserialize)]
pub struct ClosedTrade {
    pub pnl_points: f64,
    pub mfe: f64,
    pub mae: f64,
    pub target_1_hit: bool,
    pub target_2_hit: bool,
    pub stop_hit: bool,
}
#[derive(Default, Debug, Serialize, PartialEq)]
pub struct Performance {
    pub total_trades: u64,
    pub wins: u64,
    pub losses: u64,
    pub win_rate: f64,
    pub average_pnl_points: f64,
    pub average_mfe: f64,
    pub average_mae: f64,
    pub target_1_hit_rate: f64,
    pub target_2_hit_rate: f64,
    pub stop_rate: f64,
}
#[derive(Default)]
pub struct Accumulator {
    report: Performance,
    pnl: f64,
    mfe: f64,
    mae: f64,
    hits: [u64; 3],
}
#[derive(Debug)]
pub struct InvalidTrade;
impl Accumulator {
    /// Reject malformed records atomically instead of treating missing results as zero.
    pub fn push(&mut self, row: &ClosedTrade) -> Result<(), InvalidTrade> {
        let totals = [
            self.pnl + row.pnl_points,
            self.mfe + row.mfe,
            self.mae + row.mae,
        ];
        if !totals.iter().all(|v| v.is_finite()) || row.mfe < 0.0 || row.mae > 0.0 {
            return Err(InvalidTrade);
        }
        [self.pnl, self.mfe, self.mae] = totals;
        self.report.total_trades += 1;
        self.report.wins += u64::from(row.pnl_points > 0.0);
        self.report.losses += u64::from(row.pnl_points < 0.0);
        for (count, hit) in
            self.hits
                .iter_mut()
                .zip([row.target_1_hit, row.target_2_hit, row.stop_hit])
        {
            *count += u64::from(hit);
        }
        Ok(())
    }
    pub fn finish(mut self) -> Performance {
        let count = self.report.total_trades as f64;
        if count > 0.0 {
            let round2 = |v: f64| format!("{v:.2}").parse().unwrap_or(v);
            let round4 = |v: f64| format!("{v:.4}").parse().unwrap_or(v);
            self.report.win_rate = round2(self.report.wins as f64 / count * 100.0);
            self.report.average_pnl_points = round4(self.pnl / count);
            self.report.average_mfe = round4(self.mfe / count);
            self.report.average_mae = round4(self.mae / count);
            self.report.target_1_hit_rate = round2(self.hits[0] as f64 / count * 100.0);
            self.report.target_2_hit_rate = round2(self.hits[1] as f64 / count * 100.0);
            self.report.stop_rate = round2(self.hits[2] as f64 / count * 100.0);
        }
        self.report
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_statistics_preserve_breakeven_denominator_and_reject_invalid_rows() {
        let data: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/legacy-paper-performance.json"
        ))
        .unwrap();
        for case in data["cases"].as_array().unwrap() {
            let mut accumulator = Accumulator::default();
            for row in case["rows"].as_array().unwrap() {
                accumulator
                    .push(&serde_json::from_value(row.clone()).unwrap())
                    .unwrap();
            }
            assert_eq!(
                serde_json::to_value(accumulator.finish()).unwrap(),
                case["expected"]
            );
        }
        let mut accumulator = Accumulator::default();
        assert!(accumulator
            .push(&ClosedTrade {
                pnl_points: f64::NAN,
                mfe: 0.0,
                mae: 0.0,
                target_1_hit: false,
                target_2_hit: false,
                stop_hit: false
            })
            .is_err());
        assert_eq!(accumulator.finish(), Performance::default());
    }
}
