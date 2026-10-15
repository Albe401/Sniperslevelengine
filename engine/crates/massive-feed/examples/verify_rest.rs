use massive_feed::{credentials::Credentials, live};
#[tokio::main]
async fn main() {
    let key = match Credentials::load() {
        Ok(key) => key,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };
    match live::verify_rest(&key).await {
        Ok(contracts) => println!("REST Stocks + Options: acceso verificado; {contracts} contratos QQQ seleccionados. WebSocket no probado para no competir con Python."),
        Err(message) => { eprintln!("{message}"); std::process::exit(1); }
    }
}
