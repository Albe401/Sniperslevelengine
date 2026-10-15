//! Local CPU translation using the installed Helsinki-NLP English-Spanish model.
use crate::investment::NewsReport;
use serde_json::{json,Value};
use std::{collections::BTreeMap,path::{Path,PathBuf},process::Stdio,time::Duration};
use tokio::io::AsyncWriteExt;
const VERSION:&str="es-local-helsinki-5bc4493-news-rules-v2";
const WORKER:&str=include_str!("../../../scripts/news_local_worker.py");
fn cache_key(item:&Value)->String{json!([VERSION,item["id"],item["title"],item["summary"]]).to_string()}
fn validate(input:&[Value],output:&[Value])->Result<(),String>{
 if input.len()!=output.len(){return Err("Traducción local incompleta".into());}
 for(a,b) in input.iter().zip(output){
  if a["id"]!=b["id"]||b["title"].as_str().is_none_or(|s|s.trim().is_empty())||!b["summary"].is_string()||!b["reason"].is_string()||!matches!(b["sentiment"].as_str(),Some("positive"|"negative"|"mixed"|"neutral"|"unknown"))||!matches!(b["importance"].as_str(),Some("high"|"medium"|"low"))||!matches!(b["eventType"].as_str(),Some("upgrade"|"downgrade"|"other")){return Err("Traducción local o clasificación no válida".into());}
 }Ok(())
}
fn runtime_at(root:&Path)->Option<(PathBuf,PathBuf)>{
 let python=root.join(if cfg!(windows){".venv-news-local/Scripts/python.exe"}else{".venv-news-local/bin/python"});
 let model=root.join(".tools/news-local/model/opus_en_es");
 (python.is_file()&&model.join("model/model.bin").is_file()&&model.join("source.spm").is_file()&&model.join("target.spm").is_file()&&model.join("snipers-model.json").is_file()).then_some((python,model))
}
fn runtime()->Result<(PathBuf,PathBuf),String>{
 if let Some(root)=std::env::var_os("SNIPERS_LOCAL_NEWS_ROOT"){return runtime_at(Path::new(&root)).ok_or("La ruta del traductor local no está instalada".into());}
 for origin in [std::env::current_exe().ok(),std::env::current_dir().ok()].into_iter().flatten(){for root in origin.ancestors(){if let Some(found)=runtime_at(root){return Ok(found);}}}
 Err("Falta el traductor local: ejecuta scripts/install-news-local.ps1".into())
}
pub async fn translate_news(report:&mut NewsReport,path:&Path)->Result<(),String>{
 let mut cache:BTreeMap<String,Value>=if path.is_file(){let bytes=std::fs::read(path).map_err(|_|"No se pudo leer caché local")?;if bytes.len()>40_000_000{return Err("Caché local demasiado grande".into());}serde_json::from_slice(&bytes).map_err(|_|"Caché local inválida; se conservó el archivo")?}else{BTreeMap::new()};
 let pending:Vec<Value>=report.items.iter().filter(|item|!cache.contains_key(&cache_key(item))).cloned().collect();
 if !pending.is_empty(){
  let(python,model)=runtime()?;
  let mut command=tokio::process::Command::new(python);
  command.args(["-u","-c",WORKER]).arg(model).env("PYTHONUTF8","1").env("PYTHONWARNINGS","ignore::UserWarning").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
  #[cfg(windows)]command.creation_flags(0x08000000);
  let mut child=command.spawn().map_err(|_|"No se pudo iniciar el traductor local")?;
  let bytes=serde_json::to_vec(&json!({"items":pending})).map_err(|_|"No se pudo preparar texto local")?;
  let mut stdin=child.stdin.take().ok_or("Traductor local sin entrada")?;
  stdin.write_all(&bytes).await.map_err(|_|"Fallo de entrada del traductor local")?;drop(stdin);
  let output=tokio::time::timeout(Duration::from_secs(300),child.wait_with_output()).await.map_err(|_|"La traducción local agotó el tiempo; se reintentará")?.map_err(|_|"No se pudo leer la traducción local")?;
  if !output.status.success(){return Err(format!("Traductor local: {}",String::from_utf8_lossy(&output.stderr).chars().take(300).collect::<String>()));}
  if output.stdout.len()>8_000_000{return Err("Traducción local demasiado grande".into());}
  let result:Value=serde_json::from_slice(&output.stdout).map_err(|_|"Respuesta local inválida")?;
  let rows=result["items"].as_array().ok_or("Respuesta local sin noticias")?;validate(&pending,rows)?;
  for(original,translated) in pending.iter().zip(rows){cache.insert(cache_key(original),translated.clone());}
  let current:std::collections::BTreeSet<String>=report.items.iter().map(cache_key).collect();
  while cache.len()>4000{let Some(key)=cache.keys().find(|k|!current.contains(*k)).cloned()else{break;};cache.remove(&key);}
  if let Some(parent)=path.parent(){std::fs::create_dir_all(parent).map_err(|_|"No se pudo crear caché local")?;}
  let temp=path.with_extension("tmp");std::fs::write(&temp,serde_json::to_vec(&cache).map_err(|_|"No se pudo serializar la caché")?).and_then(|_|std::fs::rename(&temp,path)).map_err(|_|"No se pudo guardar traducción local")?;
 }
 for item in &mut report.items{let translated=cache.get(&cache_key(item)).ok_or("Noticia pendiente de traducción local")?;item["originalTitle"]=item["title"].clone();item["originalSummary"]=item["summary"].clone();for field in ["title","summary","sentiment","importance","eventType","reason","contentDirection","marketReaction","category","categories","subtype","critical","headlineDirection","summaryDirection","conflictState","classificationVersion","materialityBasis"]{item[field]=translated[field].clone();}item["sentimentBasis"]=json!("Lectura local de titular y resumen · news-rules-v2");item["translated"]=json!(true);item["translatedAtMs"]=json!(crate::live::now_ms());item["classifiedAtMs"]=item["translatedAtMs"].clone();item["translationEngine"]=json!("Helsinki-NLP local · inglés → español");}
 Ok(())
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn rejects_missing_or_reordered_translations(){let input=vec![json!({"id":"a"})];assert!(validate(&input,&[]).is_err());assert!(validate(&input,&[json!({"id":"b"})]).is_err());}
 #[test]fn cache_changes_when_source_changes(){assert_ne!(cache_key(&json!({"id":"a","title":"old"})),cache_key(&json!({"id":"a","title":"new"})));}
 #[tokio::test]async fn cached_news_needs_no_runtime_or_network(){let path=std::env::temp_dir().join(format!("snipers-news-test-{}.json",std::process::id()));let original=json!({"id":"cached","title":"Company raises guidance","summary":"Revenue rose"});let translated=json!({"id":"cached","title":"La compañía mejora sus previsiones","summary":"Los ingresos aumentaron","reason":"Previsiones","sentiment":"positive","importance":"high","eventType":"other"});let cache=BTreeMap::from([(cache_key(&original),translated)]);std::fs::write(&path,serde_json::to_vec(&cache).unwrap()).unwrap();let mut report=NewsReport{items:vec![original],fetched_ms:0,truncated:false};translate_news(&mut report,&path).await.unwrap();assert_eq!(report.items[0]["translated"],true);assert_eq!(report.items[0]["originalTitle"],"Company raises guidance");assert_eq!(report.items[0]["title"],"La compañía mejora sus previsiones");std::fs::remove_file(path).unwrap();}
}

