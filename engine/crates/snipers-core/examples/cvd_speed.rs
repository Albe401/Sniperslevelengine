//! Offline throughput check; no network, broker, or orders.
fn main() {
 let mut engine=snipers_core::cvd::Engine::default();
 let start_ms=snipers_core::calendar::legacy_eastern_ms("2026-10-05 10:00:00").unwrap();
 let clock=std::time::Instant::now();
 for i in 0..200_000u64 {
  let at=start_ms+i;
  engine.quote(99.99,100.,at);
  engine.trade("NVDA",if i%2==0{100.}else{99.99},10.,at,Some(i+1),&[0],at);
  if i%200==0{std::hint::black_box(engine.snapshot("NVDA",at,true));}
 }
 let elapsed=clock.elapsed();let read=engine.snapshot("NVDA",start_ms+199_999,true);
 assert_eq!(read.regular.buy,1_000_000.);assert_eq!(read.regular.sell,1_000_000.);
 assert_eq!(read.regular.delta,0.);assert_eq!(read.rejected,0);
 println!("200000 trades + quotes, 1000 snapshots: {:.2} ms; {:.2} us/event; {:.0} events/s",elapsed.as_secs_f64()*1000.,elapsed.as_secs_f64()*1e6/200_000.,200_000./elapsed.as_secs_f64());
}
