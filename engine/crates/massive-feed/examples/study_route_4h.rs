//! Usage: cargo run -p massive-feed --example study_route_4h -- QQQ 2026-04-01 2026-10-04 <directory>
#[tokio::main]
async fn main() {
    let a: Vec<_> = std::env::args().skip(1).collect();
    if a.len() != 4 {
        eprintln!("Uso: symbol from to output-directory");
        std::process::exit(2)
    }
    let dir = std::path::PathBuf::from(&a[3]);
    match massive_feed::route_4h_history::study(&a[0], &a[1], &a[2], &dir.join("minutes")).await {
        Ok(report) => {
            let path = dir.join(format!("{}-4h-study.json", a[0]));
            let bytes = serde_json::to_vec_pretty(&report).unwrap();
            if std::fs::write(&path, bytes).is_err() {
                eprintln!("No se pudo guardar estudio");
                std::process::exit(1)
            }
            println!(
                "{}",
                serde_json::json!({"symbol":a[0],"closedBlocks":report.quality.closed_blocks,"outcomes":report.outcomes.len(),"summaries":report.summaries,"probabilityClaim":report.probability_claim,"report":path})
            );
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    }
}
