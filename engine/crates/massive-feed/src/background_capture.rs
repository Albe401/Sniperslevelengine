//! Durable, bounded decoded-event capture; transport status/auth text is excluded.
use crate::{Event, Market};
use std::{fs::OpenOptions,io::Write,path::Path,sync::{Arc,atomic::{AtomicBool,AtomicU64,Ordering},mpsc},time::Duration};
#[derive(Default)]pub struct Counts {pub offered:AtomicU64,pub written:AtomicU64,pub dropped:AtomicU64,pub failed:AtomicBool,pub storage_limit:AtomicBool,pub segments:AtomicU64}
#[derive(Clone)]pub struct Capture {tx:mpsc::SyncSender<Vec<u8>>,pub counts:Arc<Counts>}
pub struct Writer {stop:Arc<AtomicBool>,thread:Option<std::thread::JoinHandle<()>>}
impl Capture {
 pub fn offer(&self,market:Market,event:&Event,received:u64){
  if matches!(event,Event::Status{..}|Event::Unknown){return;}
  self.counts.offered.fetch_add(1,Ordering::Relaxed);
  let row=serde_json::json!({"schemaVersion":1,"provider":"Massive","population":"subscribed-decoded-events-before-analysis-filters","receivedMs":received,"market":market,"event":event});
  match serde_json::to_vec(&row){Ok(mut bytes)=>{bytes.push(b'\n');if self.tx.try_send(bytes).is_err(){self.counts.dropped.fetch_add(1,Ordering::Relaxed);}},Err(_)=>{self.counts.dropped.fetch_add(1,Ordering::Relaxed);}}
 }
 pub fn health(&self)->serde_json::Value{serde_json::json!({"offered":self.counts.offered.load(Ordering::Relaxed),"written":self.counts.written.load(Ordering::Relaxed),"dropped":self.counts.dropped.load(Ordering::Relaxed),"writeFailed":self.counts.failed.load(Ordering::Relaxed),"storageLimitReached":self.counts.storage_limit.load(Ordering::Relaxed),"segments":self.counts.segments.load(Ordering::Relaxed),"retentionPolicy":"preserve_segments_no_2GiB_session_cap","completeMarketReceptionCertified":false})}
}
pub fn open(path:&Path,capacity:usize)->std::io::Result<(Capture,Writer)>{
 open_limited(path,capacity,64*1024*1024,usize::MAX)
}
#[cfg(windows)]fn disk_reserve_available(path:&Path)->bool{
 use std::os::windows::ffi::OsStrExt;
 #[link(name="kernel32")]unsafe extern "system"{fn GetDiskFreeSpaceExW(directory:*const u16,available:*mut u64,total:*mut u64,free:*mut u64)->i32;}
 let parent=path.parent().unwrap_or(Path::new("."));let wide:Vec<u16>=parent.as_os_str().encode_wide().chain(Some(0)).collect();let mut available=0;
 unsafe{GetDiskFreeSpaceExW(wide.as_ptr(),&mut available,std::ptr::null_mut(),std::ptr::null_mut())!=0&&available>=2*1024*1024*1024}
}
#[cfg(not(windows))]fn disk_reserve_available(_: &Path)->bool{true}
fn open_limited(path:&Path,capacity:usize,segment_limit:usize,max_segments:usize)->std::io::Result<(Capture,Writer)>{
 let path=path.to_path_buf();
 let mut file=OpenOptions::new().write(true).create_new(true).open(&path)?;
 let (tx,rx)=mpsc::sync_channel::<Vec<u8>>(capacity.max(1));let counts=Arc::new(Counts::default());counts.segments.store(1,Ordering::Relaxed);let worker_counts=counts.clone();let stop=Arc::new(AtomicBool::new(false));let worker_stop=stop.clone();
 let thread=std::thread::spawn(move||{
  let (mut bytes_written,mut segment)=(0usize,0usize);
  loop{match rx.recv_timeout(Duration::from_millis(100)){
   Ok(bytes)=>{
    if bytes.len()>segment_limit {worker_counts.storage_limit.store(true,Ordering::Relaxed);worker_counts.failed.store(true,Ordering::Relaxed);break;}
    if bytes_written+bytes.len()>segment_limit {
     segment+=1;if segment>=max_segments || !disk_reserve_available(&path) {worker_counts.dropped.fetch_add(1,Ordering::Relaxed);worker_counts.storage_limit.store(true,Ordering::Relaxed);worker_counts.failed.store(true,Ordering::Relaxed);break;}
     if file.sync_all().is_err(){worker_counts.failed.store(true,Ordering::Relaxed);break;}
     let next=path.with_extension(format!("{segment}.jsonl"));
     match OpenOptions::new().write(true).create_new(true).open(next){Ok(next)=>{file=next;worker_counts.segments.fetch_add(1,Ordering::Relaxed);},Err(_)=>{worker_counts.dropped.fetch_add(1,Ordering::Relaxed);worker_counts.failed.store(true,Ordering::Relaxed);break;}}
     bytes_written=0;
    }
    bytes_written+=bytes.len();if file.write_all(&bytes).is_err(){worker_counts.failed.store(true,Ordering::Relaxed);break;}let written=worker_counts.written.fetch_add(1,Ordering::Relaxed)+1;if written%256==0&&file.sync_data().is_err(){worker_counts.failed.store(true,Ordering::Relaxed);break;}},
   Err(mpsc::RecvTimeoutError::Timeout)=>{if file.sync_data().is_err(){worker_counts.failed.store(true,Ordering::Relaxed);break;}if worker_stop.load(Ordering::Relaxed){break;}},
   Err(mpsc::RecvTimeoutError::Disconnected)=>break,
  }}while rx.try_recv().is_ok(){worker_counts.dropped.fetch_add(1,Ordering::Relaxed);}if file.sync_all().is_err(){worker_counts.failed.store(true,Ordering::Relaxed);}
 });
 Ok((Capture{tx,counts},Writer{stop,thread:Some(thread)}))
}
impl Drop for Writer {fn drop(&mut self){self.stop.store(true,Ordering::Relaxed);if let Some(thread)=self.thread.take(){let _=thread.join();}}}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn durable_capture_preserves_decimals_and_excludes_auth(){
  let path=std::env::temp_dir().join(format!("snipers-capture-{}-{}.jsonl",std::process::id(),crate::live::now_ms()));
  let (capture,writer)=open(&path,4).unwrap();capture.offer(Market::Stocks,&Event::Status{status:"auth".into(),message:"must-not-be-persisted".into()},1);
  let event:Event=serde_json::from_value(serde_json::json!({"ev":"T","sym":"QQQ","p":100,"s":0,"ds":"0.002","t":1,"q":1})).unwrap();capture.offer(Market::Stocks,&event,2);drop(writer);
  let text=std::fs::read_to_string(&path).unwrap();assert!(!text.contains("must-not"));let row:serde_json::Value=serde_json::from_str(text.trim()).unwrap();assert_eq!(row["event"]["ds"],"0.002");assert_eq!(row["receivedMs"],2);assert_eq!(capture.health()["written"],1);
  assert!(open(&path,1).is_err());std::fs::remove_file(path).unwrap();
 }
}

#[cfg(test)]mod bounded_tests {
 use super::*;
 #[test]fn real_recording_rotates_past_32_segments_without_losing_events(){
  let input=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/audits/2026-10-12-auction-stream-recording.json");
  let recording=crate::recording::Recording::read(&input).unwrap();
  let dir=Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../docs/audits/2026-10-13-capture-rotation/real-rotation-{}-{}",std::process::id(),crate::live::now_ms()));std::fs::create_dir(&dir).unwrap();let path=dir.join("events.jsonl");
  let (capture,writer)=open_limited(&path,10001,16384,usize::MAX).unwrap();
  let rows:Vec<_>=recording.records.iter().filter(|r|!matches!(r.event,Event::Status{..}|Event::Unknown)).collect();
  for row in &rows{capture.offer(row.market,&row.event,row.received_ms);}drop(writer);
  let segments=capture.counts.segments.load(Ordering::Relaxed);assert!(segments>32);assert_eq!(capture.health()["writeFailed"],false);assert_eq!(capture.health()["dropped"],0);assert_eq!(capture.health()["written"],rows.len());
  let mut saved=vec![];for n in 0..segments{let file=if n==0{path.clone()}else{path.with_extension(format!("{n}.jsonl"))};for line in std::fs::read_to_string(file).unwrap().lines(){saved.push(serde_json::from_str::<serde_json::Value>(line).unwrap());}}
  assert_eq!(saved.len(),rows.len());for (actual,expected) in saved.iter().zip(&rows){assert_eq!(actual["event"],serde_json::to_value(&expected.event).unwrap());assert_eq!(actual["receivedMs"],expected.received_ms);}
  println!("{}",serde_json::json!({"realInput":input,"retainedEvidence":dir,"segments":segments,"events":saved.len(),"dropped":0,"scope":"Real retained recording with reduced segment size; not full market reception"}));
 }
 fn event()->Event{serde_json::from_value(serde_json::json!({"ev":"T","sym":"QQQ","p":100,"s":1,"t":1,"q":1})).unwrap()}
 #[test]fn saturated_queue_reports_every_dropped_offer(){
  let (tx,_rx)=mpsc::sync_channel(1);let capture=Capture{tx,counts:Arc::new(Counts::default())};capture.offer(Market::Stocks,&event(),1);capture.offer(Market::Stocks,&event(),2);assert_eq!(capture.health()["offered"],2);assert_eq!(capture.health()["dropped"],1);
 }
 #[test]fn storage_limit_is_explicit_and_never_overwrites_prior_evidence(){
  let path=std::env::temp_dir().join(format!("snipers-capture-limit-{}-{}.jsonl",std::process::id(),crate::live::now_ms()));let (capture,writer)=open_limited(&path,4,1,1).unwrap();capture.offer(Market::Stocks,&event(),2);drop(writer);assert_eq!(capture.health()["storageLimitReached"],true);assert_eq!(capture.health()["writeFailed"],true);assert_eq!(capture.health()["written"],0);std::fs::remove_file(path).unwrap();
 }
}
