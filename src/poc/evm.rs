use reth_beacon_consensus::BeaconConsensus;
use reth_blockchain_tree::{
    BlockchainTree, BlockchainTreeConfig, ShareableBlockchainTree, TreeExternals,
};
use reth_db::database::{Database, DatabaseGAT};
use reth_db::mdbx::tx::Tx;
use reth_db::mdbx::{Env, WriteMap, RO, RW};
use reth_db::transaction::DbTx;
use reth_interfaces::consensus::Consensus;
use reth_primitives::{
    BlockHashOrNumber, BlockId, BlockNumberOrTag, Chain, ChainSpec, ChainSpecBuilder, H160, H256,
    U256,
};
use reth_provider::providers::BlockchainProvider;
use reth_provider::{
    EvmEnvProvider, HistoricalStateProvider, LatestStateProvider, LatestStateProviderRef,
    ShareableDatabase, StateProvider, StateProviderFactory, Transaction, TransactionsProvider,
};
use reth_revm::database::{State, SubState};
use reth_revm::env::fill_tx_env;
use reth_revm::Factory;
use reth_rpc::eth::error::EthApiError;
use reth_staged_sync::utils::init::init_genesis;
use revm::{Database as revmDatabase, DatabaseCommit as revmDatabaseCommit};
use revm_primitives::{BlockEnv, CfgEnv, Env as revmEnv, TxEnv};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tracing::*;

/// Opens up an existing database or creates a new one at the specified path.
pub fn init_db<P: AsRef<Path>>(path: P) -> eyre::Result<Env<WriteMap>> {
    std::fs::create_dir_all(path.as_ref())?;
    let db = Env::<WriteMap>::open(path.as_ref(), reth_db::mdbx::EnvKind::RW)?;
    db.create_tables()?;
    Ok(db)
}

#[tokio::test]
async fn test_state_provider() {
    let chain_spec = ChainSpecBuilder::mainnet().build();
    let chain_spce = Arc::new(chain_spec.clone());
    let datadir = Path::new("./blockchain");

    let db = init_db(datadir).unwrap();
    let db = Arc::new(db);
    init_genesis(db.clone(), chain_spce.clone()).unwrap();
    let db_tx = Transaction::new(db.as_ref()).unwrap();
    let provider = LatestStateProviderRef::new(&*db_tx);

    let addr = H160::from_str("0xDAFEA492D9c6733ae3d56b7Ed1ADB60692c98Bc5").unwrap();
    let bal = provider.account_balance(addr);
    assert_eq!(bal, Ok(None));
}

#[tokio::test]
async fn test_existing_db() {
    let datadir = Path::new("/ssddata/wzhangcb/blockchain/mainnet-reth/db");
    if !datadir.exists() {
        panic!("{} does not exist", datadir.display());
    }
    let db = Env::<WriteMap>::open(datadir, reth_db::mdbx::EnvKind::RO).unwrap();
    let ro_dbtx = db.tx().unwrap();
    let bn = 16000000_u64;
    let provider = HistoricalStateProvider::new(ro_dbtx, bn);
    let acc = H160::from_str("0x4D9079Bb4165aeb4084c526a32695dCfd2F77381").unwrap();
    let slot = H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000000")
        .unwrap();
    let storage = provider.storage(acc, slot).unwrap();
    assert!(storage.is_some());
    assert_eq!(
        storage.unwrap(),
        U256::from_str("0x0000000000000000000000010000000000000000000000000000000000000000")
            .unwrap()
    );
}

#[tokio::test]
async fn test_reproduce_tx() {
    let chain_spec = ChainSpecBuilder::mainnet().build();
    let chain_spec = Arc::new(chain_spec);
    let datadir = Path::new("/ssddata/wzhangcb/blockchain/mainnet-reth/db");
    if !datadir.exists() {
        panic!("{} does not exist", datadir.display());
    }
    let db = Env::<WriteMap>::open(datadir, reth_db::mdbx::EnvKind::RO).unwrap();
    let db = Arc::new(db);
    let consensus: Arc<dyn Consensus> = Arc::new(BeaconConsensus::new(chain_spec.clone()));
    let executor_factory = Factory::new(chain_spec.clone());
    let tree_externals =
        TreeExternals::new(db.clone(), consensus, executor_factory, chain_spec.clone());
    let (sender, mut canon_notif) = tokio::sync::broadcast::channel(10);
    let blockchain_tree =
        BlockchainTree::new(tree_externals, sender, BlockchainTreeConfig::default()).unwrap();
    let shareable_blockchain_tree = ShareableBlockchainTree::new(blockchain_tree);
    let database = ShareableDatabase::new(db.clone(), chain_spec.clone());
    let blockchain_provider = BlockchainProvider::new(database, shareable_blockchain_tree).unwrap();
    let state_provider = blockchain_provider
        .state_by_block_id(BlockId::from(16000000_u64))
        .unwrap();

    let tx_hash =
        H256::from_str("0x05e9769544c867897687bdceb217056ae081925c1f1c23590bf6dfabb9c45002")
            .unwrap();
    let signed_tx = blockchain_provider
        .transaction_by_hash(tx_hash)
        .unwrap()
        .unwrap();
    let mut tx_env = TxEnv::default();
    let signer = signed_tx
        .recover_signer()
        .ok_or_else(|| EthApiError::InvalidTransactionSignature)
        .unwrap();
    fill_tx_env(&mut tx_env, signed_tx, signer);

    let next_bn = BlockHashOrNumber::from(16000001_u64);
    let mut cfg = CfgEnv::default();
    let mut block_env = BlockEnv::default();
    blockchain_provider
        .fill_env_at(&mut cfg, &mut block_env, next_bn)
        .unwrap();

    let env = revmEnv {
        cfg,
        block: block_env,
        tx: tx_env,
    };
    let evm_db = SubState::new(State::new(state_provider));
    let mut evm = revm::EVM::with_env(env);
    evm.database(evm_db);
    let res = evm.transact().unwrap();
    let evm_db = evm.db.as_mut().unwrap();
    evm_db.commit(res.state);

    let contract = H160::from_str("0x87B1d1B59725209879CC5C5adEb99d8BC9EcCf12").unwrap();
    let slot = U256::from_str("0x0000000000000000000000000000000000000000000000000000000000000000")
        .unwrap();
    let expected =
        U256::from_str("0x00010000b400b4005302071000000000000002ffec2a28260553fac2e4aa3bf4")
            .unwrap();
    let actual = evm_db.storage(contract, slot).unwrap();
    assert_eq!(actual, expected);
}
