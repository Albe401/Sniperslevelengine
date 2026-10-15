//! Offline JSON adapter: cargo run -p snipers-core --example auction_map -- input.json
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("Falta archivo Input JSON")?;
    let input = serde_json::from_slice(&std::fs::read(path)?)?;
    let result = snipers_core::auction_map::read(&input)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
