//! Bounded diagnostics. Sequence gaps are not evidence of missing market events.
use crate::{Event, Market, TradeMetadata};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

const IDENTITIES: usize = 65_536;
const SAMPLES: usize = 4096;
const RECORDS: usize = 10_000;
type Identity = (bool, String, String, u64);
type Fingerprint = (u64, u64, u64, TradeMetadata);

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quality {
    pub duplicates: u64,
    pub sequence_conflicts: u64,
    pub unidentified_trades: u64,
    pub future_events: u64,
    pub out_of_order: u64,
    pub identity_evictions: u64,
    pub latency_samples: usize,
    pub p50_ms: Option<u64>,
    pub p95_ms: Option<u64>,
    pub p99_ms: Option<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub market: Market,
    pub received_ms: u64,
    pub event: Event,
}
#[derive(Clone, Default)]
pub struct Telemetry {
    identities: BTreeMap<Identity, Fingerprint>,
    order: VecDeque<Identity>,
    latest: BTreeMap<(bool, String, char), u64>,
    latest_sequence: BTreeMap<(bool, String, String), u64>,
    quality: [Quality; 2],
    delays: [VecDeque<u64>; 2],
    pub records: VecDeque<Record>,
    pub overwritten_records: u64,
}
impl Telemetry {
    /// Called only for validated, selected market events. Keeps duplicates in the
    /// replay journal so replay exercises the same deduplication behavior.
    pub fn observe(&mut self, market: Market, event: &Event, received_ms: u64) -> bool {
        let options = matches!(market, Market::Options);
        let index = usize::from(options);
        let (sym, timestamp, kind) = match event {
            Event::Trade { sym, t, .. } => (sym, *t, 'T'),
            Event::Quote { sym, t, .. } => (sym, *t, 'Q'),
            Event::SecondBar { sym, e, .. } => (sym, *e, 'A'),
            Event::MinuteBar { sym, e, .. } => (sym, *e, 'M'),
            _ => return false,
        };
        self.records.push_back(Record {
            market,
            received_ms,
            event: event.clone(),
        });
        if self.records.len() > RECORDS {
            self.records.pop_front();
            self.overwritten_records += 1;
        }
        let quality = &mut self.quality[index];
        let mut sequence_is_late = false;
        if let Event::Trade {
            q, p, s, metadata, ..
        } = event
        {
            if let Some((sequence, date)) = q.zip(snipers_core::calendar::date_at(timestamp)) {
                let identity = (options, sym.clone(), date.to_string(), sequence);
                let fingerprint = (timestamp, p.to_bits(), s.to_bits(), metadata.clone());
                if let Some(previous) = self.identities.get(&identity) {
                    if previous == &fingerprint {
                        quality.duplicates += 1;
                    } else {
                        quality.sequence_conflicts += 1;
                    }
                    return false;
                }
                let previous = self
                    .latest_sequence
                    .entry((options, sym.clone(), date.to_string()))
                    .or_default();
                sequence_is_late = sequence < *previous;
                *previous = (*previous).max(sequence);
                while self.latest_sequence.len() > 256 {
                    self.latest_sequence.pop_first();
                }
                self.identities.insert(identity.clone(), fingerprint);
                self.order.push_back(identity);
                if self.order.len() > IDENTITIES {
                    let expired = self.order.pop_front().unwrap();
                    self.quality[usize::from(expired.0)].identity_evictions += 1;
                    self.identities.remove(&expired);
                }
            } else {
                quality.unidentified_trades += 1;
            }
        }
        let quality = &mut self.quality[index];
        let last = self.latest.entry((options, sym.clone(), kind)).or_default();
        if timestamp < *last || sequence_is_late {
            quality.out_of_order += 1;
        }
        *last = (*last).max(timestamp);
        // Rotation can introduce many distinct contracts over a long session.
        while self.latest.len() > 256 {
            self.latest.pop_first();
        }
        if timestamp > received_ms {
            quality.future_events += 1;
        } else if matches!(kind, 'T' | 'Q') {
            self.delays[index].push_back(received_ms - timestamp);
            if self.delays[index].len() > SAMPLES {
                self.delays[index].pop_front();
            }
        }
        true
    }
    pub fn quality(&self, market: Market) -> Quality {
        let index = usize::from(matches!(market, Market::Options));
        let mut result = self.quality[index].clone();
        let mut sorted: Vec<_> = self.delays[index].iter().copied().collect();
        sorted.sort_unstable();
        let percentile = |p: usize| {
            if sorted.is_empty() {
                None
            } else {
                Some(sorted[(sorted.len() * p).div_ceil(100) - 1])
            }
        };
        result.latency_samples = sorted.len();
        result.p50_ms = percentile(50);
        result.p95_ms = percentile(95);
        result.p99_ms = percentile(99);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn trade(t: u64, q: Option<u64>) -> Event {
        Event::Trade {
            sym: "QQQ".into(),
            p: 500.0,
            s: 1.0,
            t,
            q,
            metadata: Default::default(),
        }
    }
    #[test]
    fn identity_is_scoped_by_date_and_sequence_gaps_are_allowed() {
        let mut t = Telemetry::default();
        let time = 1_790_222_400_000;
        assert!(t.observe(Market::Stocks, &trade(time, Some(1)), time + 10));
        assert!(!t.observe(Market::Stocks, &trade(time, Some(1)), time + 20));
        assert!(t.observe(Market::Stocks, &trade(time + 1, Some(99)), time + 21));
        assert!(t.observe(
            Market::Stocks,
            &trade(time + 86_400_000, Some(1)),
            time + 86_400_010
        ));
        assert!(!t.observe(Market::Stocks, &trade(time + 2, Some(1)), time + 22));
        let q = t.quality(Market::Stocks);
        assert_eq!(q.duplicates, 1);
        assert_eq!(q.sequence_conflicts, 1);
        assert_eq!(q.out_of_order, 0);
    }
    #[test]
    fn missing_identity_is_not_deduplicated_and_future_latency_is_not_zero() {
        let mut t = Telemetry::default();
        for _ in 0..2 {
            assert!(t.observe(Market::Stocks, &trade(100, None), 90));
        }
        assert!(t.observe(Market::Stocks, &trade(90, None), 100));
        let q = t.quality(Market::Stocks);
        assert_eq!(q.unidentified_trades, 3);
        assert_eq!(q.future_events, 2);
        assert_eq!(q.out_of_order, 1);
        assert_eq!(q.latency_samples, 1);
        assert_eq!(q.p99_ms, Some(10));
    }
    #[test]
    fn replay_preserves_deduplication_and_original_receive_times() {
        let mut original = Telemetry::default();
        for (time, sequence) in [(100, 1), (100, 1), (200, 9), (150, 7)] {
            original.observe(Market::Stocks, &trade(time, Some(sequence)), 250);
        }
        let json = serde_json::to_string(&original.records).unwrap();
        let records: Vec<Record> = serde_json::from_str(&json).unwrap();
        let mut replay = Telemetry::default();
        for record in records {
            replay.observe(record.market, &record.event, record.received_ms);
        }
        assert_eq!(
            serde_json::to_value(original.quality(Market::Stocks)).unwrap(),
            serde_json::to_value(replay.quality(Market::Stocks)).unwrap()
        );
    }
    #[test]
    fn resource_limits_report_evictions_and_keep_latency_window_bounded() {
        let mut t = Telemetry::default();
        for sequence in 0..IDENTITIES as u64 + 2 {
            t.observe(
                Market::Stocks,
                &trade(1000 + sequence, Some(sequence)),
                1100 + sequence,
            );
        }
        assert_eq!(t.identities.len(), IDENTITIES);
        assert_eq!(t.order.len(), IDENTITIES);
        assert_eq!(t.records.len(), RECORDS);
        assert_eq!(
            t.overwritten_records,
            IDENTITIES as u64 + 2 - RECORDS as u64
        );
        let q = t.quality(Market::Stocks);
        assert_eq!(q.identity_evictions, 2);
        assert_eq!(q.latency_samples, SAMPLES);
        assert_eq!(q.p95_ms, Some(100));
    }
}
