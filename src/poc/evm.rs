use reth_beacon_consensus::BeaconConsensus;
use reth_db::database::{Database, DatabaseGAT};
use reth_db::mdbx::tx::Tx;
use reth_db::mdbx::{Env, WriteMap, RO, RW};
use reth_db::transaction::DbTx;
use reth_interfaces::consensus::Consensus;
use reth_primitives::{Chain, ChainSpec, ChainSpecBuilder, H160};
use reth_provider::{
    LatestStateProvider, LatestStateProviderRef, ShareableDatabase, StateProvider, Transaction,
};
use reth_revm::database::{State, SubState};
use reth_staged_sync::utils::init::init_genesis;
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
    db_tx.get_block_hash()
    let provider = LatestStateProviderRef::new(&*db_tx);

    let addr = H160::from_str("0xDAFEA492D9c6733ae3d56b7Ed1ADB60692c98Bc5").unwrap();
    let bal = provider.account_balance(addr);
    assert_eq!(bal, Ok(None));
}

async fn test_tx() {
    let chain_spec = ChainSpecBuilder::mainnet().build();
    let chain_spec = Arc::new(chain_spec.clone());
    let datadir = Path::new("./blockchain");

    let db = init_db(datadir).unwrap();
    let db = Arc::new(db);
    init_genesis(db.clone(), chain_spec.clone()).unwrap();
    let shareable_db = ShareableDatabase::new(db.as_ref(), chain_spec);
    let state = shareable_db.latest().unwrap();

    let mut db = SubState::new(State::new(state));
    let cfg = CfgEnv::default();
    let block = BlockEnv::default();
    let tx = TxEnv::default();
    let env = revmEnv { cfg, block, tx };
    let mut evm = revm::EVM::with_env(env);
    evm.database(db);
}
