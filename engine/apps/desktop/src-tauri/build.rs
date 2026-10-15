use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::Path,
};

fn fingerprint(path: &Path, hash: &mut DefaultHasher) {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .expect("source directory")
            .map(|e| e.expect("source entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            fingerprint(&entry, hash);
        }
    } else {
        path.file_name().hash(hash);
        std::fs::read(path).expect("source bytes").hash(hash);
    }
}

fn main() {
    // Build identity for comparing observations; not a security signature.
    let base = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = base.join("../../..");
    let mut hash = DefaultHasher::new();
    for relative in [
        "Cargo.lock",
        "crates/snipers-core/src",
        "crates/snipers-core/resources",
        "crates/massive-feed/src",
        "crates/snipers-store/src",
        "apps/desktop/src",
        "apps/desktop/src-tauri/src",
    ] {
        let path = root.join(relative);
        println!("cargo:rerun-if-changed={}", path.display());
        relative.hash(&mut hash);
        fingerprint(&path, &mut hash);
    }
    println!(
        "cargo:rustc-env=SNIPERS_ENGINE_BUILD={:016x}",
        hash.finish()
    );
    tauri_build::build()
}
