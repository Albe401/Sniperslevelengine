//! New immutable observations only. Legacy JSON/SQLite are never rewritten here.
use snipers_core::options::Observation;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub fn save(directory: &Path, observation: &Observation) -> Result<PathBuf, &'static str> {
    validate(observation)?;
    std::fs::create_dir_all(directory).map_err(|_| "No se pudo crear el historial OI")?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = directory.join(format!(
        "oi-{}-{}-{}-{}.json",
        observation.symbol,
        observation.observed_ms,
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| "No se pudo crear la observación OI")?;
    let mut writer = std::io::BufWriter::new(file);
    serde_json::to_writer(&mut writer, observation)
        .map_err(|_| "No se pudo escribir la observación OI")?;
    writer.flush().map_err(|_| "Escritura OI incompleta")?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|_| "No se pudo confirmar la escritura OI")?;
    Ok(path)
}
fn validate(observation: &Observation) -> Result<(), &'static str> {
    if observation.schema != 1
        || snipers_core::universe::stock_asset(&observation.symbol).is_none()
        || observation.contracts.len() > 20_000
        || snipers_core::calendar::date_at(observation.observed_ms)
            .map(|d| d.to_string())
            .as_deref()
            != Some(&observation.observed_date)
    {
        return Err("Observación OI inválida");
    }
    Ok(())
}
pub fn read(path: &Path) -> Result<Observation, &'static str> {
    let file = std::fs::File::open(path).map_err(|_| "No se pudo leer el historial OI")?;
    let mut bytes = Vec::new();
    file.take(25_000_001)
        .read_to_end(&mut bytes)
        .map_err(|_| "Lectura OI incompleta")?;
    if bytes.len() > 25_000_000 {
        return Err("Archivo OI demasiado grande");
    }
    let observation: Observation = serde_json::from_slice(&bytes)
        .map_err(|_| "Historial OI corrupto; no se sustituye por vacío")?;
    validate(&observation)?;
    Ok(observation)
}
pub fn previous(
    directory: &Path,
    symbol: &str,
    before: &str,
) -> Result<Option<Observation>, &'static str> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Carpeta OI no accesible"),
    };
    let prefix = format!("oi-{symbol}-");
    let mut candidate: Option<(u64, PathBuf)> = None;
    for entry in entries {
        let entry = entry.map_err(|_| "No se pudo enumerar el historial OI")?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".json") {
            continue;
        }
        let Some(time) = name
            .strip_prefix(&prefix)
            .and_then(|s| s.split('-').next())
            .and_then(|s| s.parse::<u64>().ok())
        else {
            continue;
        };
        if snipers_core::calendar::date_at(time).is_some_and(|d| d.to_string().as_str() < before)
            && candidate.as_ref().is_none_or(|(old, _)| time > *old)
        {
            candidate = Some((time, entry.path()));
        }
    }
    match candidate {
        Some((_, path)) => {
            let observation = read(&path)?;
            if observation.symbol != symbol || observation.observed_date.as_str() >= before {
                return Err("Historial OI de otro activo o fecha");
            }
            Ok(Some(observation))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_files_survive_restart_and_intraday_rows_are_not_daily_baselines() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/test-oi-{}-{}",
            std::process::id(),
            crate::live::now_ms()
        ));
        let mut observation = Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: 1_790_172_000_000,
            observed_date: "2026-09-23".into(),
            oi_reference_date: None,
            contracts: Default::default(),
        };
        let first = save(&directory, &observation).unwrap();
        let second = save(&directory, &observation).unwrap();
        assert_ne!(first, second);
        assert!(previous(&directory, "QQQ", "2026-09-23").unwrap().is_none());
        assert_eq!(
            previous(&directory, "QQQ", "2026-09-24")
                .unwrap()
                .unwrap()
                .observed_ms,
            observation.observed_ms
        );
        assert!(previous(&directory, "SPY", "2026-09-24").unwrap().is_none());
        observation.symbol = "../escape".into();
        assert!(save(&directory, &observation).is_err());
        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(second).unwrap();
        // Empty test directory only; no recursive deletion.
        std::fs::remove_dir(directory).unwrap();
    }
}
