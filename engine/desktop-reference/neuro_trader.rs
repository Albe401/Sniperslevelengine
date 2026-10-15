//! Starts only the independent Neuro Trader service, without touching market feeds or Paper.
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::Manager;
#[derive(Default)]
pub struct Task(pub Mutex<Option<Child>>);
fn terminate(child: &mut Child) {
    // The Windows venv launcher can own a second Python process. Stop only our tree.
    #[cfg(windows)]
    if child.try_wait().ok().flatten().is_none() {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .creation_flags(0x08000000)
            .stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    let _ = child.kill();
    let _ = child.wait();
}
impl Task {
    pub fn stop(&self) {
        if let Ok(mut slot) = self.0.lock() {
            if let Some(mut child) = slot.take() {
                terminate(&mut child);
            }
        }
    }
}
impl Drop for Task {
    fn drop(&mut self) {
        self.stop();
    }
}
fn runtime() -> Result<PathBuf, String> {
    let mut paths = Vec::new();
    if let Some(p) = std::env::var_os("SNIPERS_NEURO_DIR") {
        paths.push(PathBuf::from(p));
    }
    for base in [
        std::env::current_dir().ok(),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_owned())),
    ]
    .into_iter()
    .flatten()
    {
        for parent in base.ancestors().take(7) {
            paths.push(parent.join("integrations/neuro-trader-engine/runtime"));
        }
    }
    paths.into_iter().find(|p|p.join("snipers_service.py").is_file()&&p.join(".venv/Scripts/python.exe").is_file()&&p.join("frontend/dist/index.html").is_file())
      .ok_or("Falta instalar Neuro Trader Engine. Consulta integrations/neuro-trader-engine/README.md".into())
}
fn healthy() -> bool {
    let addr: SocketAddr = "127.0.0.1:8768".parse().unwrap();
    let Ok(mut socket) = TcpStream::connect_timeout(&addr, Duration::from_millis(250)) else {
        return false;
    };
    let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = socket.set_write_timeout(Some(Duration::from_millis(500)));
    if socket
        .write_all(
            b"GET /neuro-health HTTP/1.1\r\nHost: 127.0.0.1:8768\r\nConnection: close\r\n\r\n",
        )
        .is_err()
    {
        return false;
    }
    let mut response = String::new();
    socket.take(4096).read_to_string(&mut response).is_ok()
        && response.starts_with("HTTP/1.1 200")
        && response.contains("neuro-trader-engine-6.8")
}
#[tauri::command]
pub async fn open_neuro_trader(app: tauri::AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let task = app.state::<Task>();
        let mut slot = task.0.lock().map_err(|_| "Servicio ocupado")?;
        if let Some(child) = slot.as_mut() {
            if child
                .try_wait()
                .map_err(|_| "No se pudo verificar el servicio")?
                .is_none()
            {
                return if healthy() {
                    Ok("http://127.0.0.1:8768/ui/".into())
                } else {
                    Err("El servicio no responde; cierra y vuelve a abrir Snipers".into())
                };
            }
            *slot = None;
        }
        // A verified local Neuro instance may already serve the scanner or have
        // survived a module update. Reuse it without taking ownership of it.
        if healthy() {
            return Ok("http://127.0.0.1:8768/ui/".into());
        }
        let root = runtime()?;
        if TcpStream::connect_timeout(
            &"127.0.0.1:8768".parse().unwrap(),
            Duration::from_millis(250),
        )
        .is_ok()
        {
            return Err(
                "El puerto local 8768 está ocupado; no se reemplazó el proceso existente".into(),
            );
        }
        let mut command = Command::new(root.join(".venv/Scripts/python.exe"));
        command
            .args([
                "-m",
                "uvicorn",
                "snipers_service:app",
                "--host",
                "127.0.0.1",
                "--port",
                "8768",
                "--no-access-log",
            ])
            .current_dir(root)
            .env("PYTHONUTF8", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .map_err(|_| "No se pudo iniciar el Python aislado de Neuro Trader Engine")?;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(15) {
            if child
                .try_wait()
                .map_err(|_| "No se pudo verificar Python")?
                .is_some()
            {
                return Err(
                    "Neuro Trader Engine no inició; verifica su instalación independiente".into(),
                );
            }
            if healthy() {
                *slot = Some(child);
                return Ok("http://127.0.0.1:8768/ui/".into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        terminate(&mut child);
        Err("Neuro Trader Engine no respondió dentro del tiempo de inicio".into())
    })
    .await
    .map_err(|_| "No se pudo iniciar Neuro Trader Engine".to_owned())?
}
