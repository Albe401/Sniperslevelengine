//! Compare preserved window with native passive CVD; no network or credentials.
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();if args.len()!=2{return Err("Usage: replay_classification_window AUDIT_DIRECTORY".into());}
 let dir=std::path::Path::new(&args[1]);let state=massive_feed::live::new_state_for("QQQ")?;
 state.lock().unwrap().research_symbols=vec!["AAPL".into(),"QQQ".into()];
 let mut now=0;let mut rows=0;
 for i in 0..100 {let path=dir.join(format!("replay-{i}.json"));if !path.exists(){break;}
  let recording=massive_feed::recording::Recording::read(&path)?;
  now=now.max(recording.records.iter().map(|r|r.received_ms).max().unwrap_or(0));rows+=recording.records.len();massive_feed::live::ingest_recording(&state,&recording);
 }
 if now==0{return Err("No recorded rows".into());}
 let frames:Vec<_>=["AAPL","QQQ"].into_iter().map(|s|massive_feed::live::bot_frame(&state,s,now)).collect();
 std::fs::write(dir.join("native-replay.json"),serde_json::to_vec_pretty(&serde_json::json!({"historical":true,"rows":rows,"at":now,"frames":frames,"ordersSent":0}))?)?;println!("rows={rows}");Ok(())
}
