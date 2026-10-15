//! Explicit opt-in, single-session protocol diagnostic, NOT the production engine.
//! cargo run -p massive-feed --example stream -- stocks delayed A.QQQ
use futures_util::{SinkExt, StreamExt};
use massive_feed::{decode_batch, endpoint, subscription, Event, Feed, Market};
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: stream stocks|options realtime|delayed channel[,channel]".into());
    }
    let market = match args[0].as_str() {
        "stocks" => Market::Stocks,
        "options" => Market::Options,
        _ => return Err("Invalid market".into()),
    };
    let feed = match args[1].as_str() {
        "realtime" => Feed::Realtime,
        "delayed" => Feed::Delayed,
        _ => return Err("Invalid feed".into()),
    };
    let channels: Vec<_> = args[2].split(',').map(str::to_owned).collect();
    let subscribe = subscription(market, &channels)?;
    let key = std::env::var("MASSIVE_API_KEY")
        .map_err(|_| "Set MASSIVE_API_KEY in the Rust process environment")?;
    if key.trim().is_empty() {
        return Err("MASSIVE_API_KEY is empty".into());
    }
    let (mut socket, _) = tokio::time::timeout(
        Duration::from_secs(15),
        connect_async(endpoint(market, feed)),
    )
    .await??;
    socket
        .send(Message::Text(
            serde_json::json!({"action":"auth", "params":key})
                .to_string()
                .into(),
        ))
        .await?;
    let mut authenticated = false;
    let auth_deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let frame = if authenticated {
            socket.next().await
        } else {
            tokio::time::timeout_at(auth_deadline, socket.next()).await?
        };
        let Some(frame) = frame else {
            break;
        };
        match frame? {
            Message::Text(text) => {
                for event in decode_batch(text.as_ref())? {
                    match &event {
                        Event::Status { status, .. }
                            if status == "auth_success" && !authenticated =>
                        {
                            authenticated = true;
                            socket.send(Message::Text(subscribe.clone().into())).await?;
                            println!("Authenticated; subscription requested, awaiting provider response.");
                        }
                        Event::Status { status, .. }
                            if ["auth_failed", "not_authorized", "error", "max_connections"]
                                .contains(&status.as_str()) =>
                        {
                            // Do not echo raw provider payloads that might contain credentials.
                            return Err(format!("Provider rejected request ({status})").into());
                        }
                        Event::Status { status, .. } => println!("Provider status: {status}"),
                        _ if authenticated => println!("{event:?}"),
                        _ => {}
                    }
                }
            }
            Message::Ping(payload) => socket.send(Message::Pong(payload)).await?,
            Message::Close(_) => break,
            _ => {}
        }
    }
    Err("Connection ended; diagnostic does not reconnect or recover missed events".into())
}
