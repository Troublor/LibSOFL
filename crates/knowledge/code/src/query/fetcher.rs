use std::sync::Arc;

use foundry_block_explorers::{contract::Metadata, errors::EtherscanError};
use libsofl_core::engine::types::Address;
use tokio::sync::RwLock;

/// Fetch data from the block explorer (e.g., Etherscan).
/// Fetcher is cloneable, but the rate limit is shared among clones.
pub struct Fetcher {
    pub client: foundry_block_explorers::Client,
    pub rate_limit: Arc<RwLock<libsofl_utils::rate_limit::RateLimit>>,
}

impl Fetcher {
    pub fn new(
        cfg: &crate::config::CodeKnowledgeConfig,
    ) -> Result<Self, EtherscanError> {
        let client = cfg.get_client()?;
        let rate_limit = cfg.get_rate_limit();
        let rate_limit = RwLock::new(rate_limit);
        let rate_limit = Arc::new(rate_limit);
        Ok(Self { client, rate_limit })
    }
}

impl Clone for Fetcher {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            rate_limit: self.rate_limit.clone(),
        }
    }
}

impl Fetcher {
    pub async fn fetch_verified_code(
        &self,
        address: Address,
    ) -> Result<Metadata, EtherscanError> {
        self.rate_limit
            .write()
            .await
            .wait_and_increment_async()
            .await;
        let mut meta = self.client.contract_source_code(address).await?;
        assert_eq!(meta.items.len(), 1, "expected 1 item");
        Ok(meta.items.remove(0))
    }
}
