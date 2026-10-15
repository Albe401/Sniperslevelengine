use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum AssetKind {
    Stock,
    Etf,
    Index,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssetConfig {
    pub name: String,
    pub kind: AssetKind,
    pub level_range: f64,
    pub quick_range: f64,
    pub live_range: f64,
    pub sector: Option<String>,
    pub role: String,
}
#[derive(Deserialize)]
struct Registry {
    assets: BTreeMap<String, AssetConfig>,
}
pub fn assets() -> &'static BTreeMap<String, AssetConfig> {
    static ASSETS: OnceLock<BTreeMap<String, AssetConfig>> = OnceLock::new();
    ASSETS.get_or_init(|| {
        serde_json::from_str::<Registry>(include_str!("../../../fixtures/legacy-universe.json"))
            .expect("Invalid bundled asset registry")
            .assets
    })
}
pub fn stock_asset(symbol: &str) -> Option<&'static AssetConfig> {
    assets()
        .get(symbol)
        .filter(|asset| matches!(asset.kind, AssetKind::Stock | AssetKind::Etf))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indices_never_silently_use_stock_feed() {
        assert!(stock_asset("QQQ").is_some());
        assert!(stock_asset("SPY").is_some());
        assert!(stock_asset("SPX").is_none());
        assert!(stock_asset("UNKNOWN").is_none());
        assert_eq!(assets()["QQQ"].live_range, 5.0);
        assert!(assets().len() > 76 && assets().len() < 500);
        assert!(stock_asset("BRK.B").is_some());
        assert!(stock_asset("COST").is_some());
        assert!(stock_asset("HOOD").is_some());
        assert!(stock_asset("AAON").is_none());
        assert_eq!(assets()["HOOD"].sector.as_deref(), Some("FINANCIERO"));
    }
}
