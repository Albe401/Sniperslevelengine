//! Offline study history, separate from live chart transport. Explicit requests only.
use crate::{
    credentials::Credentials,
    route_4h::{self, Minute, Study},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
#[derive(Serialize, Deserialize)]
struct Archive {
    version: String,
    symbol: String,
    from: u64,
    to: u64,
    minutes: Vec<Minute>,
}
pub async fn study(symbol: &str, from: &str, to: &str, directory: &Path) -> Result<Study, String> {
    if snipers_core::universe::stock_asset(symbol).is_none() {
        return Err("Activo no admitido".into());
    }
    let (start, _) = snipers_core::calendar::day_bounds(from).ok_or("Fecha inicial inválida")?;
    let (_, end) = snipers_core::calendar::day_bounds(to).ok_or("Fecha final inválida")?;
    let now = crate::live::now_ms();
    let today = snipers_core::calendar::date_at(now)
        .ok_or("Calendario desconocido")?
        .to_string();
    let today_start = snipers_core::calendar::day_bounds(&today).unwrap().0;
    if from.len() != 10
        || to.len() != 10
        || from < "2026-01-01"
        || end > today_start
        || start >= end
        || end - start > 366 * 86_400_000
    {
        return Err(
            "Selecciona hasta un año completo, dentro del calendario 2026–2028 y hasta ayer".into(),
        );
    }
    std::fs::create_dir_all(directory).map_err(|_| "No se pudo crear archivo local")?;
    let mut all = BTreeMap::new();
    let mut cursor = start;
    let mut key = None;
    while cursor < end {
        let through = (cursor + 30 * 86_400_000).min(end) - 1;
        let path = directory.join(format!("{symbol}-{cursor}-{through}-v1.json"));
        let cached = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.len() < 64 * 1024 * 1024)
            .and_then(|_| std::fs::read(&path).ok())
            .and_then(|b| serde_json::from_slice::<Archive>(&b).ok())
            .filter(|a| {
                a.version == route_4h::VERSION
                    && a.symbol == symbol
                    && a.from == cursor
                    && a.to == through
                    && a.minutes.len() <= 50000
                    && a.minutes
                        .iter()
                        .all(|b| b.timestamp_ms >= cursor && b.timestamp_ms <= through)
            });
        let minutes = if let Some(a) = cached {
            a.minutes
        } else {
            if key.is_none() {
                key = Some(Credentials::load().map_err(str::to_string)?)
            }
            let bars = crate::history::fetch(key.as_ref().unwrap(), symbol, cursor, through)
                .await
                .map_err(str::to_string)?;
            let minutes: Vec<_> = bars.iter().map(Minute::from).collect();
            let archive = Archive {
                version: route_4h::VERSION.into(),
                symbol: symbol.into(),
                from: cursor,
                to: through,
                minutes: minutes.clone(),
            };
            let bytes =
                serde_json::to_vec(&archive).map_err(|_| "No se pudo serializar historial")?;
            let temp = path.with_extension("tmp");
            std::fs::write(&temp, bytes).map_err(|_| "No se pudo guardar historial")?;
            if path.exists() {
                std::fs::remove_file(&path).map_err(|_| "No se pudo renovar historial")?
            }
            std::fs::rename(temp, path).map_err(|_| "No se pudo finalizar historial")?;
            minutes
        };
        for b in minutes {
            if all.insert(b.timestamp_ms, b).is_some() {
                return Err("Historial duplicado; estudio rechazado".into());
            }
        }
        cursor = through + 1;
    }
    let minutes: Vec<_> = all.into_values().collect();
    let split = start + (end - start) * 7 / 10;
    let symbol = symbol.to_owned();
    tokio::task::spawn_blocking(move || route_4h::study(&symbol, &minutes, end, split))
        .await
        .map_err(|_| "No se pudo evaluar el estudio".into())
}
