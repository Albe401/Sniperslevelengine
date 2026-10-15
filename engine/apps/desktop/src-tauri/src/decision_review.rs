use massive_feed::live;
use snipers_store::Ledger;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub struct Reviewer {
    entries_enabled: bool,
    legacy_enabled: bool,
    ledger: Arc<Mutex<Ledger>>,
    next_ms: u64,
    last: serde_json::Value,
}
impl Reviewer {
    pub async fn held_symbol(&self) -> Result<Option<String>, String> {
        let ledger = self.ledger.clone();
        tauri::async_runtime::spawn_blocking(move || {
            ledger
                .lock()
                .map_err(|_| "Coordinador no disponible".to_owned())?
                .open_position_symbol()
                .map_err(|_| "No se pudo recuperar la posición abierta".to_owned())
        })
        .await
        .map_err(|_| "No se pudo consultar la posición".to_owned())?
    }
    pub async fn open(
        directory: PathBuf,
        entries_enabled: bool,
        legacy_enabled: bool,
    ) -> Result<Self, String> {
        let ledger = tauri::async_runtime::spawn_blocking(move || {
            Ledger::open(&directory.join("snipers-v1.sqlite"))
                .map_err(|_| "No se pudo recuperar el coordinador de decisiones".to_owned())
        })
        .await
        .map_err(|_| "No se pudo iniciar el coordinador")??;
        Ok(Self {
            entries_enabled,
            legacy_enabled,
            ledger: Arc::new(Mutex::new(ledger)),
            next_ms: 0,
            last: serde_json::Value::Null,
        })
    }
    pub async fn resume_legacy(&self) -> Result<Vec<String>, String> {
        if !self.legacy_enabled {
            return Ok(Vec::new());
        }
        let ledger = self.ledger.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let positions = ledger
                .lock()
                .map_err(|_| "Diario no disponible")?
                .preview_legacy_positions(live::now_ms())
                .map_err(|_| "No se pudieron recuperar íntegramente las posiciones anteriores")?;
            let mut symbols = Vec::new();
            for position in positions {
                live::new_state_for(&position.symbol).map_err(str::to_owned)?;
                if !symbols.contains(&position.symbol) {
                    symbols.push(position.symbol);
                }
            }
            Ok(symbols)
        })
        .await
        .map_err(|_| "No se pudo preparar el traspaso")?
    }
    pub async fn update(&mut self, snapshot: &mut live::LiveState) {
        let now = live::now_ms();
        if now >= self.next_ms {
            let rows: Vec<_> = snapshot
                .structural_paths
                .iter()
                .map(|p| p.evidence.clone())
                .collect();
            let symbol = snapshot.symbol.clone();
            let position_symbol = snapshot
                .held_symbol
                .clone()
                .unwrap_or_else(|| symbol.clone());
            let verified = snapshot.stocks.phase == "streaming";
            let price = snapshot
                .ticks
                .get(&position_symbol)
                .or(snapshot.held_tick.as_ref())
                .filter(|t| t.symbol == position_symbol)
                .and_then(|t| t.price.zip(t.trade_ms));
            let ledger = self.ledger.clone();
            let entries_enabled = self.entries_enabled;
            let legacy_enabled = self.legacy_enabled;
            let legacy_prices = snapshot
                .legacy_symbols
                .iter()
                .filter_map(|symbol| {
                    snapshot
                        .ticks
                        .get(symbol)
                        .or_else(|| snapshot.legacy_ticks.get(symbol))
                        .filter(|t| &t.symbol == symbol)
                        .and_then(|t| t.price.zip(t.trade_ms))
                        .map(|price| (symbol.clone(), price))
                })
                .collect();
            let result = tauri::async_runtime::spawn_blocking(move || {
                let mut ledger = ledger.lock().map_err(|_| ())?;
                let legacy_positions = if legacy_enabled {
                    if verified {
                        ledger
                            .resume_legacy_positions(live::now_ms())
                            .map_err(|_| ())?;
                    }
                    ledger
                        .manage_legacy_positions(&legacy_prices, live::now_ms(), verified)
                        .map_err(|_| ())?
                } else {
                    ledger.legacy_position_states().map_err(|_| ())?
                };
                let cycle = ledger
                    .coordinate(
                        &snipers_store::CycleInput {
                            symbol: &symbol,
                            position_symbol: &position_symbol,
                            position_price: price,
                            source_verified: verified,
                            evidence: &rows,
                            entries_enabled,
                        },
                        live::now_ms(),
                    )
                    .map_err(|_| ())?;
                let mut value = serde_json::to_value(cycle).map_err(|_| ())?;
                value["positionHistory"] =
                    serde_json::to_value(ledger.position_history(&symbol, 20).map_err(|_| ())?)
                        .map_err(|_| ())?;
                value["legacyPositions"] =
                    serde_json::to_value(legacy_positions).map_err(|_| ())?;
                value["legacyManagementEnabled"] = legacy_enabled.into();
                value["positionSymbol"] = position_symbol.into();
                Ok::<_, ()>(value)
            })
            .await;
            self.last = match result {
                Ok(Ok(review)) => review,
                _ => {
                    serde_json::json!({"reason":"STORAGE_UNAVAILABLE","entriesEnabled":false,"reviewedMs":now,"symbol":snapshot.symbol})
                }
            };
            self.next_ms = now.saturating_add(1000);
        }
        snapshot.decision_review = self.last.clone();
    }
}
