//! Immutable DEX baselines. Partial/corrupt latest writes never fall back silently.
use snipers_core::{
    calendar,
    dex_change::{Tracker, MAX_GAP_MS},
};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub fn save(
    directory: &Path,
    symbol: &str,
    bytes: &[u8],
    now: u64,
) -> Result<PathBuf, &'static str> {
    Tracker::restore(bytes, symbol, now)?;
    std::fs::create_dir_all(directory).map_err(|_| "No se pudo crear el historial DEX")?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = directory.join(format!(
        "dex-{symbol}-{now:020}-{:010}-{:020}.json",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|_| "No se pudo crear la base DEX")?;
    file.write_all(bytes)
        .map_err(|_| "Escritura DEX incompleta")?;
    file.sync_all()
        .map_err(|_| "No se pudo confirmar la base DEX")?;
    Ok(path)
}
fn read(path: &Path) -> Result<Vec<u8>, &'static str> {
    let file = std::fs::File::open(path).map_err(|_| "No se pudo abrir la base DEX")?;
    let mut bytes = vec![];
    file.take(8_000_001)
        .read_to_end(&mut bytes)
        .map_err(|_| "No se pudo leer la base DEX")?;
    if bytes.len() > 8_000_000 {
        return Err("Base DEX demasiado grande");
    }
    Ok(bytes)
}
pub fn restore(directory: &Path, symbol: &str, now: u64) -> Result<Tracker, &'static str> {
    if snipers_core::universe::stock_asset(symbol).is_none() {
        return Err("Activo DEX inválido");
    }
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Tracker::default()),
        Err(_) => return Err("Historial DEX no accesible"),
    };
    let prefix = format!("dex-{symbol}-");
    let mut latest = 0;
    let mut paths = vec![];
    for (count, entry) in entries.enumerate() {
        if count >= 20_000 {
            return Err("Historial DEX excede el límite de lectura");
        }
        let entry = entry.map_err(|_| "No se pudo enumerar el historial DEX")?;
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
        if time > now {
            return Err("Base DEX con reloj futuro");
        }
        if now - time > MAX_GAP_MS || calendar::date_at(time) != calendar::date_at(now) {
            continue;
        }
        if time > latest {
            latest = time;
            paths.clear();
        }
        if time == latest {
            paths.push(entry.path());
        }
    }
    let Some(path) = paths.first() else {
        return Ok(Tracker::default());
    };
    let bytes = read(path)?;
    for other in paths.iter().skip(1) {
        if read(other)? != bytes {
            return Err("Bases DEX en conflicto");
        }
    }
    Tracker::restore(&bytes, symbol, now)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_baseline_roundtrip_and_corrupt_latest_is_not_hidden() {
        let temp = std::env::temp_dir().join(format!(
            "snipers-dex-{}-{}",
            std::process::id(),
            crate::live::now_ms()
        ));
        std::fs::create_dir(&temp).unwrap();
        let now = calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let tracker = Tracker::default();
        let bytes = tracker.checkpoint("QQQ", now).unwrap();
        let first = save(&temp, "QQQ", &bytes, now).unwrap();
        let second = save(&temp, "QQQ", &bytes, now).unwrap();
        assert_ne!(first, second);
        assert!(restore(&temp, "QQQ", now).is_ok());
        std::fs::write(&second, b"partial").unwrap();
        assert!(restore(&temp, "QQQ", now).is_err());
        assert!(restore(&temp, "QQQ", now + MAX_GAP_MS + 1).is_ok());
        assert!(restore(&temp, "QQQ", now - 1).is_err());
        assert!(restore(&temp, "SPY", now).is_ok());
        assert!(save(&temp, "SPY", &bytes, now).is_err());
        let resolved = std::fs::canonicalize(&temp).unwrap();
        assert!(resolved.starts_with(std::fs::canonicalize(std::env::temp_dir()).unwrap()));
        std::fs::remove_dir_all(resolved).unwrap();
    }
}
