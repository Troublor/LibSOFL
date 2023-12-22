use libsofl_core::error::SoflError;
use libsofl_utils::config::{Config, ConfigLoader};

#[derive(Debug, Clone, Eq, PartialEq, Default, serde::Deserialize, serde::Serialize)]
pub struct JsonRpcConfig {
    pub url: String,
}

impl Config for JsonRpcConfig {}

impl JsonRpcConfig {
    pub fn load() -> Result<Self, SoflError> {
        ConfigLoader::load_cfg("jsonrpc")
    }

    pub fn must_load() -> Self {
        ConfigLoader::load_cfg_or_default("jsonrpc", Self::default())
            .expect("failed to load jsonrpc config")
    }
}

impl JsonRpcConfig {
    // pub fn bc_provider() -> JsonRpcProvider {
    //     todo!()
    // }
}
