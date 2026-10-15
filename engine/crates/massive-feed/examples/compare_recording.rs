fn main()->Result<(),Box<dyn std::error::Error>> {
 let args:Vec<_>=std::env::args().collect();
 if args.len()!=3 {return Err("Uso: compare_recording <grabacion.json> <resultado-nuevo.json>".into());}
 let recording=massive_feed::recording::Recording::read(std::path::Path::new(&args[1]))?;
 let result=massive_feed::comparison::compare(&recording)?;
 let file=std::fs::OpenOptions::new().create_new(true).write(true).open(&args[2])?;
 serde_json::to_writer_pretty(&file,&result)?;file.sync_all()?;
 println!("{}",serde_json::json!({"symbol":recording.symbol,"status":"comparison_saved","output":args[2],"automaticPromotion":false}));
 Ok(())
}
