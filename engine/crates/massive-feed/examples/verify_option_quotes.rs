//! Prints only market-data evidence. Optional report path; no credentials or raw errors.
#[tokio::main]
async fn main() -> Result<(), String> {
    let report = massive_feed::option_quote_diagnostic::run().await?;
    let text =
        serde_json::to_string_pretty(&report).map_err(|_| "No se pudo preparar el informe")?;
    if let Some(path) = std::env::args().nth(1) {
        std::fs::write(path, &text).map_err(|_| "No se pudo guardar el informe")?;
    }
    println!("{text}");
    Ok(())
}
