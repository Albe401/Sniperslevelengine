use massive_feed::alpaca::{OrderIntent, PaperClient};
use tauri::Manager;
// One recovery pass at a time; never resends a POST, even after an HTTP 404.
static RECONCILING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[tauri::command]
pub async fn reconcile_alpaca_paper_orders(
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    use std::sync::atomic::Ordering;
    if RECONCILING.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una recuperación de órdenes en curso".into());
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            RECONCILING.store(false, Ordering::SeqCst);
        }
    }
    let _guard = Guard;
    let path = journal_path(&app)?;
    let read_path = path.clone();
    let ids = tauri::async_runtime::spawn_blocking(move || {
        snipers_store::broker_orders::BrokerOrders::open(&read_path)
            .and_then(|db| db.reconciliation_candidates())
    })
    .await
    .map_err(|_| "No se pudo leer el diario")?
    .map_err(|_| "No se pudieron recuperar las órdenes pendientes")?;
    let client = client().await?;
    let mut verified = 0;
    let mut errors = 0;
    for id in ids {
        let result = client.order(&id).await;
        let stop = result.is_err();
        if stop {
            errors += 1;
        } else {
            verified += 1;
        }
        let write_path = path.clone();
        tauri::async_runtime::spawn_blocking(move || {
            snipers_store::broker_orders::BrokerOrders::open(&write_path)
                .and_then(|db| db.observe(&id, result.as_ref().map_err(String::as_str)))
        })
        .await
        .map_err(|_| "No se pudo guardar la recuperación")?
        .map_err(|_| "No se pudo guardar la respuesta del broker")?;
        // Bound network failures/rate limits to one request per recovery pass.
        if stop {
            break;
        }
    }
    Ok(
        serde_json::json!({"verified":verified,"errors":errors,"batch_limit":20,"orders":get_alpaca_paper_outbox(app).await?}),
    )
}
fn journal_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Carpeta del diario no disponible")?;
    std::fs::create_dir_all(&dir).map_err(|_| "No se pudo preparar el diario del broker")?;
    Ok(dir.join("alpaca-paper.sqlite"))
}
#[tauri::command]
pub async fn get_alpaca_paper_outbox(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let path = journal_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let db = snipers_store::broker_orders::BrokerOrders::open(&path)
            .map_err(|_| "No se pudo abrir el diario del broker")?;
        db.recent()
            .map(serde_json::Value::from)
            .map_err(|_| "No se pudo leer el diario del broker".into())
    })
    .await
    .map_err(|_| "No se pudo consultar el diario del broker".to_owned())?
}

#[tauri::command]
pub async fn get_alpaca_option_quote(symbol: String) -> Result<serde_json::Value, String> {
    client().await?.option_quote(&symbol).await
}

async fn client() -> Result<PaperClient, String> {
    tauri::async_runtime::spawn_blocking(PaperClient::load)
        .await
        .map_err(|_| "No se pudieron cargar las credenciales Paper".to_owned())?
}
#[tauri::command]
pub async fn get_alpaca_paper_snapshot(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let mut snapshot = client().await?.snapshot().await?;
    // Failure to recover orders must remain visible without hiding account connectivity.
    snapshot["reconciliation"] = match reconcile_alpaca_paper_orders(app).await {
        Ok(v) => v,
        Err(e) => serde_json::json!({"error":e}),
    };
    Ok(snapshot)
}
#[tauri::command]
pub async fn get_alpaca_option_contracts(
    underlying: String,
    expiration: String,
    page_token: Option<String>,
) -> Result<serde_json::Value, String> {
    client()
        .await?
        .contracts(&underlying, &expiration, page_token.as_deref())
        .await
}
#[tauri::command]
pub async fn get_alpaca_paper_order(client_order_id: String) -> Result<serde_json::Value, String> {
    client().await?.order(&client_order_id).await
}
#[tauri::command]
pub async fn submit_alpaca_paper_order(
    app: tauri::AppHandle,
    intent: OrderIntent,
) -> Result<serde_json::Value, String> {
    submit_fresh(app, intent, None).await
}
pub async fn submit_fresh(
    app: tauri::AppHandle,
    intent: OrderIntent,
    quote_ms: Option<u64>,
) -> Result<serde_json::Value, String> {
    intent.validate()?;
    let client = client().await?;
    let path = journal_path(&app)?;
    let reserve_path = path.clone();
    let id = intent.client_order_id.clone();
    let reserve_id = id.clone();
    let payload = serde_json::to_string(&intent).map_err(|_| "No se pudo preparar la orden")?;
    let reserved = tauri::async_runtime::spawn_blocking(move || {
        snipers_store::broker_orders::BrokerOrders::open(&reserve_path)
            .and_then(|db| db.reserve(&reserve_id, &payload))
            .map_err(|_| "No se pudo persistir la intención; no se envió ninguna orden")
    })
    .await
    .map_err(|_| "No se pudo reservar la orden")??;
    if !reserved {
        return Err(
            "Este client_order_id ya está registrado. Consulta su estado antes de volver a operar"
                .into(),
        );
    }
    let outcome = client.submit_outcome(intent, quote_ms).await;
    let not_submitted = outcome.as_ref().err().is_some_and(|e| !e.attempted);
    let result = outcome.map_err(|e| e.message);
    let success = result.is_ok();
    let response = match &result {
        Ok(v) => v.to_string(),
        Err(e) => e.clone(),
    };
    tauri::async_runtime::spawn_blocking(move || {
        snipers_store::broker_orders::BrokerOrders::open(&path).and_then(|db| {
            if not_submitted {
                db.not_submitted(&id, &response)
            } else {
                db.finish(&id, &response, success)
            }
        })
    })
    .await
    .map_err(|_| "Estado de envío incierto: consulta la orden por client_order_id")?
    .map_err(|_| "No se pudo registrar la respuesta: consulta la orden por client_order_id")?;
    result
}
#[tauri::command]
pub async fn cancel_alpaca_paper_order(
    client_order_id: String,
) -> Result<serde_json::Value, String> {
    client().await?.cancel(&client_order_id).await
}
