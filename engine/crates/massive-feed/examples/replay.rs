//! Offline only: cargo run -p massive-feed --example replay -- path/to/recording.json
fn main() {
    let Some(path) = std::env::args_os().nth(1) else {
        eprintln!("Indica el archivo de registro exportado por Snipers.");
        std::process::exit(2);
    };
    match massive_feed::recording::Recording::read(std::path::Path::new(&path))
        .and_then(|recording| recording.replay())
    {
        Ok(report) => println!("{report}"),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}
