//! Session-scoped CME cache. It is not a certification of live data or licensing.
use crate::{
    cme::Error,
    cme_data::{normalize, Instrument, Observation},
};
use serde_json::Value;
use std::collections::BTreeMap;
type Key = (String, String, String);

#[derive(Default)]
pub struct InstrumentState {
    pub trade: Option<Observation>,
    pub book: Option<Observation>,
    pub statistics: Option<Observation>,
}
pub struct Cache {
    instruments: BTreeMap<Key, InstrumentState>,
    sequences: BTreeMap<String, u64>,
    usable: bool,
    capacity: usize,
}
impl Default for Cache {
    fn default() -> Self {
        Self::new(10_000)
    }
}
impl Cache {
    pub fn states(&self) -> impl Iterator<Item = &InstrumentState> {
        self.instruments.values()
    }
    pub fn new(capacity: usize) -> Self {
        Self {
            instruments: BTreeMap::new(),
            sequences: BTreeMap::new(),
            usable: true,
            capacity: capacity.min(50_000),
        }
    }
    pub fn get(&self, exchange: &str, segment: &str, id: &str) -> Option<&InstrumentState> {
        self.instruments
            .get(&(exchange.into(), segment.into(), id.into()))
    }
    pub fn len(&self) -> usize {
        self.instruments.len()
    }
    pub fn is_empty(&self) -> bool {
        self.instruments.is_empty()
    }
    /// Called on disconnect, overflow, or malformed/gapped input. Previous data
    /// cannot survive as apparently current after recovery.
    pub fn invalidate(&mut self) {
        self.instruments.clear();
        self.sequences.clear();
        self.usable = false;
    }
    pub fn begin_session(&mut self) {
        self.invalidate();
        self.usable = true;
    }
    pub fn apply(&mut self, message: &Value) -> Result<usize, Error> {
        let result = self.apply_inner(message);
        if result.is_err() {
            self.invalidate();
        }
        result
    }
    fn apply_inner(&mut self, message: &Value) -> Result<usize, Error> {
        if !self.usable {
            return Err(Error::Transport);
        }
        let observations = normalize(message)?;
        let topic = message["header"]["messageType"]
            .as_str()
            .ok_or(Error::Protocol)?;
        if !["TRD", "TOB", "STAT"].contains(&topic) {
            return Ok(0);
        }
        let raw = &message["header"]["sequenceNumber"];
        let seq = raw
            .as_u64()
            .or_else(|| raw.as_str()?.parse().ok())
            .ok_or(Error::Protocol)?;
        if self
            .sequences
            .get(topic)
            .is_some_and(|last| last.checked_add(1) != Some(seq))
        {
            return Err(Error::Protocol);
        }
        let mut additional = BTreeMap::new();
        for observation in &observations {
            let instrument = identity(observation);
            let key = key(instrument);
            if additional
                .get(&key)
                .is_some_and(|previous| *previous != instrument)
            {
                return Err(Error::Protocol);
            }
            if let Some(existing) = self.instruments.get(&key) {
                let previous = existing
                    .trade
                    .as_ref()
                    .or(existing.book.as_ref())
                    .or(existing.statistics.as_ref());
                if previous.is_some_and(|p| identity(p) != instrument) {
                    return Err(Error::Protocol);
                }
            } else {
                additional.insert(key, instrument);
            }
        }
        if self.instruments.len() + additional.len() > self.capacity {
            return Err(Error::Protocol);
        }
        let count = observations.len();
        for observation in observations {
            let state = self
                .instruments
                .entry(key(identity(&observation)))
                .or_default();
            match observation {
                Observation::Trade { .. } => state.trade = Some(observation),
                Observation::Book { .. } => state.book = Some(observation),
                Observation::Statistics { .. } => state.statistics = Some(observation),
            }
        }
        self.sequences.insert(topic.to_owned(), seq);
        Ok(count)
    }
}
fn identity(observation: &Observation) -> &Instrument {
    match observation {
        Observation::Trade { instrument, .. }
        | Observation::Book { instrument, .. }
        | Observation::Statistics { instrument, .. } => instrument,
    }
}
fn key(instrument: &Instrument) -> Key {
    (
        instrument.exchange.clone(),
        instrument.segment.clone(),
        instrument.id.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn book(seq: u64, id: &str) -> Value {
        json!({"header":{"messageType":"TOB","sequenceNumber":seq.to_string()},"payload":[{"instrument":{"id":id,"exchangeMic":"XCME","marketSegmentId":"1","symbol":"NQZ6","productCode":"NQ","productType":"FUT"},"bidLevel":[{"price":"100","qty":"2","lastUpdateTime":"2026-09-24T14:00:00Z"}],"askLevel":[]}]})
    }
    #[test]
    fn books_replace_and_disconnect_or_sequence_gaps_require_new_session() {
        let mut cache = Cache::default();
        cache.apply(&book(10, "1")).unwrap();
        let mut clear = book(11, "1");
        clear["payload"][0]["bidLevel"] = json!([]);
        cache.apply(&clear).unwrap();
        assert!(matches!(
            cache.get("XCME", "1", "1").unwrap().book,
            Some(Observation::Book {
                bid: None,
                ask: None,
                ..
            })
        ));
        assert!(cache.apply(&book(13, "1")).is_err());
        assert!(cache.is_empty());
        assert!(cache.apply(&book(14, "1")).is_err());
        cache.begin_session();
        cache.apply(&book(1, "1")).unwrap();
        cache.invalidate();
        assert!(cache.is_empty());
    }
    #[test]
    fn capacity_and_identity_conflicts_do_not_publish_partial_batches() {
        let mut cache = Cache::new(1);
        cache.apply(&book(1, "1")).unwrap();
        assert!(cache.apply(&book(2, "2")).is_err());
        assert!(cache.is_empty());
        cache.begin_session();
        cache.apply(&book(1, "1")).unwrap();
        let mut foreign = book(2, "1");
        foreign["payload"][0]["instrument"]["symbol"] = json!("DIFFERENT");
        assert!(cache.apply(&foreign).is_err());
        assert!(cache.is_empty());
        cache.begin_session();
        let mut conflict = book(1, "1");
        let mut other = conflict["payload"][0].clone();
        other["instrument"]["symbol"] = json!("OTHER");
        conflict["payload"].as_array_mut().unwrap().push(other);
        assert!(cache.apply(&conflict).is_err());
        assert!(cache.is_empty());
    }
}
