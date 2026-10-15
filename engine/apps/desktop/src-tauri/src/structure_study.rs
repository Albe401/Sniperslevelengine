use serde_json::{json,Value};
use std::{process::{Command,Stdio},sync::Mutex};
use tauri::Manager;
#[derive(Default)]
pub struct Task(Mutex<Option<String>>);
fn valid_symbol(symbol:&str)->bool{
 symbol.len()<=8&&symbol.split('.').count()<=2&&symbol.split('.').all(|p|!p.is_empty()&&p.bytes().all(|b|b.is_ascii_uppercase()))
}
fn queue_alive(dir:&std::path::Path)->bool{
 #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;let p=dir.join("queue.lock");p.exists()&&std::fs::OpenOptions::new().read(true).write(true).share_mode(0).open(p).is_err()}
 #[cfg(not(windows))]{let _=dir;false}
}
#[tauri::command]
pub fn structure_study(app:tauri::AppHandle,task:tauri::State<'_,Task>,symbol:String,start:bool)->Result<Value,String>{
 if !valid_symbol(&symbol){return Err("Símbolo inválido".into())}
 let dir=app.path().app_local_data_dir().map_err(|_|"STUDY_STORAGE")?.join("structure-study");
 std::fs::create_dir_all(&dir).map_err(|_|"STUDY_STORAGE")?;
 let path=dir.join(format!("{symbol}.json"));
 let seed=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/structure-study");
 for suffix in [".json",".bars.json"] {
  let source=seed.join(format!("{symbol}{suffix}"));let destination=dir.join(format!("{symbol}{suffix}"));
  if !destination.exists()&&source.exists(){let _=std::fs::copy(source,destination);}
 }
 let mut result=std::fs::read(&path).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).unwrap_or(json!({"symbol":symbol,"status":"pending","groups":[]}));
 let mut guard=task.0.lock().map_err(|_|"STUDY_BUSY")?;
 if start {
  if guard.is_some()||queue_alive(&dir){return Err("Ya hay un activo en estudio; espera a que termine.".into())}
  std::fs::write(dir.join("structure_study.py"),include_str!("../../../../scripts/structure_study.py")).map_err(|_|"STUDY_STORAGE")?;
  std::fs::write(dir.join("reconstruct_iv_v1.py"),include_str!("../../../../scripts/reconstruct_iv_v1.py")).map_err(|_|"STUDY_STORAGE")?;
  *guard=Some(symbol.clone());let handle=app.clone();let worker=symbol.clone();
  let worker_dir=dir.clone();
  tauri::async_runtime::spawn_blocking(move||{
   let dir=worker_dir;
   let python=std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_default().join(".cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe");
   let mut cmd=Command::new(if python.is_file(){python}else{"python".into()});
   cmd.arg(dir.join("structure_study.py")).arg("--symbol").arg(&worker).arg("--output").arg(&dir).env("PYTHONDONTWRITEBYTECODE","1").stdout(Stdio::null()).stderr(Stdio::null());
   if std::env::var_os("SNIPERS_ENV_FILE").is_none(){cmd.env("SNIPERS_ENV_FILE",std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../.env"));}
   #[cfg(windows)]{use std::os::windows::process::CommandExt;cmd.creation_flags(0x08000000);}
   let ok=cmd.status().map(|s|s.success()).unwrap_or(false);
   if !ok {let _=std::fs::write(dir.join(format!("{worker}.error")),"No se pudo completar la consulta histórica. Reintenta; las velas guardadas se conservan.");}else{let _=std::fs::remove_file(dir.join(format!("{worker}.error")));}
   if let Ok(mut guard)=handle.state::<Task>().0.lock(){*guard=None;}
  });
 }
 result["running"]=json!(guard.as_deref()==Some(&symbol));
 result["activeSymbol"]=json!(*guard);
 if let Ok(error)=std::fs::read_to_string(dir.join(format!("{symbol}.error"))){result["error"]=json!(error);}
 Ok(result)
}

#[tauri::command]
pub fn structure_queue(app:tauri::AppHandle,task:tauri::State<'_,Task>,action:String,symbols:Vec<String>)->Result<Value,String>{
 let dir=app.path().app_local_data_dir().map_err(|_|"STUDY_STORAGE")?.join("structure-study");std::fs::create_dir_all(&dir).map_err(|_|"STUDY_STORAGE")?;
 let path=dir.join("queue.json");
 let mut queue=std::fs::read(&path).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).unwrap_or(json!({"remaining":[],"status":"pending","current":null}));
 let mut guard=task.0.lock().map_err(|_|"STUDY_BUSY")?;
 if action=="pause"{std::fs::write(dir.join("queue.pause"),"").map_err(|_|"STUDY_STORAGE")?;}
 else if action=="start"||action=="resume"{
  if guard.is_some()||queue_alive(&dir){return Err("El activo actual sigue en análisis. La pausa se aplica al terminarlo.".into())}
  if action=="start"{
   if symbols.is_empty()||symbols.len()>250||symbols.iter().any(|s|!valid_symbol(s)){return Err("Selecciona activos válidos.".into())}
   let mut remaining=Vec::new();for symbol in symbols {
    let complete=std::fs::read(dir.join(format!("{symbol}.json"))).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).is_some_and(|v|v["status"]=="complete");
    if !complete&&!remaining.contains(&symbol){remaining.push(symbol);}
   }
   queue=json!({"remaining":remaining,"status":"pending","current":null});
  }
  let _=std::fs::remove_file(dir.join("queue.pause"));
  std::fs::write(&path,serde_json::to_vec(&queue).map_err(|_|"STUDY_STORAGE")?).map_err(|_|"STUDY_STORAGE")?;
  for (name,code) in [("structure_queue.py",include_str!("../../../../scripts/structure_queue.py")),("structure_study.py",include_str!("../../../../scripts/structure_study.py")),("reconstruct_iv_v1.py",include_str!("../../../../scripts/reconstruct_iv_v1.py"))]{std::fs::write(dir.join(name),code).map_err(|_|"STUDY_STORAGE")?;}
  *guard=Some("__QUEUE__".into());let handle=app.clone();let worker_dir=dir.clone();
  tauri::async_runtime::spawn_blocking(move||{
   let python=std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_default().join(".cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe");
   let mut cmd=Command::new(if python.is_file(){python}else{"python".into()});
   cmd.arg(worker_dir.join("structure_queue.py")).arg("--output").arg(&worker_dir).stdout(Stdio::null()).stderr(Stdio::null()).env("PYTHONDONTWRITEBYTECODE","1");
   if std::env::var_os("SNIPERS_ENV_FILE").is_none(){cmd.env("SNIPERS_ENV_FILE",std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../.env"));}
   #[cfg(windows)]{use std::os::windows::process::CommandExt;cmd.creation_flags(0x08000000);}
   let ok=cmd.status().map(|s|s.success()).unwrap_or(false);
   if !ok{let _=std::fs::write(worker_dir.join("queue.error"),"No se pudo iniciar el análisis. Puedes continuar la lista guardada.");}
   if let Ok(mut guard)=handle.state::<Task>().0.lock(){*guard=None;}
  });
 }else if action!="status"{return Err("Acción inválida".into())}
 let mut rows=Vec::new();
 for entry in std::fs::read_dir(&dir).map_err(|_|"STUDY_STORAGE")?.flatten(){
  let file=entry.path();let name=file.file_stem().and_then(|v|v.to_str()).unwrap_or("");
  if file.extension().and_then(|v|v.to_str())!=Some("json")||!valid_symbol(name){continue}
  if let Some(v)=std::fs::read(&file).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()){rows.push(json!({"symbol":name,"status":v["status"],"through":v["through"],"sessions":v["sessions"]}));}
 }
 Ok(json!({"queue":queue,"rows":rows,"running":guard.is_some()||queue_alive(&dir),"paused":dir.join("queue.pause").exists()}))
}

#[cfg(test)] mod tests{
 use super::*;
 #[test]fn catalog_symbols_and_paths(){
  for symbol in ["NVDA","BRK.B","NBIS"]{assert!(valid_symbol(symbol));}
  for symbol in ["","../AAPL","A..B",".B","B.","brk.b","ABCDEFGHI"]{assert!(!valid_symbol(symbol));}
 }
}
