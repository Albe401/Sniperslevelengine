//! Snapshot-to-snapshot context, never an order-flow or entry signal.
use crate::{calendar, horizons::Horizon, options::Observation, universe};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Application policy, not a provider guarantee about Greek publication times.
pub const MAX_GAP_MS: u64 = 15 * 60_000;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub available: bool,
    pub side: String,
    pub reason: String,
    pub call: Option<f64>,
    pub put: Option<f64>,
    pub net: Option<f64>,
    pub pct_gross: Option<f64>,
    pub previous_net: Option<f64>,
    pub previous_ms: Option<u64>,
    pub observed_ms: u64,
}
impl Change {
    fn missing(time: u64, reason: &str) -> Self {
        Self {
            available: false,
            side: "BASELINE".into(),
            reason: reason.into(),
            call: None,
            put: None,
            net: None,
            pct_gross: None,
            previous_net: None,
            previous_ms: None,
            observed_ms: time,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Sample {
    symbol: String,
    mode: String,
    expirations: Vec<String>,
    // Multiplier changes must reset comparison even when the ticker is unchanged.
    contracts: Vec<(String, Option<u64>)>,
    source: String,
    time: u64,
    call: f64,
    put: f64,
    net: f64,
    complete: bool,
}
impl Sample {
    fn valid(&self, now: u64) -> bool {
        universe::stock_asset(&self.symbol).is_some()
            && matches!(
                self.mode.as_str(),
                "DAY" | "NEXT_SESSION" | "WEEK" | "NEXT4"
            )
            && self.complete
            && !self.expirations.is_empty()
            && !self.contracts.is_empty()
            && self.contracts.len() <= 20_000
            && !self.source.is_empty()
            && self.source.len() <= 128
            && self.time <= now
            && now - self.time <= MAX_GAP_MS
            && calendar::date_at(self.time).is_some()
            && calendar::date_at(self.time) == calendar::date_at(now)
            && [
                self.call,
                self.put,
                self.net,
                self.call.abs() + self.put.abs(),
            ]
            .iter()
            .all(|n| n.is_finite())
            && self.call >= 0.0
            && self.put <= 0.0
            && (self.net - (self.call + self.put)).abs()
                <= (self.call.abs() + self.put.abs()).max(1.0) * 1e-10
    }
}

#[derive(Default)]
pub struct Tracker {
    // Only configured stock/ETF symbols and four modes can enter this map.
    previous: BTreeMap<(String, String), (Sample, Change)>,
}
#[derive(Serialize, Deserialize)]
struct Checkpoint {
    schema: u32,
    symbol: String,
    written_ms: u64,
    samples: Vec<Sample>,
}
impl Tracker {
    /// Store only raw baselines. Cached analytical conclusions are never trusted
    /// on restore; a fresh observation must recompute the next comparison.
    pub fn checkpoint(&self, symbol: &str, now: u64) -> Result<Vec<u8>, &'static str> {
        if universe::stock_asset(symbol).is_none() {
            return Err("Activo DEX inválido");
        }
        let samples = self
            .previous
            .values()
            .filter(|(s, _)| s.symbol == symbol && s.valid(now))
            .map(|(s, _)| s.clone())
            .collect();
        serde_json::to_vec(&Checkpoint {
            schema: 1,
            symbol: symbol.into(),
            written_ms: now,
            samples,
        })
        .map_err(|_| "No se pudo serializar la base DEX")
    }
    pub fn restore(bytes: &[u8], symbol: &str, now: u64) -> Result<Self, &'static str> {
        if bytes.len() > 8_000_000 {
            return Err("Base DEX demasiado grande");
        }
        let data: Checkpoint = serde_json::from_slice(bytes).map_err(|_| "Base DEX corrupta")?;
        if data.schema != 1
            || data.symbol != symbol
            || universe::stock_asset(symbol).is_none()
            || data.written_ms > now
            || now - data.written_ms > MAX_GAP_MS
            || calendar::date_at(now).is_none()
            || calendar::date_at(data.written_ms) != calendar::date_at(now)
            || data.samples.len() > 4
        {
            return Err("Base DEX de otra versión, activo o sesión");
        }
        let mut tracker = Self::default();
        for sample in data.samples {
            let mut ids = std::collections::BTreeSet::new();
            let mut dates = std::collections::BTreeSet::new();
            if sample.symbol != symbol
                || !sample.valid(now)
                || sample.time > data.written_ms
                || sample.expirations.len() > 32
                || sample
                    .expirations
                    .iter()
                    .any(|d| calendar::day_bounds(d).is_none() || !dates.insert(d))
                || sample.contracts.iter().any(|(id, m)| {
                    crate::options::underlying(id) != Some(symbol)
                        || !ids.insert(id)
                        || m.is_some_and(|bits| {
                            let n = f64::from_bits(bits);
                            !n.is_finite() || n <= 0.0
                        })
                })
            {
                return Err("Identidad o valores de base DEX inválidos");
            }
            let key = (sample.symbol.clone(), sample.mode.clone());
            let missing =
                Change::missing(sample.time, "Base recuperada; esperando observación nueva");
            if tracker.previous.insert(key, (sample, missing)).is_some() {
                return Err("Horizonte DEX duplicado");
            }
        }
        Ok(tracker)
    }
    pub fn clear(&mut self) {
        self.previous.clear();
    }

    pub fn observe(&mut self, observation: &Observation, horizon: &Horizon, now: u64) -> Change {
        let mut expirations = horizon.expirations.clone();
        expirations.sort();
        expirations.dedup();
        let e = &horizon.exposure;
        let contracts = observation
            .contracts
            .iter()
            .filter(|(_, c)| expirations.contains(&c.expiration))
            .map(|(id, c)| (id.clone(), c.premium_multiplier.map(f64::to_bits)))
            .collect();
        self.apply(
            Sample {
                symbol: observation.symbol.clone(),
                mode: horizon.mode.clone(),
                expirations,
                contracts,
                source: horizon.reference_source.clone(),
                time: observation.observed_ms,
                call: e.call_dex,
                put: e.put_dex,
                net: e.net_dex,
                complete: e.dex_complete && horizon.observed_ms == observation.observed_ms,
            },
            now,
        )
    }

    fn apply(&mut self, current: Sample, now: u64) -> Change {
        let key = (current.symbol.clone(), current.mode.clone());
        if !current.valid(now) {
            self.previous.remove(&key);
            return Change::missing(current.time, "DEX incompleto, inválido o fuera de vigencia");
        }
        let mut output = Change::missing(current.time, "Primera observación comparable");
        if let Some((old, cached)) = self.previous.get(&key) {
            if current.time < old.time {
                return Change::missing(current.time, "Snapshot fuera de orden; base conservada");
            }
            if current.time == old.time {
                if current == *old {
                    return cached.clone();
                }
                self.previous.remove(&key);
                return Change::missing(
                    current.time,
                    "Conflicto en el mismo timestamp; base invalidada",
                );
            }
            if calendar::date_at(old.time) != calendar::date_at(current.time)
                || current.time - old.time > MAX_GAP_MS
            {
                output.reason = "Nueva base por sesión o intervalo superior a 15 minutos".into();
            } else if old.expirations != current.expirations
                || old.contracts != current.contracts
                || old.source != current.source
            {
                output.reason =
                    "Nueva base por cambio de vencimientos, contratos o referencia".into();
            } else {
                let call = current.call - old.call;
                let put = current.put - old.put;
                let net = current.net - old.net;
                let gross = current.call.abs() + current.put.abs();
                let bias = if gross > 0.0 { net / gross } else { 0.0 };
                let pct = (bias * 100_000.0).round_ties_even() / 1000.0;
                if [call, put, net, pct].iter().all(|v| v.is_finite()) {
                    output = Change {
                        available: true,
                        side: if bias >= 0.005 {
                            "CALL"
                        } else if bias <= -0.005 {
                            "PUT"
                        } else {
                            "FLAT"
                        }
                        .into(),
                        reason:
                            "Variación entre observaciones REST comparables; no flujo negociado"
                                .into(),
                        call: Some(call),
                        put: Some(put),
                        net: Some(net),
                        pct_gross: Some(pct),
                        previous_net: Some(old.net),
                        previous_ms: Some(old.time),
                        observed_ms: current.time,
                    };
                } else {
                    output.reason = "Desbordamiento; nueva base sin variación".into();
                }
            }
        }
        self.previous.insert(key, (current, output.clone()));
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    fn time(text: &str) -> u64 {
        chrono::DateTime::parse_from_rfc3339(text)
            .unwrap()
            .timestamp_millis() as u64
    }
    fn sample() -> Sample {
        Sample {
            symbol: "QQQ".into(),
            mode: "DAY".into(),
            expirations: vec!["2026-09-24".into()],
            contracts: vec![("one".into(), Some(100f64.to_bits()))],
            source: "stock_ws".into(),
            time: time("2026-09-24T10:30:00-04:00"),
            call: 1000.0,
            put: -800.0,
            net: 200.0,
            complete: true,
        }
    }
    #[test]
    fn checkpoint_restores_baseline_without_trusting_cached_readings_or_foreign_identity() {
        let mut old = sample();
        old.contracts[0].0 = "O:QQQ260924C00500000".into();
        let mut tracker = Tracker::default();
        tracker.apply(old.clone(), old.time);
        let bytes = tracker.checkpoint("QQQ", old.time).unwrap();
        let mut restored = Tracker::restore(&bytes, "QQQ", old.time + 1).unwrap();
        assert!(!restored.apply(old.clone(), old.time + 1).available);
        let mut new = old.clone();
        new.time += 60_000;
        new.call += 300.;
        new.net += 300.;
        let actual = restored.apply(new.clone(), new.time);
        assert!(actual.available);
        assert_eq!(actual.net, Some(300.));
        assert!(Tracker::restore(&bytes, "SPY", old.time + 1).is_err());
        assert!(Tracker::restore(&bytes, "QQQ", old.time + MAX_GAP_MS + 1).is_err());
        assert!(Tracker::restore(&bytes, "QQQ", old.time - 1).is_err());
        for variant in 0..4 {
            let mut data: Value = serde_json::from_slice(&bytes).unwrap();
            match variant {
                0 => data["samples"][0]["contracts"][0][0] = "O:SPY260924C00500000".into(),
                1 => data["samples"][0]["net"] = 999999.into(),
                2 => {
                    let duplicate = data["samples"][0].clone();
                    data["samples"].as_array_mut().unwrap().push(duplicate);
                }
                _ => data["schema"] = 2.into(),
            }
            assert!(
                Tracker::restore(&serde_json::to_vec(&data).unwrap(), "QQQ", old.time + 1).is_err()
            );
        }
    }
    #[test]
    fn seven_python_fixtures_match_including_noise_boundaries() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-dex-change.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let mut tracker = Tracker::default();
            let mut current = sample();
            current.call = case["current"]["call_dex"].as_f64().unwrap();
            current.put = case["current"]["put_dex"].as_f64().unwrap();
            current.net = case["current"]["net_dex"].as_f64().unwrap();
            current.complete = case["current"]["available"].as_bool().unwrap();
            current.expirations = serde_json::from_value(case["expirations"].clone()).unwrap();
            if !case["previous"].is_null() {
                let p = &case["previous"];
                let mut previous = sample();
                previous.call = p["call_dex"].as_f64().unwrap();
                previous.put = p["put_dex"].as_f64().unwrap();
                previous.net = p["net_dex"].as_f64().unwrap();
                previous.time = time(p["timestamp"].as_str().unwrap());
                tracker.apply(previous, current.time);
            }
            let actual = tracker.apply(current.clone(), current.time);
            let expected = &case["result"];
            assert_eq!(
                actual.available,
                expected["dex_change_available"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
            assert_eq!(actual.side, expected["dex_change_side"].as_str().unwrap());
            for (value, field) in [
                (actual.call, "call_dex_change"),
                (actual.put, "put_dex_change"),
                (actual.net, "net_dex_change"),
                (actual.pct_gross, "dex_change_pct_gross"),
                (actual.previous_net, "previous_net_dex"),
            ] {
                assert_eq!(value, expected[field].as_f64(), "{} {field}", case["name"]);
            }
            assert_eq!(
                actual.previous_ms,
                expected["previous_timestamp"].as_str().map(time)
            );
        }
    }
    #[test]
    fn reset_guards_prevent_comparing_changed_or_incomplete_universes() {
        for variant in 0..8 {
            let mut tracker = Tracker::default();
            let old = sample();
            tracker.apply(old.clone(), old.time);
            let mut new = old.clone();
            new.time += 60_000;
            new.call += 100.0;
            new.net += 100.0;
            match variant {
                0 => new.symbol = "SPY".into(),
                1 => new.mode = "WEEK".into(),
                2 => new.contracts.push(("two".into(), Some(100f64.to_bits()))),
                3 => new.source = "snapshot".into(),
                4 => new.expirations = vec!["2026-09-25".into()],
                5 => new.time += MAX_GAP_MS,
                6 => new.complete = false,
                _ => new.contracts[0].1 = Some(50f64.to_bits()),
            }
            assert!(
                !tracker.apply(new.clone(), new.time).available,
                "variant {variant}"
            );
            if variant == 6 {
                new.complete = true;
                new.time += 1;
                assert!(!tracker.apply(new.clone(), new.time).available);
            }
        }
    }
    #[test]
    fn normalized_chain_integrates_without_comparing_daily_oi_as_tick_flow() {
        let mut observation = Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: sample().time,
            observed_date: "2026-09-24".into(),
            oi_reference_date: None,
            contracts: BTreeMap::new(),
        };
        for (kind, cp, delta) in [("call", "C", 0.5), ("put", "P", -0.5)] {
            let mut c = crate::options::normalize(&serde_json::json!({
                "details":{"ticker":format!("O:QQQ260924{cp}00500000"),"expiration_date":"2026-09-24","contract_type":kind,"strike_price":500},
                "open_interest":10,"day":{"volume":20},"greeks":{"delta":delta,"gamma":0.01},"implied_volatility":0.2
            }), "QQQ").unwrap();
            c.premium_multiplier = Some(100.0);
            observation.contracts.insert(c.ticker.clone(), c);
        }
        let mut tracker = Tracker::default();
        let first = crate::horizons::calculate(&observation, 500.0, "stock_ws").unwrap();
        assert!(
            !tracker
                .observe(&observation, &first[0], observation.observed_ms)
                .available
        );
        observation.observed_ms += 60_000;
        observation
            .contracts
            .values_mut()
            .find(|c| c.kind == "call")
            .unwrap()
            .delta = Some(0.6);
        let second = crate::horizons::calculate(&observation, 500.0, "stock_ws").unwrap();
        let change = tracker.observe(&observation, &second[0], observation.observed_ms);
        assert!(change.available);
        assert_eq!(change.side, "CALL");
        // DEX is equivalent shares: delta * OI * multiplier, without spot.
        assert_eq!(change.net, Some(100.0));
        observation.observed_ms += 60_000;
        observation
            .contracts
            .values_mut()
            .next()
            .unwrap()
            .open_interest = None;
        let partial = crate::horizons::calculate(&observation, 500.0, "stock_ws").unwrap();
        assert!(
            !tracker
                .observe(&observation, &partial[0], observation.observed_ms)
                .available
        );
        let midnight = time("2026-09-25T00:00:00-04:00");
        let mut old = sample();
        old.time = midnight - 1;
        tracker.apply(old.clone(), old.time);
        old.time = midnight;
        assert!(!tracker.apply(old.clone(), midnight).available);
    }
    #[test]
    fn replay_order_clock_and_conflicts_cannot_fabricate_a_change() {
        let mut tracker = Tracker::default();
        let first = sample();
        tracker.apply(first.clone(), first.time);
        let mut second = first.clone();
        second.time += 60_000;
        second.net += 9.0;
        second.call += 9.0;
        let result = tracker.apply(second.clone(), second.time);
        assert!(result.available);
        assert_eq!(tracker.apply(second.clone(), second.time).net, result.net);
        assert!(!tracker.apply(first.clone(), second.time).available);
        assert_eq!(
            tracker.previous.values().next().unwrap().0.time,
            second.time
        );
        second.net += 1.0;
        second.call += 1.0;
        assert!(!tracker.apply(second.clone(), second.time).available);
        assert!(tracker.previous.is_empty());
        assert!(!tracker.apply(second.clone(), first.time).available);
        second.call = f64::NAN;
        assert!(!tracker.apply(second.clone(), second.time).available);
    }
}
