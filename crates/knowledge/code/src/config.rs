use alloy_chains::Chain;
use foundry_block_explorers::errors::EtherscanError;
use libsofl_utils::config::Config;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct CodeKnowledgeConfig {
    pub chain_id: u64,
    pub api_key: String,
}

impl Default for CodeKnowledgeConfig {
    fn default() -> Self {
        Self {
            chain_id: 1,
            api_key: "".to_string(),
        }
    }
}

impl Config for CodeKnowledgeConfig {
    fn section_name() -> &'static str {
        "code_knowledge"
    }
}

impl CodeKnowledgeConfig {
    pub fn get_client(
        &self,
    ) -> Result<foundry_block_explorers::Client, EtherscanError> {
        foundry_block_explorers::Client::new(
            Chain::from(self.chain_id),
            self.api_key.as_str(),
        )
    }
}