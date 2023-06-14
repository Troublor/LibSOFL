//! This modules provides helper functions to instantiate variables reth providers.

use std::{path::Path, sync::Arc};

use reth_beacon_consensus::BeaconConsensus;
use reth_blockchain_tree::{
    BlockchainTree, BlockchainTreeConfig, ShareableBlockchainTree, TreeExternals,
};
use reth_db::mdbx::{Env, WriteMap};
use reth_interfaces::{consensus::Consensus, Error as rethError};
use reth_primitives::{ChainSpec, ChainSpecBuilder};
use reth_provider::{providers::BlockchainProvider, ShareableDatabase};
use reth_revm::Factory;

pub struct BlockchainProviderBuilder {
    chain_spec: Arc<ChainSpec>,
    consensus: Arc<dyn Consensus>,
}

impl BlockchainProviderBuilder {
    pub fn mainnet() -> Self {
        let chain_spec = Arc::new(ChainSpecBuilder::mainnet().build());
        let consensus = Arc::new(BeaconConsensus::new(chain_spec.clone()));
        Self {
            chain_spec,
            consensus,
        }
    }
}

impl Default for BlockchainProviderBuilder {
    fn default() -> Self {
        Self::mainnet()
    }
}

pub type BcDB = Arc<Env<WriteMap>>;
pub type BcTree = ShareableBlockchainTree<Arc<Env<WriteMap>>, Arc<dyn Consensus>, Factory>;

impl BlockchainProviderBuilder {
    pub fn with_existing_db(
        &self,
        datadir: &Path,
    ) -> Result<BlockchainProvider<BcDB, BcTree>, rethError> {
        let db = Env::<WriteMap>::open(&datadir.join("db"), reth_db::mdbx::EnvKind::RO)?;
        let db = Arc::new(db);
        let executor_factory = Factory::new(self.chain_spec.clone());
        let tree_externals = TreeExternals::new(
            db.clone(),
            self.consensus.clone(),
            executor_factory,
            self.chain_spec.clone(),
        );
        let (sender, _) = tokio::sync::broadcast::channel(10);
        let blockchain_tree =
            BlockchainTree::new(tree_externals, sender, BlockchainTreeConfig::default())?;
        let shareable_blockchain_tree = ShareableBlockchainTree::new(blockchain_tree);
        let database = ShareableDatabase::new(db, self.chain_spec.clone());
        BlockchainProvider::new(database, shareable_blockchain_tree)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{config::flags::SeeFuzzConfig, engine::builders::BlockchainProviderBuilder};

    #[test]
    fn test_build_blockchain_provider_from_reth_db() {
        let cfg = SeeFuzzConfig::load().unwrap();
        let datadir = Path::new(&cfg.reth.datadir);
        let maybe_provider = BlockchainProviderBuilder::mainnet().with_existing_db(datadir);
        assert!(maybe_provider.is_ok());
    }
}
