use std::path::Path;

use libsofl_core::error::SoflError;
use libsofl_utils::{config::Config, log::info};

use crate::blockchain::provider::RethProvider;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RethConfig {
    pub datadir: String,
}

impl Default for RethConfig {
    fn default() -> Self {
        Self {
            datadir: "blockchain".to_string(),
        }
    }
}

impl RethConfig {
    pub fn bc_provider(&self) -> Result<RethProvider, SoflError> {
        info!("loading bc provider with reth db from {}", self.datadir);
        let datadir = Path::new(&self.datadir);
        RethProvider::from_db(datadir)
    }
}

impl Config for RethConfig {
    fn section_name() -> &'static str {
        "reth"
    }
}
