//! Joins references, dated STAT and prior OI; missing inputs cannot certify levels.
use crate::{
    cme::Error,
    cme_cache::Cache,
    cme_data::Observation,
    cme_oi::{business_date, Row, Snapshot},
    cme_reference::{OptionMetadata, ReferenceMap},
};
use snipers_core::legacy_levels::StrikeData;

pub struct InputRow {
    pub metadata: OptionMetadata,
    pub open_interest: f64,
    pub oi_change: Option<f64>,
    pub volume: Option<f64>,
}
pub struct Inputs {
    pub rows: Vec<InputRow>,
    pub expected: usize,
    pub snapshot: Snapshot,
}
impl Inputs {
    pub fn legacy_rows(&self) -> Result<Vec<StrikeData>, Error> {
        if self.expected == 0 || self.rows.len() != self.expected {
            return Err(Error::Protocol);
        }
        self.rows
            .iter()
            .map(|row| {
                Ok(StrikeData {
                    strike: row.metadata.strike,
                    option_type: row.metadata.side.clone(),
                    open_interest: row.open_interest,
                    oi_change: row.oi_change.ok_or(Error::Protocol)?,
                    volume: row.volume.ok_or(Error::Protocol)?,
                    most_active: false,
                })
            })
            .collect()
    }
}
pub fn build(
    cache: &Cache,
    references: &ReferenceMap,
    date: &str,
    environment: &str,
    previous: Option<&Snapshot>,
) -> Result<Inputs, Error> {
    if business_date(date).as_deref() != Some(date) || !["UAT", "PROD"].contains(&environment) {
        return Err(Error::Configuration);
    }
    if let Some(snapshot) = previous {
        snapshot.validate()?;
    }
    let baseline: std::collections::BTreeMap<_, _> = previous
        .filter(|s| s.environment == environment && s.business_date.as_str() < date)
        .into_iter()
        .flat_map(|s| s.rows.iter())
        .map(|row| {
            (
                (
                    row.exchange.as_str(),
                    row.segment.as_str(),
                    row.id.as_str(),
                    row.symbol.as_str(),
                    row.series.as_str(),
                ),
                row.oi,
            )
        })
        .collect();
    let mut result = Inputs {
        rows: Vec::new(),
        expected: references.len(),
        snapshot: Snapshot {
            schema: 1,
            environment: environment.into(),
            business_date: date.into(),
            rows: Vec::new(),
        },
    };
    for state in cache.states() {
        let Some(Observation::Statistics {
            instrument,
            open_interest: Some(oi),
            cleared_volume,
            raw_statistics,
            ..
        }) = &state.statistics
        else {
            continue;
        };
        let Some(metadata) = references.lookup(instrument) else {
            continue;
        };
        if raw_statistics["openInterestDate"]
            .as_str()
            .and_then(business_date)
            .as_deref()
            != Some(date)
        {
            continue;
        }
        let current = Row {
            exchange: instrument.exchange.clone(),
            segment: instrument.segment.clone(),
            id: instrument.id.clone(),
            symbol: instrument.symbol.clone(),
            series: metadata.series_guid.clone(),
            oi: *oi,
        };
        let change = baseline
            .get(&(
                current.exchange.as_str(),
                current.segment.as_str(),
                current.id.as_str(),
                current.symbol.as_str(),
                current.series.as_str(),
            ))
            .map(|old| current.oi - old);
        let volume = if raw_statistics["clearedVolumeDate"]
            .as_str()
            .and_then(business_date)
            .as_deref()
            == Some(date)
        {
            *cleared_volume
        } else {
            None
        };
        result.rows.push(InputRow {
            metadata: metadata.clone(),
            open_interest: *oi,
            oi_change: change,
            volume,
        });
        result.snapshot.rows.push(current);
    }
    result.snapshot.validate()?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn offline_reference_stat_history_to_legacy_rows_preserves_unknowns() {
        let mut references = ReferenceMap::default();
        references.replace(&[json!({"instrumentName":"NQZ6 21000 Call","globexSymbol":"NQZ6 C21000","globexStrikePx":"21000","globexSecurityId":"123","exchangeGlobex":"XCME","seriesGuid":"ABC123","lastTradeDate":"2026-12-18"})]).unwrap();
        let mut cache = Cache::default();
        let message = json!({"header":{"messageType":"STAT","sequenceNumber":"10"},"payload":[{"instrument":{"id":"123","exchangeMic":"XCME","marketSegmentId":"1","symbol":"NQZ6 C21000","productCode":"NQ","productType":"OOF"},"tradeStatistics":{"openInterest":"120","openInterestDate":"20260924","clearedVolume":"80","clearedVolumeDate":"20260924"}}]});
        cache.apply(&message).unwrap();
        let current = build(&cache, &references, "2026-09-24", "UAT", None).unwrap();
        assert_eq!(current.snapshot.rows.len(), 1);
        assert!(current.legacy_rows().is_err());
        let mut prior = current.snapshot;
        prior.business_date = "2026-09-23".into();
        prior.rows[0].oi = 100.0;
        let inputs = build(&cache, &references, "2026-09-24", "UAT", Some(&prior)).unwrap();
        let rows = inputs.legacy_rows().unwrap();
        assert_eq!(rows[0].oi_change, 20.0);
        assert_eq!(rows[0].volume, 80.0);
        assert!(
            build(&cache, &references, "2026-09-24", "PROD", Some(&prior))
                .unwrap()
                .legacy_rows()
                .is_err()
        );
        assert!(
            build(&cache, &references, "2026-09-25", "UAT", Some(&prior))
                .unwrap()
                .legacy_rows()
                .is_err()
        );
        cache.invalidate();
        assert!(
            build(&cache, &references, "2026-09-24", "UAT", Some(&prior))
                .unwrap()
                .legacy_rows()
                .is_err()
        );
    }
}
