//! SQLite calls stay off the async network path and outside market-state locks.
use massive_feed::live::{self, SharedState};
use snipers_store::Ledger;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
pub struct SelectionStorage {
    bot_path: PathBuf,
    bot_saved: Option<String>,
    ledger: Arc<Mutex<Ledger>>,
    date: String,
    saved: Option<(Vec<String>, u64)>,
    pub error: Option<String>,
    retry_ms: u64,
}

impl SelectionStorage {
    pub async fn open(directory: PathBuf, state: &SharedState) -> Result<Self, String> {
        let bot_path=directory.join("alpaca-paper.sqlite");
        let ledger = tauri::async_runtime::spawn_blocking(move || {
            std::fs::create_dir_all(&directory).map_err(|_| "No se pudo abrir la carpeta local")?;
            Ledger::open(&directory.join("snipers-v1.sqlite"))
                .map_err(|_| "No se pudo abrir el almacén de selección")
        })
        .await
        .map_err(|_| "No se pudo iniciar SQLite")??;
        let mut this = Self {
            bot_path,bot_saved:None,
            ledger: Arc::new(Mutex::new(ledger)),
            date: String::new(),
            saved: None,
            error: None,
            retry_ms: 0,
        };
        this.restore(state, live::now_ms()).await?;
        Ok(this)
    }
    async fn restore(&mut self, state: &SharedState, now: u64) -> Result<(), String> {
        let date = snipers_core::calendar::date_at(now)
            .ok_or("Fecha ET no disponible")?
            .to_string();
        if self.date == date {
            return Ok(());
        }
        state
            .lock()
            .map_err(|_| "Estado no disponible")?
            .scanner_store
            .selection_storage_ready = false;
        let ledger = self.ledger.clone();
        let request_date = date.clone();
        let saved = tauri::async_runtime::spawn_blocking(move || {
            ledger
                .lock()
                .map_err(|_| "Almacén ocupado")?
                .load_selection(&request_date)
                .map_err(|_| "No se pudo recuperar la selección")
        })
        .await
        .map_err(|_| "No se pudo recuperar SQLite")??;
        let bot_path=self.bot_path.clone();let bot_date=date.clone();
        let stored_bot=tauri::async_runtime::spawn_blocking(move||snipers_store::broker_orders::BrokerOrders::open(&bot_path).and_then(|db|db.load_bot_universe(&bot_date))).await.map_err(|_|"Recuperación Top 5 fallida")?.map_err(|_|"No se pudo recuperar Top 5 del bot")?;
        let mut s = state.lock().map_err(|_| "Estado no disponible")?;
        if let Some((symbols, time)) = &saved {
            if !s
                .scanner_store
                .restore_selection(symbols.clone(), *time, now)
            {
                return Err("La selección guardada no corresponde a una sesión válida".into());
            }
        }
        if let Some(payload)=stored_bot {
            let saved=serde_json::from_str(&payload).map_err(|_|"Top 5 guardado inválido")?;
            if !s.scanner_store.bot_selection.restore(saved,now){return Err("Top 5 guardado no pertenece a esta sesión".into());}
            self.bot_saved=Some(date.clone());
        } else {self.bot_saved=None;}
        s.scanner_store.selection_storage_ready = true;
        self.date = date;
        self.saved = saved;
        self.error = None;
        Ok(())
    }
    pub async fn before_snapshot(&mut self, state: &SharedState) {
        let now = live::now_ms();
        if now < self.retry_ms {
            return;
        }
        if let Err(error) = self.restore(state, now).await {
            self.error = Some(error);
            self.retry_ms = now + 5000;
        }
    }
    pub async fn save(&mut self, snapshot: &mut live::LiveState) {
        let bot=&snapshot.scanner.bot_selection;
        if bot.verified_ms.is_some()&&self.bot_saved.as_ref()!=Some(&bot.session) {
            let path=self.bot_path.clone();let date=bot.session.clone();let payload=serde_json::to_string(bot).unwrap();
            let result=tauri::async_runtime::spawn_blocking(move||snipers_store::broker_orders::BrokerOrders::open(&path).and_then(|db|db.save_bot_universe(&date,&payload))).await;
            if matches!(result,Ok(Ok(()))){self.bot_saved=Some(bot.session.clone());}
            else {snapshot.scanner.storage_error=Some("Top 5 del bot pendiente de guardar".into());}
        }
        let selection = &snapshot.scanner.selection;
        let key = selection.locked_at.map(|t| (selection.symbols.clone(), t));
        snapshot.scanner.selection_saved = selection.locked && key == self.saved;
        if !selection.locked || key == self.saved || live::now_ms() < self.retry_ms {
            snapshot.scanner.storage_error = self.error.clone();
            return;
        }
        let Some((symbols, time)) = key.clone() else {
            return;
        };
        let ledger = self.ledger.clone();
        let date = selection.session.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            ledger
                .lock()
                .map_err(|_| "Almacén ocupado")?
                .save_selection(&date, &symbols, time)
                .map_err(|_| "No se pudo guardar la selección fija")
        })
        .await;
        match result {
            Ok(Ok(())) => {
                self.saved = key;
                self.error = None;
                self.retry_ms = 0;
            }
            _ => {
                self.error = Some("Selección sin guardar; recuperación pendiente".into());
                self.retry_ms = live::now_ms() + 5000;
            }
        }
        snapshot.scanner.storage_error = self.error.clone();
        snapshot.scanner.selection_saved = self.error.is_none() && self.saved.is_some();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn restores_before_scanning_and_reports_conflicting_write_without_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("selection.sqlite");
        let now = snipers_core::calendar::legacy_eastern_ms("2026-09-24 10:00:00").unwrap();
        let mut ledger = Ledger::open(&path).unwrap();
        ledger
            .save_selection("2026-09-24", &["QQQ".into(), "SPY".into()], now - 1000)
            .unwrap();
        let mut storage = SelectionStorage {
            bot_path:temp.path().join("paper.sqlite"),bot_saved:None,
            ledger: Arc::new(Mutex::new(ledger)),
            date: String::new(),
            saved: None,
            error: None,
            retry_ms: 0,
        };
        let state = live::new_state();
        storage.restore(&state, now).await.unwrap();
        let report = state.lock().unwrap().scanner_store.snapshot(now, false);
        assert_eq!(report.selection.symbols, ["QQQ", "SPY"]);
        assert!(report.top.is_empty());
        let mut snapshot = live::LiveState::default();
        snapshot.scanner = report;
        storage.save(&mut snapshot).await;
        assert!(snapshot.scanner.selection_saved);
        snapshot.scanner.selection.symbols = vec!["SPY".into()];
        storage.save(&mut snapshot).await;
        assert!(!snapshot.scanner.selection_saved);
        assert!(snapshot.scanner.storage_error.is_some());
        assert_eq!(
            Ledger::open(&path)
                .unwrap()
                .load_selection("2026-09-24")
                .unwrap()
                .unwrap()
                .0,
            ["QQQ", "SPY"]
        );
        storage.restore(&state, now + 86_400_000).await.unwrap();
        assert!(state
            .lock()
            .unwrap()
            .scanner_store
            .snapshot(now + 86_400_000, false)
            .selection
            .symbols
            .is_empty());
    }
}
