//! In-memory shadow telemetry. No registration, orders, automatic learning or disk writes.
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
#[derive(Default)]
pub struct Observer {
    last: BTreeMap<String, (u64, String)>,
    transitions: BTreeMap<String, String>,
    records: VecDeque<Value>,
    evicted: usize,
}
impl Observer {
    pub fn observe(
        &mut self,
        frame: &Value,
        baseline: &Value,
        anchor: Option<f64>,
        held: Option<&str>,
        now: u64,
    ) -> Value {
        let symbol = frame["symbol"].as_str().unwrap_or("");
        let baseline = if baseline["symbol"] == symbol
            && baseline["at"].as_u64().is_some_and(|at| at <= now && now - at <= 5000)
        { baseline.clone() } else { Value::Null };
        if self.last.get(symbol).is_some_and(|(at, _)| now < *at) {
            return json!({"version":"smc-shadow-v1","state":"reordered","strategyConnected":false,"executionEnabled":false});
        }
        let context = crate::smc::read(frame, symbol, now);
        let review = crate::smc::advisory(&context, &baseline, anchor, held, now);
        let date = snipers_core::calendar::date_at(now)
            .map(|d| d.to_string())
            .unwrap_or_default();
        if self.last.get(symbol).is_some_and(|(_, d)| d != &date) {
            let prefix = format!("{symbol}:");
            self.transitions.retain(|id, _| !id.starts_with(&prefix));
        }
        if self.last.len() >= 16 && !self.last.contains_key(symbol) {
            self.last.clear();
            self.transitions.clear();
        }
        self.last.insert(symbol.into(), (now, date));
        let mut new_records = vec![];
        for layer in context["layers"].as_array().into_iter().flatten() {
            for field in ["events", "setups", "orderBlocks"] {
                for evidence in layer[field].as_array().into_iter().flatten() {
                    let Some(id) = evidence["id"].as_str() else {
                        continue;
                    };
                    let key = format!("{symbol}:{field}:{id}");
                    let fingerprint = evidence.to_string();
                    if self.transitions.get(&key).is_some_and(|old| old == &fingerprint) {
                        continue;
                    }
                    let revised = self.transitions.insert(key, fingerprint).is_some();
                    let record = json!({"version":"smc-shadow-v1","symbol":symbol,"at":now,"timeframe":layer["timeframe"],"kind":field,"evidence":evidence,"revised":revised,"baselineEligible":baseline["eligible"],"entryBasis":baseline["entry_basis"],"review":review,"strategyConnected":false,"executionEnabled":false});
                    new_records.push(record.clone());
                    if self.records.len() == 1000 {
                        self.records.pop_front();
                        self.evicted += 1;
                    }
                    self.records.push_back(record);
                }
            }
        }
        if self.transitions.len() > 4096 {
            self.transitions.clear();
        }
        json!({"version":"smc-shadow-v1","context":context,"review":review,"newRecords":new_records,"retained":self.records.len(),"evicted":self.evicted,"strategyConnected":false,"executionEnabled":false})
    }
    pub fn records(&self) -> Vec<Value> {
        self.records.iter().cloned().collect()
    }
    pub fn summarize(&self) -> Value {
        let mut by_kind = BTreeMap::<String, usize>::new();
        for r in &self.records {
            *by_kind
                .entry(r["kind"].as_str().unwrap_or("unknown").into())
                .or_default() += 1;
        }
        json!({"version":"smc-shadow-v1","records":self.records.len(),"byKind":by_kind,"evicted":self.evicted,"basis":"pattern-observations-not-trade-results","profitability":null,"automaticUpdates":false,"executionEnabled":false})
    }
}
