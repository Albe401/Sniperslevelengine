fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        serde_json::to_string_pretty(&massive_feed::clock_diagnostic::inspect()?)?
    );
    Ok(())
}
