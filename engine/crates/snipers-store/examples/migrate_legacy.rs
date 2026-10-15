//! Explicit one-time copy/import. Never mutates the legacy source.
use std::{error::Error, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err("Usage: migrate_legacy SOURCE BACKUP_NEW RUST_DATABASE_NEW".into());
    }
    if args[2].exists() {
        return Err("Destination already exists; refusing replacement".into());
    }
    snipers_store::backup_legacy(&args[0], &args[1])?;
    drop(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&args[2])?,
    );
    let mut ledger = snipers_store::Ledger::open(&args[2])?;
    let report = ledger.import_legacy(&args[1])?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}
