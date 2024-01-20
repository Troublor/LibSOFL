use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use libsofl_core::{
    blockchain::{
        provider::{BcProvider, BcStateProvider},
        transaction::Tx,
        tx_position::TxPosition,
    },
    engine::{
        inspector::no_inspector,
        memory::MemoryBcState,
        state::BcState,
        transition::TransitionSpecBuilder,
        types::{
            BlockEnv, BlockHash, BlockHashOrNumber, BlockNumber, CfgEnv, TxEnv,
            TxHashOrPosition,
        },
    },
    error::SoflError,
};
use reth_beacon_consensus::BeaconConsensus;
use reth_blockchain_tree::{
    BlockchainTree, ShareableBlockchainTree, TreeExternals,
};
use reth_db::{open_db_read_only, DatabaseEnv};
use reth_primitives::revm::env::fill_tx_env;
use reth_primitives::ChainSpecBuilder;
pub use reth_provider::{
    providers::BlockchainProvider, BlockHashReader, BlockNumReader,
    BlockchainTreePendingStateProvider, ChainSpecProvider, EvmEnvProvider,
    HeaderProvider, HistoricalStateProvider, ProviderFactory, ReceiptProvider,
    ReceiptProviderIdExt, StateProvider, StateProviderBox,
    StateProviderFactory, StateRootProvider, TransactionsProvider,
    TransactionsProviderExt,
};
use reth_revm::{database::StateProviderDatabase, EvmProcessorFactory};

use crate::conversion::ConvertTo;

use super::transaction::RethTx;

pub type RethBlockchainProvider = BlockchainProvider<
    Arc<DatabaseEnv>,
    ShareableBlockchainTree<Arc<DatabaseEnv>, EvmProcessorFactory>,
>;

lazy_static! {
    /// Global caches of the reth db instance per datadir.
    static ref DB_CACHE: Mutex<HashMap<String, Arc<DatabaseEnv>>> = Mutex::new(HashMap::default());
}

#[derive(Clone)]
pub struct RethProvider {
    pub bp: RethBlockchainProvider,
}

impl RethProvider {
    pub fn from_db(datadir: &Path) -> Result<Self, SoflError> {
        let chain_spec = Arc::new(ChainSpecBuilder::mainnet().build());
        let consensus = Arc::new(BeaconConsensus::new(chain_spec.clone()));

        let mut db_cache = DB_CACHE.lock().unwrap();
        let datadir_str: String = datadir.to_string_lossy().into();
        let maybe_db = db_cache.get(&datadir_str);
        let db;
        if maybe_db.is_none() {
            let db_inner =
                open_db_read_only(datadir.join("db").as_path(), None).map_err(
                    |e| {
                        SoflError::Provider(format!("failed to open db: {}", e))
                    },
                )?;
            db = Arc::new(db_inner);
            db_cache.insert(datadir_str, db.clone());
        } else {
            db = maybe_db.unwrap().clone();
        }

        let provider_factory =
            ProviderFactory::new(db.clone(), chain_spec.clone());
        let executor_factory = EvmProcessorFactory::new(chain_spec.clone());
        let tree_externals =
            TreeExternals::new(provider_factory, consensus, executor_factory);
        let blockchain_tree =
            BlockchainTree::new(tree_externals, Default::default(), None)
                .map_err(|e| {
                    SoflError::Provider(format!(
                        "failed to create blockchain tree: {}",
                        e
                    ))
                })?;
        let shareable_blockchain_tree =
            ShareableBlockchainTree::new(blockchain_tree);
        let database = ProviderFactory::new(db, chain_spec);
        let bp: BlockchainProvider<
            Arc<DatabaseEnv>,
            ShareableBlockchainTree<Arc<DatabaseEnv>, EvmProcessorFactory>,
        > = BlockchainProvider::new(database, shareable_blockchain_tree)
            .map_err(|e| {
                SoflError::Provider(format!(
                    "failed to create blockchain provider: {}",
                    e
                ))
            })?;
        Ok(Self { bp })
    }
}

impl BcStateProvider<StateProviderDatabase<StateProviderBox>> for RethProvider {
    /// Create a BcState from the the state before the transaction at the position is executed.
    fn bc_state_at(
        &self,
        pos: TxPosition,
    ) -> Result<MemoryBcState<StateProviderDatabase<StateProviderBox>>, SoflError>
    {
        let bn = match pos.block {
            BlockHashOrNumber::Hash(hash) => self
                .bp
                .block_number(hash)
                .map_err(|e| {
                    SoflError::Provider(format!(
                        "failed to get block number: {}",
                        e
                    ))
                })?
                .ok_or(SoflError::NotFound(format!("block {}", hash)))?,
            BlockHashOrNumber::Number(n) => n,
        };
        let sp = self.bp.state_by_block_id((bn - 1).into()).map_err(|e| {
            SoflError::Provider(format!(
                "failed to create reth state provider: {}",
                e
            ))
        })?;
        let wrapped = StateProviderDatabase::new(sp);
        let mut state = MemoryBcState::new(wrapped);

        // execute proceeding transactions
        if pos.index > 0 {
            let txs = self
                .bp
                .transactions_by_block(pos.block.cvt())
                .map_err(|e| {
                    SoflError::Provider(format!(
                        "failed to get transactions by block: {}",
                        e
                    ))
                })?
                .ok_or(SoflError::NotFound(format!("position {}", pos)))?;
            let txs: Vec<RethTx> = txs
                .into_iter()
                .take(pos.index as usize)
                .map(move |t| t.into())
                .collect();
            // prepare
            let mut spec_builder =
                TransitionSpecBuilder::new().at_block(self.clone(), pos.block);
            for tx in txs.into_iter() {
                spec_builder = spec_builder.append_tx(tx);
            }
            let spec = spec_builder.build();
            state.transit(spec, no_inspector())?;
        }

        Ok(state)
    }
}

impl BcProvider<RethTx> for RethProvider {
    fn tx(&self, tx: TxHashOrPosition) -> Result<RethTx, SoflError> {
        let hash = match tx {
            TxHashOrPosition::Hash(hash) => hash,
            TxHashOrPosition::Position(pos) => {
                let txs = self
                    .bp
                    .transactions_by_block(pos.block.cvt())
                    .map_err(|e| {
                        SoflError::Provider(format!(
                            "failed to get transactions by block: {}",
                            e
                        ))
                    })?;
                txs.map(|mut s| s.remove(pos.index as usize))
                    .ok_or(SoflError::NotFound(format!("transaction {}", pos)))?
                    .hash()
            }
        };
        RethTx::from_hash(&self.bp, hash)
    }

    fn txs_in_block(
        &self,
        block: BlockHashOrNumber,
    ) -> Result<Vec<RethTx>, SoflError> {
        let txs = self
            .bp
            .transactions_by_block(block.cvt())
            .map_err(|e| {
                SoflError::Provider(format!(
                    "failed to get transactions by block: {}",
                    e
                ))
            })?
            .ok_or(SoflError::NotFound(format!("block {}", block)))?;
        let txs: Result<Vec<RethTx>, _> =
            txs.into_iter().map(|t| self.tx(t.hash().into())).collect();
        let txs = txs.map_err(|e| {
            SoflError::Provider(format!("failed to get transaction: {}", e))
        })?;
        Ok(txs)
    }

    fn fill_cfg_env(
        &self,
        env: &mut CfgEnv,
        block: BlockHashOrNumber,
    ) -> Result<(), SoflError> {
        self.bp.fill_cfg_env_at(env, block.cvt()).map_err(|e| {
            SoflError::Provider(format!("failed to fill cfg env: {}", e))
        })
    }

    fn fill_block_env(
        &self,
        env: &mut BlockEnv,
        block: BlockHashOrNumber,
    ) -> Result<(), SoflError> {
        self.bp.fill_block_env_at(env, block.cvt()).map_err(|e| {
            SoflError::Provider(format!("failed to fill block env: {}", e))
        })
    }

    fn fill_tx_env(
        &self,
        env: &mut TxEnv,
        tx: TxHashOrPosition,
    ) -> Result<(), SoflError> {
        let tx = self.tx(tx)?;
        let sender = tx.sender();
        fill_tx_env(env, Box::new(tx.tx.transaction), sender.cvt());
        Ok(())
    }

    fn block_number_by_hash(
        &self,
        hash: BlockHash,
    ) -> Result<BlockNumber, SoflError> {
        self.bp
            .block_number(hash)
            .map_err(|e| {
                SoflError::Provider(format!(
                    "failed to get block number: {}",
                    e
                ))
            })?
            .ok_or(SoflError::NotFound(format!("block {}", hash)))
    }

    fn block_hash_by_number(
        &self,
        number: BlockNumber,
    ) -> Result<BlockHash, SoflError> {
        self.bp
            .block_hash(number)
            .map_err(|e| {
                SoflError::Provider(format!("failed to get block hash: {}", e))
            })?
            .ok_or(SoflError::NotFound(format!("block {}", number)))
    }

    fn chain_id(&self) -> u64 {
        self.bp.chain_spec().chain.id()
    }
}

#[cfg(test)]
mod tests_with_db {
    use std::{path::Path, sync::Arc};

    use libsofl_core::{
        blockchain::{
            provider::{BcProvider, BcStateProvider},
            tx_position::TxPosition,
        },
        conversion::ConvertTo,
        engine::{
            inspector::no_inspector, state::BcState,
            transition::TransitionSpec, types::TxHash,
        },
    };
    use libsofl_utils::config::Config;
    use reth_provider::ReceiptProvider;

    use crate::config::RethConfig;

    #[test]
    fn test_create_provider() {
        let cfg = RethConfig::must_load();
        let bp = cfg.bc_provider().unwrap();
        let h = bp.block_hash_by_number(1).unwrap();
        let h_str: String = h.cvt();
        assert_eq!(
            h_str,
            "0x88e96d4537bea4d9c05d12549907b32561d3bf31f45aae734cdc119f13406cb6"
        )
    }

    #[test]
    fn test_create_multiple_provider() {
        let cfg = RethConfig::must_load();
        let datadir = Path::new(cfg.datadir.as_str());
        let provider1 = super::RethProvider::from_db(datadir).unwrap();
        let provider2 = super::RethProvider::from_db(datadir);
        let _ = Arc::new(provider1);
        assert!(provider2.is_ok());
    }

    #[test]
    fn test_reproduce_tx() {
        let cfg = RethConfig::must_load();
        let bp = cfg.bc_provider().unwrap();
        let fork_at = TxPosition::new(17000000, 0);

        // prepare state
        let mut state = bp.bc_state_at(fork_at).unwrap();

        // collect the tx
        let tx_hash: TxHash =
            "0xa278205118a242c87943b9ed83aacafe9906002627612ac3672d8ea224e38181".cvt();
        let spec = TransitionSpec::from_tx_hash(&bp, tx_hash).unwrap();

        // simulate
        let r = state.transit(spec, no_inspector()).unwrap().pop().unwrap();
        // let r = BcState::dry_run(&state, spec, no_inspector())
        //     .unwrap()
        //     .pop()
        // .unwrap();
        assert!(r.is_success());
        let receipt = bp.bp.receipt_by_hash(tx_hash).unwrap().unwrap();
        assert_eq!(receipt.success, r.is_success());
        assert_eq!(receipt.logs.len(), r.logs().len());
        for (log, receipt_log) in r.logs().iter().zip(receipt.logs.iter()) {
            assert_eq!(log.address, receipt_log.address);
            assert_eq!(log.topics, receipt_log.topics);
            assert_eq!(*log.data, *receipt_log.data);
        }
        assert_eq!(receipt.cumulative_gas_used, r.gas_used());
    }
}
