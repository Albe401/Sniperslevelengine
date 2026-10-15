//! Read-only REST check; does not open sockets or persist observations.
#[tokio::main]
async fn main() {
    let key = massive_feed::credentials::Credentials::load().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1)
    });
    let Some(spot) = std::env::args().nth(1).and_then(|s| s.parse::<f64>().ok()) else {
        eprintln!("Indica una referencia spot y un símbolo opcional (QQQ por defecto) para limitar el rango");
        std::process::exit(2)
    };
    let symbol = std::env::args().nth(2).unwrap_or_else(|| "QQQ".into());
    match massive_feed::option_snapshot::fetch(&key, &symbol, spot, 4).await {
        Ok(snapshot) => {
            let horizons = snipers_core::horizons::calculate(
                &snapshot,
                spot,
                "Referencia manual de diagnóstico",
            )
            .unwrap();
            println!(
                "{}",
                serde_json::json!({"exposure":horizons.iter().map(|h|serde_json::json!({"mode":h.mode,"contracts":h.exposure.contracts,"eligible":h.exposure.eligible,"gammaUsed":h.exposure.gamma_used,"deltaUsed":h.exposure.delta_used,"scanUsed":h.exposure.scan_used,"zeroGamma":h.exposure.zero_gamma})).collect::<Vec<_>>(),"multipliersVerified":snapshot.contracts.values().filter(|c|c.premium_multiplier.is_some()).count()})
            );
            println!(
                "{}",
                serde_json::json!({"symbol":snapshot.symbol,"observedDate":snapshot.observed_date,"contracts":snapshot.contracts.len(),"oiPresent":snapshot.contracts.values().filter(|c|c.open_interest.is_some()).count(),"gammaPresent":snapshot.contracts.values().filter(|c|c.gamma.is_some()).count(),"oiReferenceDate":snapshot.oi_reference_date,"expirations":snapshot.contracts.values().map(|c|c.expiration.clone()).collect::<std::collections::BTreeSet<_>>()})
            )
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1)
        }
    }
}
