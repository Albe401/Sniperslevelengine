//! Offline replay of identified real recordings. No socket, credentials or orders.
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();if args.len()!=3{return Err("Usage: replay_research_coverage INPUT OUTPUT".into());}
 let recording=massive_feed::recording::Recording::read(std::path::Path::new(&args[1]))?;
 let now=recording.records.iter().map(|r|r.received_ms).max().ok_or("Empty recording")?;
 let symbols:std::collections::BTreeSet<String>=recording.records.iter().filter_map(|r|match &r.event{massive_feed::Event::Trade{sym,..}|massive_feed::Event::Quote{sym,..}=>Some(sym.clone()),_=>None}).collect();
 let state=massive_feed::live::new_state_for("QQQ")?;state.lock().unwrap().research_symbols=symbols.iter().cloned().collect();
 massive_feed::live::ingest_recording(&state,&recording);
 let frames:Vec<_>=symbols.iter().map(|s|massive_feed::live::bot_frame(&state,s,now)).collect();
 let report=serde_json::json!({"historical":true,"partialRecording":true,"ordersSent":0,"at":now,"rows":recording.records.len(),"frames":frames});
 std::fs::write(&args[2],serde_json::to_vec_pretty(&report)?)?;println!("{}",serde_json::json!({"rows":recording.records.len(),"symbols":symbols,"ordersSent":0}));Ok(())
}
