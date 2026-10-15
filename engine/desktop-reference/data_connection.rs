//! Explicit, local transfer of feed ownership; never touches the desktop process.
use serde_json::{json,Value};
use std::{path::{Path,PathBuf},process::Command,time::{Duration,Instant}};
fn root()->Result<PathBuf,String>{
 let exe=std::env::current_exe().map_err(|_|"Ruta de Snipers no disponible")?;
 exe.parent().and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf).ok_or("Ruta de instalación inválida".into())
}
fn pointer(root:&Path)->Result<Value,String>{
 let bytes=std::fs::read(root.join("data/background-service/active-run.json")).map_err(|_|"No se pudo comprobar el receptor registrado")?;
 let text=std::str::from_utf8(&bytes).map_err(|_|"Registro de receptor inválido")?.trim_start_matches('\u{feff}');
 serde_json::from_str(text).map_err(|_|"Registro de receptor inválido".into())
}
fn run_directory(root:&Path,value:&Value)->Result<PathBuf,String>{
 let folder=PathBuf::from(value["directory"].as_str().ok_or("Directorio del receptor ausente")?);
 let actual=folder.canonicalize().map_err(|_|"Directorio del receptor no disponible")?;
 let expected=root.join("data/background-service").canonicalize().map_err(|_|"Directorio de servicio no disponible")?;
 if actual.parent()!=Some(expected.as_path())||!actual.file_name().is_some_and(|n|n.to_string_lossy().starts_with("run-")){return Err("Directorio del receptor fuera de la instalación".into());}
 Ok(actual)
}
#[cfg(windows)]fn hidden(command:&mut Command){use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
#[cfg(not(windows))]fn hidden(_: &mut Command){}
#[derive(Debug,PartialEq)]enum ProcessState{Gone,Receiver,Other}
fn identify_row(text:&str,pid:u32)->ProcessState{
 for row in text.lines(){let fields:Vec<_>=row.split(',').map(|s|s.trim().trim_matches('"')).collect();
  if fields.get(1).and_then(|s|s.parse::<u32>().ok())==Some(pid){return if matches!(fields.first().copied(),Some("snipers-data-engine.exe"|"snipers-data-engine-shared.exe"|"snipers-data-engine-shared-v2.exe")){ProcessState::Receiver}else{ProcessState::Other};}
 }ProcessState::Gone
}
fn process_state(pid:u32)->Result<ProcessState,String>{
 let mut cmd=Command::new("tasklist.exe");cmd.args(["/FI",&format!("PID eq {pid}"),"/FO","CSV","/NH"]);hidden(&mut cmd);
 let output=cmd.output().map_err(|_|"No se pudo comprobar el proceso del receptor")?;
 if !output.status.success(){return Err("No se pudo comprobar el proceso del receptor".into());}
 Ok(identify_row(&String::from_utf8_lossy(&output.stdout),pid))
}
pub fn stop_receiver()->Result<(),String>{
 let root=root()?;
 if !root.join("data/background-service/active-run.json").exists(){return if massive_feed::service_ipc::present(){Err("Receptor sin registro verificable; no se abre otra conexión".into())}else{Ok(())};}
 let value=pointer(&root)?;let pid=value["pid"].as_u64().and_then(|p|u32::try_from(p).ok()).filter(|p|*p!=0).ok_or("PID del receptor inválido")?;
 match process_state(pid)?{ProcessState::Other=>return Err("El PID ya no pertenece al receptor; no se detuvo ningún proceso".into()),ProcessState::Gone=>{},ProcessState::Receiver=>{
  std::fs::write(run_directory(&root,&value)?.join("STOP"),b"").map_err(|_|"No se pudo solicitar la detención del receptor")?;
  let deadline=Instant::now()+Duration::from_secs(5);
  while Instant::now()<deadline&&process_state(pid)?==ProcessState::Receiver{std::thread::sleep(Duration::from_millis(200));}
  if process_state(pid)?==ProcessState::Receiver{
   // A frozen receiver cannot consume STOP. Only its verified PID may be terminated.
   let mut cmd=Command::new("taskkill.exe");cmd.args(["/PID",&pid.to_string(),"/F"]);hidden(&mut cmd);
   if !cmd.output().map_err(|_|"No se pudo detener el receptor congelado")?.status.success(){return Err("El receptor no se detuvo; se cancela la conexión directa".into());}
   let deadline=Instant::now()+Duration::from_secs(3);while Instant::now()<deadline&&process_state(pid)?==ProcessState::Receiver{std::thread::sleep(Duration::from_millis(100));}
  }
  if process_state(pid)?!=ProcessState::Gone{return Err("No se confirmó la salida del receptor; no se abre otra conexión".into());}
 }}
 if massive_feed::service_ipc::present(){return Err("El puerto del receptor sigue ocupado; no se abre una conexión duplicada".into());}
 Ok(())
}
pub fn ensure_receiver(symbol:&str)->Result<(),String>{
 if massive_feed::service_ipc::present(){return massive_feed::service_ipc::request(json!({"method":"snapshot"})).map(|_|());}
 if snipers_core::universe::stock_asset(symbol).is_none(){return Err("Activo no habilitado".into());}
 let root=root()?;
 if let Ok(value)=pointer(&root){if let Some(pid)=value["pid"].as_u64().and_then(|p|u32::try_from(p).ok()){if process_state(pid)?!=ProcessState::Gone{return Err("Hay un receptor sin respuesta; primero cambia a conexión directa para detenerlo".into());}}}
 let folder=root.join("data/background-service");std::fs::create_dir_all(&folder).map_err(|_|"Carpeta del receptor no disponible")?;
 let run=folder.join(format!("run-{}-{}",massive_feed::live::now_ms(),std::process::id()));
 let exe=root.join("target/debug/snipers-data-engine-shared-v2.exe");
 if !exe.is_file(){return Err("Ejecutable del receptor no instalado".into());}
 let mut cmd=Command::new(&exe);cmd.arg("--resume-shared").arg(symbol).arg(&run).current_dir(&root);hidden(&mut cmd);
 let stdout=std::fs::File::create(run.with_extension("stdout.log")).map_err(|_|"No se pudo guardar el arranque")?;
 let stderr=std::fs::File::create(run.with_extension("stderr.log")).map_err(|_|"No se pudo guardar el arranque")?;
 cmd.stdout(stdout).stderr(stderr);let child=cmd.spawn().map_err(|_|"No se pudo iniciar el receptor")?;
 std::fs::write(folder.join("active-run.json"),serde_json::to_vec_pretty(&json!({"pid":child.id(),"exe":exe,"directory":run,"symbol":symbol,"mode":"integrated_read_only","startedMs":massive_feed::live::now_ms()})).unwrap()).map_err(|_|"No se pudo registrar el receptor; revisa el proceso antes de reintentar")?;
 let deadline=Instant::now()+Duration::from_secs(10);
 while Instant::now()<deadline{if massive_feed::service_ipc::present(){return massive_feed::service_ipc::request(json!({"method":"snapshot"})).map(|_|());}if process_state(child.id())?==ProcessState::Gone{return Err("El receptor falló al arrancar; consulta su registro local".into());}std::thread::sleep(Duration::from_millis(200));}
 Err("El receptor aún no responde; no se abre una conexión directa automáticamente".into())
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn stale_pid_never_authorizes_stopping_desktop_or_another_process(){assert_eq!(identify_row("\"snipers-desktop.exe\",\"42\",\"Console\",\"1\"",42),ProcessState::Other);assert_eq!(identify_row("\"snipers-data-engine-shared-v2.exe\",\"43\"",42),ProcessState::Gone);assert_eq!(identify_row("\"snipers-data-engine-shared-v2.exe\",\"42\"",42),ProcessState::Receiver);}
 #[test]fn run_path_must_be_an_immediate_child_of_the_service_directory(){let root=std::env::temp_dir().join(format!("snipers-path-test-{}",massive_feed::live::now_ms()));let run=root.join("data/background-service/run-test");std::fs::create_dir_all(&run).unwrap();assert_eq!(run_directory(&root,&json!({"directory":run})).unwrap(),run.canonicalize().unwrap());assert!(run_directory(&root,&json!({"directory":root})).is_err());}
}
