//! Offline diagnostic replay of every retained market recording.
fn main()->Result<(),Box<dyn std::error::Error>>{
 let dir=std::path::PathBuf::from(std::env::args().nth(1).ok_or("recordings directory required")?);
 let mut results=vec![];
 for entry in std::fs::read_dir(dir)?{
  let p=entry?.path();if p.extension().is_none_or(|e|e!="json"){continue;}
  let result=massive_feed::recording::Recording::read(&p).and_then(|r|r.replay());
  results.push(match result{Ok(v)=>serde_json::json!({"file":p.file_name().and_then(|s|s.to_str()),"replay":v}),Err(e)=>serde_json::json!({"file":p.file_name().and_then(|s|s.to_str()),"error":e})});
 }
 println!("{}",serde_json::to_string_pretty(&serde_json::json!({"version":"recording-history-replay-v1","results":results,"executionEnabled":false,"validated":false,"scope":"Native telemetry replay with original receive/provider clocks, duplicates and retained-ring limits. Not a reconstructed complete trading session."}))?);Ok(())
}
