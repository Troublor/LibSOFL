use std::sync::{atomic::AtomicI32, Arc};

use alloy_chains::Chain;
use foundry_block_explorers::{contract::Metadata, errors::EtherscanError};
use jsonrpsee::tracing::debug;
use libsofl_core::engine::types::Address;
use libsofl_utils::rate_limit::RateLimit;
use tokio::sync::RwLock;

use crate::config::CodeKnowledgeConfig;

/// Fetch data from the block explorer (e.g., Etherscan).
/// Fetcher is cloneable, but the rate limit is shared among clones.
pub struct Fetcher {
    pub client: foundry_block_explorers::Client,
    pub rate_limit: Arc<RwLock<libsofl_utils::rate_limit::RateLimit>>,
}

impl Fetcher {
    pub fn new(
        chain_id: u64,
        api_key: &str,
        rate_limit: RateLimit,
    ) -> Result<Self, EtherscanError> {
        // let client = cfg.get_client()?;
        // let rate_limit = cfg.get_rate_limit();
        let client = foundry_block_explorers::Client::new(
            Chain::from(chain_id),
            api_key,
        )?;
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

pub struct MultiplexedFetcher {
    fetchers: Vec<Fetcher>,
    index: AtomicI32,
}

impl MultiplexedFetcher {
    pub fn new(cfg: &CodeKnowledgeConfig) -> Self {
        let fetchers = cfg
            .api_keys
            .iter()
            .map(|api_key| {
                Fetcher::new(cfg.chain_id, api_key, cfg.get_rate_limit())
                    .expect("failed to create fetcher")
            })
            .collect();
        Self {
            fetchers,
            index: AtomicI32::new(0),
        }
    }
}

impl Clone for MultiplexedFetcher {
    fn clone(&self) -> Self {
        Self {
            fetchers: self.fetchers.clone(),
            index: AtomicI32::new(0),
        }
    }
}

impl MultiplexedFetcher {
    pub async fn fetch_verified_code(
        &self,
        address: Address,
    ) -> Result<Metadata, EtherscanError> {
        let index =
            self.index.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let len = self.fetchers.len();
        let fetcher = &self.fetchers[index as usize % len];
        debug!(address = address.to_string(), "fetching contract code");
        let r = fetcher.fetch_verified_code(address).await;
        match r {
            Ok(m) => {
                debug!(address = address.to_string(), "fetched contract code");
                Ok(m)
            }
            Err(err) => {
                debug!(address = address.to_string(), err = ?err, "failed to fetch contract code");
                Err(err)
            }
        }
    }
}
