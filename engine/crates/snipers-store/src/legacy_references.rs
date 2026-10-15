//! Preserve original OI/DEX files as an archive, never as certified live baselines.
use serde::Serialize;
use std::{
    io::{Read, Write},
    path::Path,
};
const NAMES: [&str; 3] = [
    "massive_oi_history.json",
    "dex_history.json",
    "oi_history.json",
];
const LIMIT: u64 = 32_000_000;
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSummary {
    pub file: String,
    pub bytes: usize,
    pub records: usize,
}
fn collect(directory: &Path) -> Result<Vec<(ReferenceSummary, Vec<u8>)>, &'static str> {
    let directory = directory
        .canonicalize()
        .map_err(|_| "Carpeta del legado no disponible")?;
    let mut result = vec![];
    for name in NAMES {
        let path = directory.join(name);
        if !path
            .try_exists()
            .map_err(|_| "No se pudo revisar el historial del legado")?
        {
            continue;
        }
        let resolved = path
            .canonicalize()
            .map_err(|_| "Historial del legado no accesible")?;
        if resolved.parent() != Some(directory.as_path()) {
            return Err("El historial apunta fuera de la carpeta del legado");
        }
        let file = std::fs::File::open(&resolved)
            .map_err(|_| "No se pudo abrir el historial del legado")?;
        if !file
            .metadata()
            .map_err(|_| "Historial no accesible")?
            .is_file()
        {
            return Err("Historial del legado no es un archivo");
        }
        let mut bytes = vec![];
        file.take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "No se pudo leer el historial del legado")?;
        if bytes.len() as u64 > LIMIT {
            return Err("Historial del legado excede capacidad");
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| "Historial del legado no contiene JSON válido")?;
        let records = if name == "massive_oi_history.json" {
            value
                .get("snapshots")
                .and_then(|v| v.as_array())
                .map(|v| v.len())
        } else {
            value
                .as_object()
                .map(|v| v.len())
                .or_else(|| value.as_array().map(|v| v.len()))
        }
        .ok_or("Estructura de archivo histórico no reconocida")?;
        result.push((
            ReferenceSummary {
                file: name.into(),
                bytes: bytes.len(),
                records,
            },
            bytes,
        ));
    }
    Ok(result)
}
pub fn inspect_references(directory: &Path) -> Result<Vec<ReferenceSummary>, &'static str> {
    Ok(collect(directory)?
        .into_iter()
        .map(|(summary, _)| summary)
        .collect())
}
/// The destination must be new. Its manifest is written last; no source is changed.
pub fn archive_references(
    source: &Path,
    destination: &Path,
) -> Result<Vec<ReferenceSummary>, &'static str> {
    let entries = collect(source)?;
    if entries.is_empty() {
        return Ok(vec![]);
    }
    std::fs::create_dir(destination)
        .map_err(|_| "No se pudo crear una copia nueva de referencias")?;
    for (summary, bytes) in &entries {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination.join(&summary.file))
            .map_err(|_| "No se pudo crear la copia de referencia")?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| "Copia de referencia incompleta")?;
    }
    let summaries: Vec<_> = entries.into_iter().map(|(s, _)| s).collect();
    let manifest=serde_json::to_vec_pretty(&serde_json::json!({"schema":1,"purpose":"LEGACY_REFERENCE_ARCHIVE_ONLY","liveBaseline":false,"files":summaries})).map_err(|_|"No se pudo preparar el manifiesto")?;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination.join("manifest.json"))
        .map_err(|_| "No se pudo crear el manifiesto")?;
    file.write_all(&manifest)
        .and_then(|()| file.sync_all())
        .map_err(|_| "Manifiesto incompleto")?;
    Ok(summaries)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn references_preserve_original_bytes_and_do_not_create_live_baselines() {
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("source");
        std::fs::create_dir(&src).unwrap();
        let bytes = b"{\n  \"snapshots\": [{\"date\":\"2026-09-23\",\"symbol\":\"QQQ\"}]\n}\n";
        std::fs::write(src.join(NAMES[0]), bytes).unwrap();
        std::fs::write(src.join(NAMES[1]), b"{\"QQQ|DAY\":{\"net_dex\":5}}").unwrap();
        std::fs::write(src.join(".env"), b"NEVER_COPY_THIS=secret").unwrap();
        let dest = root.path().join("copy");
        let summaries = archive_references(&src, &dest).unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].records, 1);
        assert_eq!(std::fs::read(dest.join(NAMES[0])).unwrap(), bytes);
        assert_eq!(std::fs::read(src.join(NAMES[0])).unwrap(), bytes);
        assert!(!dest.join(".env").exists());
        assert!(dest.join("manifest.json").is_file());
        assert!(archive_references(&src, &dest).is_err());
        std::fs::write(src.join(NAMES[1]), b"{").unwrap();
        let bad = root.path().join("bad");
        assert!(archive_references(&src, &bad).is_err());
        assert!(!bad.exists());
    }
}
