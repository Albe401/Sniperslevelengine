use std::path::{Path, PathBuf};

/// Intentionally no Debug/Serialize implementation: keys never cross IPC or logs.
pub struct Credentials(pub(crate) String);

impl Credentials {
    pub fn load() -> Result<Self, &'static str> {
        if let Ok(key) = std::env::var("MASSIVE_API_KEY") {
            if !key.trim().is_empty() {
                return Ok(Self(key));
            }
        }
        let mut paths = Vec::new();
        if let Ok(path) = std::env::var("SNIPERS_ENV_FILE") {
            return Self::from_file(Path::new(&path));
        }
        if let Ok(dir) = std::env::current_dir() {
            candidates(&dir, &mut paths);
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates(dir, &mut paths);
            }
        }
        for path in paths {
            if path.is_file() {
                return Self::from_file(&path);
            }
        }
        Err("No se encontró .env. Configura SNIPERS_ENV_FILE con su ruta local.")
    }

    fn from_file(path: &Path) -> Result<Self, &'static str> {
        let entries = dotenvy::from_path_iter(path).map_err(|_| "No se puede leer .env.")?;
        for entry in entries {
            let (name, value) = entry.map_err(|_| "Formato de .env no válido.")?;
            if name == "MASSIVE_API_KEY" && !value.trim().is_empty() {
                return Ok(Self(value));
            }
        }
        Err("Falta MASSIVE_API_KEY en .env.")
    }
}

pub(crate) fn candidates(dir: &Path, paths: &mut Vec<PathBuf>) {
    for parent in dir.ancestors().take(6) {
        let path = parent.join(".env");
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
}
