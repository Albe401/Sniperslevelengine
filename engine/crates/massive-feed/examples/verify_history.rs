use massive_feed::{credentials::Credentials, history, live};
#[tokio::main]
async fn main() {
    let key = match Credentials::load() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    };
    let symbol = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "QQQ".into())
        .to_uppercase();
    let state = live::new_state_for(&symbol).unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2)
    });
    history::recover(&key, &state).await;
    let snapshot = live::snapshot(&state);
    println!(
        "{}",
        serde_json::json!({"symbol":symbol,"history":snapshot.history,"metrics":snapshot.metrics,"chartBars":snapshot.minutes.len(),"websocketOpened":false})
    );
    if snapshot.history.phase != "ready" {
        std::process::exit(1);
    }
}
