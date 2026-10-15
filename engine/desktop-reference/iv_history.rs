use serde_json::{json,Value};
use std::{collections::BTreeMap,process::{Command,Stdio},sync::Mutex};
use tauri::Manager;
#[derive(Default)]
pub struct Task(Mutex<BTreeMap<String,(bool,u64)>>);

#[tauri::command]
pub fn get_iv_history(app:tauri::AppHandle,task:tauri::State<'_,Task>,symbol:String)->Result<Value,String>{
 if symbol.is_empty()||symbol.len()>8||!symbol.bytes().all(|b|b.is_ascii_uppercase()){return Err("Símbolo inválido".into())}
 let dir=app.path().app_local_data_dir().map_err(|_|"IV_STORAGE")?.join("iv-history");
 std::fs::create_dir_all(&dir).map_err(|_|"IV_STORAGE")?;
 let path=dir.join(format!("{symbol}.json"));
 let source=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/iv-history").join(format!("{symbol}.json"));
 if !path.exists()&&source.exists(){let _=std::fs::copy(source,&path);}
 let mut result=std::fs::read(&path).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).unwrap_or(json!({"symbol":symbol,"status":"pending","observations":[],"expected":0}));
 let staged=dir.join(format!("{symbol}.v2.json"));
 if let Some(candidate)=std::fs::read(&staged).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()){
  if result["observations"].as_array().is_some_and(|r|!r.is_empty()) {
   result["experimentalProgress"]=json!(candidate["observations"].as_array().map_or(0,|r|r.len()));
  }
 }
 let now=massive_feed::live::now_ms();
 let mut guard=task.0.lock().map_err(|_|"IV_BUSY")?;
 let (running,last)=guard.get(&symbol).copied().unwrap_or((false,0));
 if !running&&now.saturating_sub(last)>300_000&&guard.values().filter(|v|v.0).count()<2 {
  let script=dir.join("reconstruct_iv.py");
  let code=include_str!("../../../../scripts/reconstruct_iv_v1.py");
  if std::fs::read_to_string(&script).ok().as_deref()!=Some(code){std::fs::write(&script,code).map_err(|_|"IV_STORAGE")?;}
  guard.insert(symbol.clone(),(true,now));
  let handle=app.clone();let worker_symbol=symbol.clone();
  tauri::async_runtime::spawn_blocking(move||{
   let python=std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_default().join(".cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe");
   let mut command=Command::new(if python.is_file(){python}else{"python".into()});
   command.arg(script).arg("--symbols").arg(&worker_symbol).arg("--output").arg(&dir).stdout(Stdio::null()).stderr(Stdio::null()).env("PYTHONDONTWRITEBYTECODE","1");
   // Credentials remain in the local environment/file; never cross frontend IPC.
   if std::env::var_os("SNIPERS_ENV_FILE").is_none(){
    let root=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    command.env("SNIPERS_ENV_FILE",root.join(".env"));
   }
   #[cfg(windows)]{use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
   let success=command.status().map(|s|s.success()).unwrap_or(false);
   if let Ok(mut guard)=handle.state::<Task>().0.lock(){guard.insert(worker_symbol,(false,massive_feed::live::now_ms()));}
   if !success {let _=std::fs::write(dir.join(format!("{symbol}.error")),"IV_RECONSTRUCTION_FAILED");}
  });
  result["running"]=json!(true);
 }else{result["running"]=json!(running);}
 Ok(result)
}
