use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::Manager;
struct Maintenance(Arc<AtomicBool>);
impl Drop for Maintenance {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
fn source(path: Option<String>) -> Result<PathBuf, String> {
    let candidate = match path.filter(|s| !s.trim().is_empty()) {
        Some(path) => PathBuf::from(path),
        None => {
            let roots = [
                std::env::current_dir().ok(),
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.to_owned())),
            ];
            roots
                .into_iter()
                .flatten()
                .flat_map(|p| {
                    p.ancestors()
                        .take(7)
                        .map(|a| a.join("data/snipers_paper_trades.db"))
                        .collect::<Vec<_>>()
                })
                .find(|p| p.is_file())
                .ok_or("Indica la ruta del diario Python")?
        }
    };
    let path = candidate
        .canonicalize()
        .map_err(|_| "No se encontró el diario indicado")?;
    let meta = std::fs::metadata(&path).map_err(|_| "No se pudo revisar el diario")?;
    if !meta.is_file() || meta.len() > 256_000_000 {
        return Err("El diario indicado no es una base admitida".into());
    }
    Ok(path)
}
#[tauri::command]
pub async fn preview_legacy_database(path: Option<String>) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = source(path)?;
        let counts = snipers_store::inspect_legacy(&path)
            .map_err(|_| "El esquema del diario no coincide con el proyecto anterior")?;
        let references = snipers_store::legacy_references::inspect_references(path.parent().ok_or("Carpeta del legado no disponible")?)?;
        Ok(serde_json::json!({"path":path.to_string_lossy(),"counts":counts,"references":references}))
    })
    .await
    .map_err(|_| "No se pudo revisar el diario")?
}
#[tauri::command]
pub async fn get_legacy_journal(
    app: tauri::AppHandle,
    before_id: Option<i64>,
) -> Result<serde_json::Value, String> {
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta local no disponible")?;
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&directory).map_err(|_| "Carpeta local no disponible")?;
        let ledger = snipers_store::Ledger::open(&directory.join("snipers-v1.sqlite"))
            .map_err(|_| "Diario no disponible")?;
        serde_json::to_value(
            ledger
                .legacy_journal(before_id, 25)
                .map_err(|_| "No se pudo leer el archivo del legado")?,
        )
        .map_err(|_| "No se pudo presentar el diario".to_owned())
    })
    .await
    .map_err(|_| "No se pudo consultar el diario")?
}
#[tauri::command]
pub async fn import_legacy_database(
    path: String,
    app: tauri::AppHandle,
    task: tauri::State<'_, crate::LiveTask>,
) -> Result<serde_json::Value, String> {
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta local no disponible")?;
    let guard = {
        let active = task.handle.lock().map_err(|_| "Monitor no disponible")?;
        if active.is_some() {
            return Err("Desconecta el monitor Rust antes de importar el diario".into());
        }
        if task.maintenance.swap(true, Ordering::SeqCst) {
            return Err("Ya hay una importación en curso".into());
        }
        Maintenance(task.maintenance.clone())
    };
    tauri::async_runtime::spawn_blocking(move||{
        let _guard=guard;let source=source(Some(path))?;
        snipers_store::inspect_legacy(&source).map_err(|_|"No se pudo verificar el esquema del legado")?;
        std::fs::create_dir_all(&directory).map_err(|_|"No se pudo abrir el almacén local")?;
        let backups=directory.join("migration-backups");std::fs::create_dir_all(&backups).map_err(|_|"No se pudo crear la carpeta de copias")?;
        static ID:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
        let id=format!("{}-{}-{}",massive_feed::live::now_ms(),std::process::id(),ID.fetch_add(1,Ordering::SeqCst));
        let legacy_copy=backups.join(format!("legacy-{id}.sqlite"));let rust_copy=backups.join(format!("rust-before-import-{id}.sqlite"));
        snipers_store::backup_legacy(&source,&legacy_copy).map_err(|_|"No se pudo crear una copia consistente del legado")?;
        let mut ledger=snipers_store::Ledger::open(&directory.join("snipers-v1.sqlite")).map_err(|_|"No se pudo abrir el diario Rust")?;
        ledger.backup_to(&rust_copy).map_err(|_|"No se pudo respaldar el diario Rust; importación cancelada")?;
        let reference_copy=backups.join(format!("references-{id}"));
        let references=snipers_store::legacy_references::archive_references(source.parent().ok_or("Carpeta del legado no disponible")?,&reference_copy)?;
        let imported=ledger.import_legacy(&legacy_copy).map_err(|_|"La importación se rechazó por incompatibilidad o cambios en registros ya archivados; se conserva el diario anterior")?;
        Ok(serde_json::json!({"imported":imported,"legacyBackup":legacy_copy.to_string_lossy(),"rustBackup":rust_copy.to_string_lossy(),"references":references,"referenceBackup":if references.is_empty(){None}else{Some(reference_copy.to_string_lossy())}}))
    }).await.map_err(|_|"No se pudo completar la importación")?
}
