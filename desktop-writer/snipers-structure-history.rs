//! Bounded read-only cache input for the frozen web price-structure kernel.
use massive_feed::{credentials::Credentials,price_structure_history};
#[tokio::main]
async fn main()->std::process::ExitCode {
 let result=async {
  let mut args=std::env::args().skip(1);
  let symbol=args.next().ok_or(())?;
  let destination=args.next().ok_or(())?;
  if symbol.is_empty()||symbol.len()>15||!symbol.bytes().all(|c|c.is_ascii_uppercase()||c.is_ascii_digit()||c==b'.'){return Err(());}
  let key=Credentials::load().map_err(|_|())?;
  let history=price_structure_history::fetch(&key,&symbol,massive_feed::live::now_ms()).await.map_err(|_|())?;
  let bytes=serde_json::to_vec(&history).map_err(|_|())?;
  std::fs::write(destination,bytes).map_err(|_|())?;
  Ok::<(),()>(())
 }.await;
 if result.is_ok(){std::process::ExitCode::SUCCESS}else{eprintln!("Structure history unavailable; no substitute data.");std::process::ExitCode::FAILURE}
}
