use std::{path::Path, sync::Arc};

use reth_beacon_consensus::BeaconConsensus;
use reth_blockchain_tree::{
    BlockchainTree, BlockchainTreeConfig, ShareableBlockchainTree,
    TreeExternals,
};
use reth_db::mdbx::{Env, WriteMap};
use reth_interfaces::{consensus::Consensus, Error as rethError};
use reth_primitives::{ChainSpec, ChainSpecBuilder};
use reth_provider::{providers::BlockchainProvider, ProviderFactory};
use reth_revm::Factory;
use std::sync::Mutex;

use crate::config::flags::SoflConfig;

use super::BcProviderBuilder;

type BcDB = Arc<Env<WriteMap>>;
type BcTree =
    ShareableBlockchainTree<Arc<Env<WriteMap>>, Arc<dyn Consensus>, Factory>;

pub type RethBcProvider = BlockchainProvider<BcDB, BcTree>;

static DB_ENV_SINGLETON: Mutex<Option<Arc<Env<WriteMap>>>> = Mutex::new(None);

impl BcProviderBuilder {
    pub fn default_db() -> Result<RethBcProvider, rethError> {
        let cfg = SoflConfig::load().unwrap();
        let datadir = Path::new(cfg.reth.datadir.as_str());
        Ok(BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap())
    }

    pub fn with_mainnet_reth_db(
        datadir: &Path,
    ) -> Result<RethBcProvider, rethError> {
        let chain_spec = Arc::new(ChainSpecBuilder::mainnet().build());
        let consensus = Arc::new(BeaconConsensus::new(chain_spec.clone()));
        Self::with_reth_db(chain_spec, consensus, datadir)
    }

    pub fn with_reth_db(
        chain_spec: Arc<ChainSpec>,
        consensus: Arc<dyn Consensus>,
        datadir: &Path,
    ) -> Result<RethBcProvider, rethError> {
        let mut maybe_db = DB_ENV_SINGLETON.lock().unwrap();
        let db;
        if maybe_db.is_none() {
            let db_inner = Env::<WriteMap>::open(
                &datadir.join("db"),
                reth_db::mdbx::EnvKind::RO,
            )?;
            db = Arc::new(db_inner);
            *maybe_db = Some(db.clone());
        } else {
            db = maybe_db.as_ref().unwrap().clone();
        }
        let executor_factory = Factory::new(chain_spec.clone());
        let tree_externals = TreeExternals::new(
            db.clone(),
            consensus,
            executor_factory,
            chain_spec.clone(),
        );
        let (sender, _) = tokio::sync::broadcast::channel(10);
        let blockchain_tree = BlockchainTree::new(
            tree_externals,
            sender,
            BlockchainTreeConfig::default(),
        )?;
        let shareable_blockchain_tree =
            ShareableBlockchainTree::new(blockchain_tree);
        let database = ProviderFactory::new(db, chain_spec);
        BlockchainProvider::new(database, shareable_blockchain_tree)
    }
}

#[cfg(test)]
mod tests_with_db {
    use std::{path::Path, sync::Arc};

    use crate::config::flags::SoflConfig;

    #[test]
    fn test_create_multiple_provider() {
        let cfg = SoflConfig::load().unwrap();
        let datadir = Path::new(cfg.reth.datadir.as_str());
        let provider1 =
            super::BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();
        let provider2 = super::BcProviderBuilder::with_mainnet_reth_db(datadir);
        let _ = Arc::new(provider1);
        assert!(provider2.is_ok());
    }
}
