use serde_json::{json,Value};
use std::path::PathBuf;
fn root()->PathBuf{PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")}
#[tauri::command]
pub async fn get_company_research_catalog()->Result<Value,String>{
 tauri::async_runtime::spawn_blocking(||{
  let directory=root().join("data/company-research");
  let mut value=snipers_store::company_research::catalog(&directory.join("companies.sqlite"))?;
  value["progress"]=std::fs::read(directory.join("run-status.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
  Ok(value)
 }).await.map_err(|_|"No se pudo consultar el catálogo".to_string())?
}
#[tauri::command]
pub async fn get_company_research_detail(symbol:String)->Result<Option<Value>,String>{
 tauri::async_runtime::spawn_blocking(move||snipers_store::company_research::detail(&root().join("data/company-research/companies.sqlite"),&symbol)).await.map_err(|_|"No se pudo consultar la compañía".to_string())?
}
#[tauri::command]
pub async fn start_company_research()->Result<Value,String>{
 tauri::async_runtime::spawn_blocking(||{
  let dir=root().join("data/company-research");
  if let Ok(bytes)=std::fs::read(dir.join("run-status.json")){if serde_json::from_slice::<Value>(&bytes).ok().is_some_and(|v|v["state"]=="running"){return Ok(json!({"state":"running"}));}}
  std::fs::create_dir_all(&dir).map_err(|_|"No se pudo abrir la carpeta de investigación")?;
  let log=std::fs::OpenOptions::new().create(true).append(true).open(dir.join("worker.log")).map_err(|_|"No se pudo guardar el registro")?;
  let err=log.try_clone().map_err(|_|"No se pudo guardar el registro")?;
  let mut cmd=std::process::Command::new("node");
  cmd.current_dir(root()).args(["scripts/run-company-research.mjs","--online","--only-new"]).stdout(log).stderr(err).stdin(std::process::Stdio::null());
  #[cfg(windows)]{use std::os::windows::process::CommandExt;cmd.creation_flags(0x08000000);}
  cmd.spawn().map_err(|_|"No se pudo iniciar el análisis. Comprueba que Node está disponible.")?;
  Ok(json!({"state":"starting"}))
 }).await.map_err(|_|"No se pudo iniciar la investigación".to_string())?
}
#[tauri::command]
pub async fn register_company_research(symbol:String,name:String,included:bool)->Result<Value,String>{
 if symbol.is_empty()||symbol.len()>12||!symbol.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit()||b==b'.'||b==b'-')||name.len()>300{return Err("Compañía inválida".into());}
 tauri::async_runtime::spawn_blocking(move||{
  static WRITER:std::sync::Mutex<()>=std::sync::Mutex::new(());
  let _guard=WRITER.lock().map_err(|_|"Vigilancia ocupada")?;
  let dir=root().join("data/company-research");std::fs::create_dir_all(&dir).map_err(|_|"No se pudo guardar compañía")?;
  let file=dir.join("extra-watchlist.json");
  let mut extras=if file.is_file(){let bytes=std::fs::read(&file).map_err(|_|"No se pudo leer vigilancia")?;serde_json::from_slice::<serde_json::Map<String,Value>>(&bytes).map_err(|_|"Vigilancia inválida; se preservó el archivo")?}else{serde_json::Map::new()};
  if included{extras.insert(symbol.clone(),json!({"name":name,"kind":"STOCK","sector":null}));}else{extras.remove(&symbol);}
  let bytes=serde_json::to_vec_pretty(&extras).map_err(|_|"No se pudo guardar vigilancia")?;
  let temp=dir.join("extra-watchlist.tmp");std::fs::write(&temp,bytes).and_then(|_|std::fs::rename(temp,file)).map_err(|_|"No se pudo guardar compañía")?;
  if included&&dir.join("companies.sqlite").is_file(){snipers_store::company_research::enqueue(&dir.join("companies.sqlite"),&symbol,&name)?;}
  Ok(json!({"registered":included}))
 }).await.map_err(|_|"No se pudo registrar compañía".to_string())?
}
