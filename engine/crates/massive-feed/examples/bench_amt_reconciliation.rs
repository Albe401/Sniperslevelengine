use massive_feed::auction_reconciliation::{self,Request};
fn main()->Result<(),Box<dyn std::error::Error>> {
 let args:Vec<_>=std::env::args().collect();if args.len()!=3{return Err("Uso: evidencia-real resultado-nuevo".into());}
 let data:serde_json::Value=serde_json::from_reader(std::fs::File::open(&args[1])?)?;
 let request=Request{symbol:data["symbol"].as_str().ok_or("Sin símbolo")?.into(),epoch:1,start_ms:data["window"]["startMs"].as_u64().ok_or("Sin ventana")?,end_ms:data["window"]["endMs"].as_u64().ok_or("Sin ventana")?,live:serde_json::from_value(data["live"].clone())?};
 let rest=serde_json::from_value::<Vec<snipers_core::auction_map::Trade>>(data["reference"]["trades"].clone())?;
 let excluded=data["reference"]["excluded_rows"].as_u64().unwrap_or(0) as usize;
 let corrections=data["reference"]["unresolved_corrections"].as_u64().unwrap_or(0) as usize;
 let mut timings=vec![];
 for _ in 0..300 {let timer=std::time::Instant::now();let r=auction_reconciliation::compare(&request,&rest,excluded,corrections,request.end_ms)?;assert_eq!(r.status,"matched_scoped_window");std::hint::black_box(r);timings.push(timer.elapsed().as_secs_f64()*1000.);}
 timings.sort_by(f64::total_cmp);let r=serde_json::json!({"scope":"Debug comparator only on saved real 111-trade window; no network, market latency or full-session capacity benchmark","iterations":300,"unit":"milliseconds","p50":timings[150],"p95":timings[285],"max":timings[299],"rawTimingsSorted":timings});
 let file=std::fs::OpenOptions::new().write(true).create_new(true).open(&args[2])?;serde_json::to_writer_pretty(file,&r)?;println!("{}",serde_json::json!({"p50":r["p50"],"p95":r["p95"],"max":r["max"]}));Ok(())
}
