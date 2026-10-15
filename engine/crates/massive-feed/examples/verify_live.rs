//! Bounded read-only check. Never logs credentials, requests, or raw provider errors.
use massive_feed::{credentials::Credentials, live};
#[tokio::main]
async fn main() {
    let key = match Credentials::load() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let state = live::new_state();
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(25),
        live::run(key, state.clone()),
    )
    .await;
    let snapshot = state.lock().unwrap();
    println!(
        "{}",
        serde_json::json!({"stocks":snapshot.stocks,"options":snapshot.options,"symbols":snapshot.ticks.len(),"bars":snapshot.bars.len()})
    );
}
