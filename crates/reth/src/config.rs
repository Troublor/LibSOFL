use std::path::Path;

use libsofl_core::error::SoflError;
use libsofl_utils::{
    config::{Config, ConfigLoader},
    log::info,
};

use crate::blockchain::provider::RethProvider;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RethConfig {
    pub datadir: String,
}

impl RethConfig {
    pub fn bc_provider(&self) -> Result<RethProvider, SoflError> {
        info!("loading bc provider with reth db from {}", self.datadir);
        let datadir = Path::new(&self.datadir);
        RethProvider::from_db(datadir)
    }

    pub fn must_load() -> RethConfig {
        ConfigLoader::load_cfg(CONFIG_SECTION).expect("failed to load config")
    }

    pub fn load() -> Result<RethConfig, SoflError> {
        ConfigLoader::load_cfg(CONFIG_SECTION)
    }
}

pub static CONFIG_SECTION: &str = "reth";

impl Config for RethConfig {}
