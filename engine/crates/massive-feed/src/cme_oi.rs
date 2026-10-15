//! Immutable CME daily observations. Business dates come from STAT, never the
//! local receipt date. Legacy history is not overwritten or assumed comparable.
use crate::cme::Error;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Row {
    pub exchange: String,
    pub segment: String,
    pub id: String,
    pub symbol: String,
    pub series: String,
    pub oi: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub schema: u32,
    pub environment: String,
    pub business_date: String,
    pub rows: Vec<Row>,
}
pub fn business_date(raw: &str) -> Option<String> {
    let normalized = if raw.len() == 8 && raw.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{}", &raw[..4], &raw[4..6], &raw[6..])
    } else {
        raw.to_owned()
    };
    if normalized.len() != 10 || snipers_core::calendar::day_bounds(&normalized).is_none() {
        return None;
    }
    Some(normalized)
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema != 1
            || !["UAT", "PROD"].contains(&self.environment.as_str())
            || business_date(&self.business_date).as_deref() != Some(&self.business_date)
            || self.rows.len() > 50_000
        {
            return Err(Error::Protocol);
        }
        let mut keys = BTreeSet::new();
        for row in &self.rows {
            if [
                &row.exchange,
                &row.segment,
                &row.id,
                &row.symbol,
                &row.series,
            ]
            .iter()
            .any(|v| v.is_empty() || v.len() > 256)
                || !row.oi.is_finite()
                || row.oi < 0.0
                || row.oi.fract() != 0.0
                || row.oi > i32::MAX as f64
                || !keys.insert((&row.exchange, &row.segment, &row.id))
            {
                return Err(Error::Protocol);
            }
        }
        Ok(())
    }
    pub fn save_new(&self, path: &Path) -> Result<(), Error> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Protocol)?;
        if bytes.len() > 20_000_000 {
            return Err(Error::Protocol);
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| Error::Transport)?;
        file.write_all(&bytes).map_err(|_| Error::Transport)?;
        file.sync_all().map_err(|_| Error::Transport)
    }
    pub fn read(path: &Path) -> Result<Self, Error> {
        let file = std::fs::File::open(path).map_err(|_| Error::Transport)?;
        let mut bytes = Vec::new();
        file.take(20_000_001)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Transport)?;
        if bytes.len() > 20_000_000 {
            return Err(Error::Protocol);
        }
        let snapshot: Self = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
        snapshot.validate()?;
        Ok(snapshot)
    }
    /// Same-day or future files cannot become a daily OI baseline. Contract
    /// symbol and series protect against reuse of a security id after expiry.
    pub fn change(
        &self,
        current: &Row,
        current_date: &str,
        environment: &str,
    ) -> Result<Option<f64>, Error> {
        self.validate()?;
        if self.environment != environment {
            return Ok(None);
        }
        if business_date(current_date).as_deref() != Some(current_date) {
            return Err(Error::Protocol);
        }
        if self.business_date.as_str() >= current_date {
            return Ok(None);
        }
        if !current.oi.is_finite() || current.oi < 0.0 || current.oi.fract() != 0.0 {
            return Err(Error::Protocol);
        }
        Ok(self
            .rows
            .iter()
            .find(|old| {
                old.exchange == current.exchange
                    && old.segment == current.segment
                    && old.id == current.id
                    && old.symbol == current.symbol
                    && old.series == current.series
            })
            .map(|old| current.oi - old.oi))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn row() -> Row {
        Row {
            exchange: "XCME".into(),
            segment: "1".into(),
            id: "123".into(),
            symbol: "NQZ6 C21000".into(),
            series: "ABC123".into(),
            oi: 100.0,
        }
    }
    #[test]
    fn daily_baseline_does_not_compare_same_day_or_reused_ids() {
        let snapshot = Snapshot {
            schema: 1,
            environment: "UAT".into(),
            business_date: "2026-09-23".into(),
            rows: vec![row()],
        };
        let mut current = row();
        current.oi = 120.0;
        assert_eq!(
            snapshot.change(&current, "2026-09-24", "UAT").unwrap(),
            Some(20.0)
        );
        assert_eq!(
            snapshot.change(&current, "2026-09-23", "UAT").unwrap(),
            None
        );
        assert_eq!(
            snapshot.change(&current, "2026-09-24", "PROD").unwrap(),
            None
        );
        current.series = "NEW".into();
        assert_eq!(
            snapshot.change(&current, "2026-09-24", "UAT").unwrap(),
            None
        );
        assert_eq!(business_date("20260923"), Some("2026-09-23".into()));
        assert_eq!(business_date("20260230"), None);
    }
    #[test]
    fn immutable_history_survives_restart_and_corruption_is_explicit() {
        let path = std::env::temp_dir().join(format!(
            "snipers-cme-oi-{}-{}.json",
            std::process::id(),
            crate::live::now_ms()
        ));
        let snapshot = Snapshot {
            schema: 1,
            environment: "UAT".into(),
            business_date: "2026-09-23".into(),
            rows: vec![row()],
        };
        snapshot.save_new(&path).unwrap();
        assert!(snapshot.save_new(&path).is_err());
        assert_eq!(Snapshot::read(&path).unwrap(), snapshot);
        std::fs::write(&path, b"{broken").unwrap();
        assert!(Snapshot::read(&path).is_err());
        std::fs::remove_file(&path).unwrap();
        let mut duplicate = snapshot;
        duplicate.rows.push(row());
        assert!(duplicate.validate().is_err());
    }
}
