//! Local authenticated bridge. PostgreSQL credentials never cross desktop IPC.
use serde_json::{json, Value};
use std::{path::PathBuf,process::{Command,Stdio},time::Duration};
pub(crate) fn root()->Result<PathBuf,String>{
    let mut starts=Vec::new();if let Ok(p)=std::env::current_dir(){starts.push(p);}
    if let Ok(p)=std::env::current_exe(){if let Some(p)=p.parent(){starts.push(p.to_path_buf());}}
    for p in starts {for a in p.ancestors(){
        if a.join("scripts/snapshot_service.py").is_file(){return Ok(a.to_path_buf())}
        let r=a.join("snipers-rust");if r.join("scripts/snapshot_service.py").is_file(){return Ok(r)}
    }}Err("Instalación de snapshots no encontrada".into())
}
pub fn start()->Result<(),String>{
    let root=root()?;
    if !root.join("data/snapshots/connection.secret").is_file(){return Ok(())}
    if std::net::TcpStream::connect_timeout(&"127.0.0.1:36543".parse().unwrap(),Duration::from_millis(200)).is_ok(){return Ok(())}
    let bytes=std::fs::read_to_string(root.join("data/snapshots/runtime.json")).map_err(|_|"Runtime de snapshots pendiente")?;
    let config:Value=serde_json::from_str(bytes.trim_start_matches('\u{feff}')).map_err(|_|"Runtime de snapshots inválido")?;
    let python=config["python"].as_str().ok_or("Python de snapshots ausente")?;
    let mut cmd=Command::new(python);cmd.arg(root.join("scripts/snapshot_service.py")).current_dir(&root).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]{use std::os::windows::process::CommandExt;cmd.creation_flags(0x08000000);}
    let child=cmd.spawn().map_err(|_|"No se pudo iniciar snapshots")?;
    std::fs::write(root.join("data/snapshots/process.json"),serde_json::to_vec(&json!({"pid":child.id(),"startedMs":crate::live::now_ms()})).unwrap()).map_err(|_|"No se pudo registrar snapshots")?;
    Ok(())
}
pub async fn request(path:&str,body:Option<Value>)->Result<Value,String>{
    if !["status","latest","observe"].contains(&path){return Err("Consulta de snapshots inválida".into())}
    let root=root()?;
    // Native Windows launches may forbid cross-process loopback. These caches are
    // written only by the database service after a real Neon read, never from live data.
    if matches!(path,"status"|"latest") {
        let file=root.join(if path=="latest"{"data/snapshots/latest-from-neon.json"}else{"data/snapshots/status.json"});
        if let Ok(bytes)=std::fs::read(&file) {if let Ok(value)=serde_json::from_slice::<Value>(&bytes){
            if path=="latest" || value["writtenMs"].as_u64().is_some_and(|ms|crate::live::now_ms().saturating_sub(ms)<30000){return Ok(value)}
        }}
    }
    if path=="observe" {
        if let Some(value)=body.as_ref(){
            let folder=root.join("data/snapshots/observations");std::fs::create_dir_all(&folder).map_err(|_|"Observación snapshot pendiente")?;
            static SEQUENCE:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
            let name=format!("{}-{}-{}",crate::live::now_ms(),std::process::id(),SEQUENCE.fetch_add(1,std::sync::atomic::Ordering::Relaxed));
            let tmp=folder.join(format!("{name}.tmp"));let target=folder.join(format!("{name}.json"));
            std::fs::write(&tmp,serde_json::to_vec(value).map_err(|_|"Observación inválida")?).and_then(|_|std::fs::rename(tmp,target)).map_err(|_|"Observación snapshot pendiente")?;
            return Ok(json!({"ok":true}));
        }
    }
    let token=std::fs::read_to_string(root.join("data/snapshots/api.token")).map_err(|_|"Servicio de snapshots pendiente")?;
    let client=reqwest::Client::builder().timeout(Duration::from_secs(35)).build().map_err(|_|"Servicio de snapshots no disponible")?;
    let url=format!("http://127.0.0.1:36543/{path}");
    let request=if let Some(body)=body{client.post(url).json(&body)}else{client.get(url)};
    let response=request.header("X-Snipers-Token",token.trim()).send().await.map_err(|_|"Servicio de snapshots desconectado")?;
    if !response.status().is_success(){return Err("Snapshot no disponible en la base de datos".into())}
    response.json().await.map_err(|_|"Respuesta de snapshots inválida".into())
}
