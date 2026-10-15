//! Portable, bounded diagnostic replay; no network, credentials or database writes.
use crate::{
    telemetry::{Record, Telemetry},
    Event, Market,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub schema: u32,
    pub symbol: String,
    pub overwritten_records: u64,
    pub records: Vec<Record>,
}
impl Recording {
    pub fn capture(symbol: &str, telemetry: &Telemetry) -> Self {
        Self {
            schema: 1,
            symbol: symbol.into(),
            overwritten_records: telemetry.overwritten_records,
            records: telemetry.records.iter().cloned().collect(),
        }
    }
    pub fn read(path: &Path) -> Result<Self, &'static str> {
        let file = std::fs::File::open(path).map_err(|_| "No se pudo abrir el registro")?;
        let mut bytes = Vec::new();
        file.take(25_000_001)
            .read_to_end(&mut bytes)
            .map_err(|_| "No se pudo leer el registro")?;
        if bytes.len() > 25_000_000 {
            return Err("Registro demasiado grande");
        }
        let result: Self = serde_json::from_slice(&bytes).map_err(|_| "Registro inválido")?;
        result.validate()?;
        Ok(result)
    }
    fn validate(&self) -> Result<(), &'static str> {
        if self.schema != 1
            || self.records.len() > 10_000
            || snipers_core::universe::stock_asset(&self.symbol).is_none()
        {
            return Err("Formato, tamaño o activo de registro no admitido");
        }
        if self
            .records
            .iter()
            .any(|r| matches!(r.event, Event::Status { .. } | Event::Unknown))
        {
            return Err("Solo se admiten eventos de mercado");
        }
        Ok(())
    }
    pub fn save(&self, directory: &Path) -> Result<PathBuf, &'static str> {
        self.validate()?;
        std::fs::create_dir_all(directory)
            .map_err(|_| "No se pudo crear la carpeta de registros")?;
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let path = directory.join(format!(
            "{}-{}-{}-{}.json",
            self.symbol,
            crate::live::now_ms(),
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| "No se pudo crear el registro")?;
        let mut writer = std::io::BufWriter::new(file);
        serde_json::to_writer(&mut writer, self)
            .map_err(|_| "No se pudo guardar el registro completo")?;
        writer
            .flush()
            .map_err(|_| "No se pudo completar el registro")?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|_| "No se pudo confirmar la escritura del registro")?;
        Ok(path)
    }
    /// Reproduce quality statistics for the retained window, not a whole trading day.
    pub fn replay(&self) -> Result<serde_json::Value, &'static str> {
        self.validate()?;
        let mut replay = Telemetry::default();
        for record in &self.records {
            replay.observe(record.market, &record.event, record.received_ms);
        }
        Ok(serde_json::json!({
            "scope": "retained-market-event-window", "symbol": self.symbol,
            "records": self.records.len(), "overwrittenRecords": self.overwritten_records,
            "partial": self.overwritten_records > 0,
            "stocks": replay.quality(Market::Stocks), "options": replay.quality(Market::Options),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_roundtrip_preserves_conditions_and_original_timestamps() {
        let event: Event = serde_json::from_str(r#"{"ev":"T","sym":"QQQ","p":500,"s":1,"t":1000,"q":7,"x":4,"i":"123","c":[0,12],"pt":999,"trfi":1,"trft":1000}"#).unwrap();
        let mut telemetry = Telemetry::default();
        telemetry.observe(Market::Stocks, &event, 1010);
        telemetry.observe(Market::Stocks, &event, 1020);
        let recording = Recording::capture("QQQ", &telemetry);
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-recordings");
        let path = recording.save(&directory).unwrap();
        let loaded = Recording::read(&path).unwrap();
        assert_eq!(
            serde_json::to_value(&loaded.records[0].event).unwrap()["c"],
            serde_json::json!([0, 12])
        );
        assert_eq!(loaded.replay().unwrap()["stocks"]["duplicates"], 1);
        assert_eq!(loaded.replay().unwrap()["stocks"]["p50Ms"], 10);
        // Only this test's newly created file; no directory traversal or recursive removal.
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn recordings_reject_control_messages_and_report_truncation() {
        let mut recording = Recording {
            schema: 1,
            symbol: "QQQ".into(),
            overwritten_records: 1,
            records: vec![],
        };
        assert_eq!(recording.replay().unwrap()["partial"], true);
        recording.records.push(Record {
            market: Market::Stocks,
            received_ms: 1,
            event: Event::Status {
                status: "auth_success".into(),
                message: String::new(),
            },
        });
        assert!(recording.replay().is_err());
        recording.records.clear();
        recording.schema = 2;
        assert!(recording.replay().is_err());
    }
}
