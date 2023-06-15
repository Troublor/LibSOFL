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

use super::BcProviderBuilder;

type BcDB = Arc<Env<WriteMap>>;
type BcTree = ShareableBlockchainTree<Arc<Env<WriteMap>>, Arc<dyn Consensus>, Factory>;

pub type RethBcProvider = BlockchainProvider<BcDB, BcTree>;

impl BcProviderBuilder {
    pub fn with_mainnet_reth_db(datadir: &Path) -> Result<RethBcProvider, rethError> {
        let chain_spec = Arc::new(ChainSpecBuilder::mainnet().build());
        let consensus = Arc::new(BeaconConsensus::new(chain_spec.clone()));
        Self::with_reth_db(chain_spec, consensus, datadir)
    }

    pub fn with_reth_db(
        chain_spec: Arc<ChainSpec>,
        consensus: Arc<dyn Consensus>,
        datadir: &Path,
    ) -> Result<RethBcProvider, rethError> {
        let db = Env::<WriteMap>::open(&datadir.join("db"), reth_db::mdbx::EnvKind::RO)?;
        let db = Arc::new(db);
        let executor_factory = Factory::new(chain_spec.clone());
        let tree_externals =
            TreeExternals::new(db.clone(), consensus, executor_factory, chain_spec.clone());
        let (sender, _) = tokio::sync::broadcast::channel(10);
        let blockchain_tree =
            BlockchainTree::new(tree_externals, sender, BlockchainTreeConfig::default())?;
        let shareable_blockchain_tree = ShareableBlockchainTree::new(blockchain_tree);
        let database = ShareableDatabase::new(db, chain_spec);
        BlockchainProvider::new(database, shareable_blockchain_tree)
    }
}
