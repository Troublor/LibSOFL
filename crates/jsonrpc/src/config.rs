use libsofl_core::error::SoflError;
use libsofl_utils::config::Config;

use crate::provider::JsonRpcProvider;

#[derive(
    Debug, Clone, Eq, PartialEq, Default, serde::Deserialize, serde::Serialize,
)]
pub struct JsonRpcConfig {
    pub url: String,
}

impl Config for JsonRpcConfig {
    fn section_name() -> &'static str {
        "jsonrpc"
    }
}

impl JsonRpcConfig {
    pub fn bc_provider(&self) -> Result<JsonRpcProvider, SoflError> {
        JsonRpcProvider::new(self.url.clone())
    }
}
