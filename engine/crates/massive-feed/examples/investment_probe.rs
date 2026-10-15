//! Read-only integration check. Optional output path stores the public market report as UTF-8.
use massive_feed::investment::{scan, Config};
use std::sync::{atomic::AtomicBool, Arc};
#[tokio::main]
async fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let config = Config {
        symbols: args
            .first()
            .filter(|s| s.as_str() != "ALL")
            .cloned()
            .unwrap_or_default(),
        limit: 30,
        ..Default::default()
    };
    match scan(config, Arc::new(AtomicBool::new(false)), |r| {
        if !r.current.is_empty() {
            eprintln!("{}/{} {}", r.completed, r.total, r.current);
        }
    })
    .await
    {
        Ok(r) => {
            let json = serde_json::to_string_pretty(&r).unwrap();
            if let Some(path) = args.get(1) {
                std::fs::write(path, json).expect("Cannot save report");
                println!(
                    "{} companies; {} ratio records; report saved",
                    r.completed, r.universe
                );
            } else {
                println!("{json}");
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
