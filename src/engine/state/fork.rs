use std::{fmt::Debug, ops::Deref, ops::DerefMut, sync::Arc};

use reth_provider::{
    EvmEnvProvider, StateProviderBox, StateProviderFactory,
    TransactionsProvider,
};
use reth_revm::database::State as WrappedDB;
use revm::{db::CacheDB, Database, DatabaseCommit};
use revm_primitives::{
    db::DatabaseRef, Account, AccountInfo, Address, BlockEnv, Bytecode, CfgEnv,
    HashMap, B160, B256, B256 as H256, U256,
};

use crate::{
    engine::{inspectors::no_inspector, transactions::position::TxPosition},
    error::SoflError,
};

use super::{BcState, DatabaseEditable};

/// Abstraction of the forked state in revm that can be cloned.
/// This type implements both BcState and BcStateGround
#[derive(Clone)]
pub struct ForkedBcState<'a>(InnerForkedBcState<'a>);

pub type InnerForkedBcState<'a> = CacheDB<Arc<WrappedDB<StateProviderBox<'a>>>>;

impl Debug for ForkedBcState<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForkedBcState").finish()
    }
}

impl<'a> Deref for ForkedBcState<'a> {
    type Target = InnerForkedBcState<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> DerefMut for ForkedBcState<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> AsRef<InnerForkedBcState<'a>> for ForkedBcState<'a> {
    fn as_ref(&self) -> &InnerForkedBcState<'a> {
        &self.0
    }
}

impl<'a> AsMut<InnerForkedBcState<'a>> for ForkedBcState<'a> {
    fn as_mut(&mut self) -> &mut InnerForkedBcState<'a> {
        todo!()
    }
}

impl<'a> From<InnerForkedBcState<'a>> for ForkedBcState<'a> {
    fn from(st: InnerForkedBcState<'a>) -> Self {
        Self(st)
    }
}

impl<'a> From<ForkedBcState<'a>> for InnerForkedBcState<'a> {
    fn from(st: ForkedBcState<'a>) -> Self {
        st.0
    }
}

impl<'a> ForkedBcState<'a> {
    pub fn new(st: CacheDB<Arc<WrappedDB<StateProviderBox<'a>>>>) -> Self {
        Self(st)
    }

    /// fork from the current latest blockchain state
    pub fn latest<P: StateProviderFactory>(
        p: &'a P,
    ) -> Result<Self, SoflError<<Self as BcState>::DbErr>> {
        let sp = p.latest().map_err(SoflError::Reth)?;
        let wrapped = WrappedDB::new(sp);
        let state = CacheDB::new(Arc::new(wrapped));
        Ok(Self::new(state))
    }

    /// Create a forked state from the the state before the transaction at the position is executed.
    pub fn fork_at<
        P: StateProviderFactory + EvmEnvProvider + TransactionsProvider,
    >(
        p: &'a P,
        pos: TxPosition,
    ) -> Result<Self, SoflError<<Self as BcState>::DbErr>> {
        let pos_cp = pos.clone();
        let bn = pos
            .get_block_number(p)
            .map_err(|_| SoflError::Fork(pos_cp))?;
        let sp = p
            .state_by_block_id((bn - 1).into())
            .map_err(SoflError::Reth)?;
        let wrapped = WrappedDB::new(sp);
        let state = CacheDB::new(Arc::new(wrapped));

        let mut this = Self::new(state);

        // execute proceeding transactions
        if pos.index > 0 {
            let txs = p
                .transactions_by_block(pos.block)
                .map_err(SoflError::Reth)?;
            // prepare env
            let mut evm_cfg = CfgEnv::default();
            let mut block_env = BlockEnv::default();
            p.fill_env_at(&mut evm_cfg, &mut block_env, pos.block)
                .map_err(SoflError::Reth)?;

            // fork error if the fork position block does not exist
            let pos_cp = pos.clone();
            let txs = txs.ok_or(SoflError::Fork(pos_cp))?;
            for tx in txs.into_iter().take(pos.index as usize) {
                let r = this.transact(
                    evm_cfg.clone(),
                    block_env.clone(),
                    tx,
                    no_inspector(),
                )?;
                this.commit(r.state);
            }
        }
        Ok(this)
    }

    /// Create a forked state from the the state after the transaction at the position is executed.
    pub fn fork_from<
        P: StateProviderFactory + EvmEnvProvider + TransactionsProvider,
    >(
        p: &'a P,
        pos: TxPosition,
    ) -> Result<Self, SoflError<<Self as BcState>::DbErr>> {
        let mut pos_mut = pos.clone();
        pos_mut.shift(p, 1).map_err(|_| SoflError::Fork(pos))?;
        Self::fork_at(p, pos_mut)
    }
}

impl<'a> BcState for ForkedBcState<'a> {
    type DbErr = reth_interfaces::Error;
}

impl<'a> DatabaseEditable for ForkedBcState<'a> {
    type Error = reth_interfaces::Error;

    fn insert_account_storage(
        &mut self,
        address: Address,
        slot: U256,
        value: U256,
    ) -> Result<(), Self::Error> {
        self.0.insert_account_storage(address, slot, value)
    }

    fn insert_account_info(&mut self, address: B160, mut info: AccountInfo) {
        self.0.insert_account_info(address, info)
    }
}

/// Delegate as revm Database
impl<'a> Database for ForkedBcState<'a> {
    type Error = reth_interfaces::Error;

    #[doc = " Get basic account information."]
    fn basic(
        &mut self,
        address: Address,
    ) -> Result<Option<AccountInfo>, Self::Error> {
        self.0.basic(address)
    }

    #[doc = " Get account code by its hash"]
    fn code_by_hash(
        &mut self,
        code_hash: H256,
    ) -> Result<revm_primitives::Bytecode, Self::Error> {
        self.0.code_by_hash(code_hash)
    }

    #[doc = " Get storage value of address at index."]
    fn storage(
        &mut self,
        address: Address,
        index: U256,
    ) -> Result<U256, Self::Error> {
        self.0.storage(address, index)
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        self.0.block_hash(number)
    }
}

impl<'a> DatabaseCommit for ForkedBcState<'a> {
    fn commit(&mut self, changes: HashMap<B160, Account>) {
        self.0.commit(changes)
    }
}
impl<'a> DatabaseRef for ForkedBcState<'a> {
    type Error = reth_interfaces::Error;

    #[doc = " Whether account at address exists."]
    #[doc = " Get basic account information."]
    fn basic(&self, address: B160) -> Result<Option<AccountInfo>, Self::Error> {
        self.0.basic(address)
    }

    #[doc = " Get account code by its hash"]
    fn code_by_hash(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.0.code_by_hash(code_hash)
    }

    #[doc = " Get storage value of address at index."]
    fn storage(&self, address: B160, index: U256) -> Result<U256, Self::Error> {
        self.0.storage(address, index)
    }

    fn block_hash(&self, number: U256) -> Result<B256, Self::Error> {
        self.0.block_hash(number)
    }
}

#[cfg(test)]
mod tests_with_db {
    use std::path::Path;

    use reth_provider::{
        EvmEnvProvider, ReceiptProvider, TransactionsProvider,
    };
    use revm_primitives::{BlockEnv, CfgEnv, ExecutionResult};

    use crate::{
        config::flags::SoflConfig,
        engine::{
            inspectors::no_inspector, providers::BcProviderBuilder,
            state::BcState, transactions::position::TxPosition,
        },
    };

    use super::ForkedBcState;

    #[test]
    fn test_reproduce_block() {
        let datadir = SoflConfig::load().unwrap().reth.datadir;
        let datadir = Path::new(&datadir);
        let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();
        let fork_at = TxPosition::new(17000000, 0);
        let txs = bp.transactions_by_block(fork_at.block).unwrap().unwrap();
        let receipts = bp.receipts_by_block(fork_at.block).unwrap().unwrap();

        // prepare state
        let state = ForkedBcState::fork_at(&bp, fork_at.clone()).unwrap();

        // prepare cfg and env
        let mut cfg = CfgEnv::default();
        let mut block_env = BlockEnv::default();
        bp.fill_env_at(&mut cfg, &mut block_env, fork_at.block)
            .unwrap();

        // execute
        let (_, results) =
            state.transit(cfg, block_env, txs, no_inspector()).unwrap();

        assert_eq!(results.len(), receipts.len());

        for (result, receipt) in results.iter().zip(receipts.iter()) {
            match result {
                ExecutionResult::Success { logs, .. } => {
                    assert!(receipt.success);
                    assert_eq!(receipt.logs.len(), logs.len());
                    for (log, receipt_log) in
                        logs.iter().zip(receipt.logs.iter())
                    {
                        assert_eq!(log.address, receipt_log.address);
                        assert_eq!(log.topics, receipt_log.topics);
                        assert_eq!(*log.data, *receipt_log.data);
                    }
                }
                _ => assert!(!receipt.success),
            }
        }
    }
}

#[cfg(test)]
mod tests_with_jsonrpc {
    use reth_provider::{
        EvmEnvProvider, ReceiptProvider, TransactionsProvider,
    };
    use revm_primitives::{BlockEnv, CfgEnv};

    use crate::{
        config::flags::SoflConfig,
        engine::{
            inspectors::no_inspector,
            providers::BcProviderBuilder,
            state::{fork::ForkedBcState, BcState},
            transactions::position::TxPosition,
        },
        utils::conversion::{Convert, ToPrimitive},
    };

    #[test]
    fn test_reproduce_tx() {
        let cfg = SoflConfig::load().unwrap();
        let url = cfg.jsonrpc.endpoint.clone();
        let bp = BcProviderBuilder::with_jsonrpc_via_http_with_auth(
            url,
            cfg.jsonrpc,
        )
        .unwrap();
        let fork_at = TxPosition::new(17000000, 0);

        // prepare state
        let mut state = ForkedBcState::fork_at(&bp, fork_at.clone()).unwrap();

        // prepare env and state
        let mut cfg = CfgEnv::default();
        let mut block_env = BlockEnv::default();
        bp.fill_env_at(&mut cfg, &mut block_env, fork_at.block)
            .unwrap();

        // collect the tx
        let tx_hash = ToPrimitive::cvt("0xa278205118a242c87943b9ed83aacafe9906002627612ac3672d8ea224e38181");
        let tx = bp.transaction_by_hash(tx_hash).unwrap().unwrap();

        // simulate
        let r = state
            .transact(cfg, block_env, tx, no_inspector())
            .unwrap()
            .result;
        assert!(r.is_success());
        let receipt = bp.receipt_by_hash(tx_hash).unwrap().unwrap();
        assert_eq!(receipt.success, r.is_success());
        assert_eq!(receipt.logs.len(), r.logs().len());
        for (log, receipt_log) in r.logs().iter().zip(receipt.logs.iter()) {
            assert_eq!(log.address, receipt_log.address);
            assert_eq!(log.topics, receipt_log.topics);
            assert_eq!(*log.data, *receipt_log.data);
        }
    }
}
