use std::sync::Arc;

use reth_primitives::{BlockHashOrNumber, BlockId};
use reth_provider::{
    EvmEnvProvider, StateProviderBox, StateProviderFactory,
    TransactionsProvider,
};
use reth_revm::database::{State, SubState};
use revm::{
    db::{CacheDB, DatabaseRef, EmptyDB},
    inspectors::NoOpInspector,
    Database, DatabaseCommit, Inspector, EVM,
};
use revm_primitives::{
    db::DatabaseRef, BlockEnv, Bytes, CfgEnv, EVMError, Env, Eval,
    ExecutionResult, Output, ResultAndState, U256,
};

use super::transaction::{Tx, TxPosition, TxPositionOutOfRangeError};

#[derive(Debug)]
pub enum ExecutorError<DBERR> {
    Evm(EVMError<DBERR>),
}

macro_rules! trait_alias {
    ($name:ident = $base1:ident + $($base2:ident +)*) => {
        pub trait $name: $base1 $(+ $base2)* { }
        impl<T: $base1 $(+ $base2)*> $name for T { }
    };
}

// Abstraction of blockchain state
trait_alias!(BcState = Database + DatabaseCommit +);
// Abstration of the forked state from which the blockchain state is built upon.
trait_alias!(BcStateGround = DatabaseRef +);
// Abstraction of the readonly blockchain state
trait_alias!(ReadonlyBcState = DatabaseRef +);

/// Abstraction of the forked state in revm that can be cloned.
/// This type implements both BcState and BcStateGround
pub type ClonableForkedState<'a> = CacheDB<Arc<State<StateProviderBox<'a>>>>;

/// A blockchain state that is empty and complete in memory.
pub type FreshBcState = CacheDB<EmptyDB>;

pub type NoInspector = NoOpInspector;

// TODO: refactor this
pub const DEFAULT_TIME_INTERVAL: u64 = 12;

pub struct Executor<S> {
    evm: EVM<S>,
    block_interval: u64,
}

impl<'a> Executor<ClonableForkedState<'a>> {
    /// Create an executor with fork state from a transaction position.
    /// The forked state is the the state after the transaction at the position is executed.
    pub fn fork_from<
        BP: StateProviderFactory + EvmEnvProvider + TransactionsProvider,
    >(
        p: &'a BP,
        pos: TxPosition,
    ) -> Result<Self, TxPositionOutOfRangeError> {
        let mut pos = pos;
        pos.shift(p, 1)?;
        Self::fork_at(p, pos)
    }

    /// Create an executor with fork state from a transaction position.
    /// The forked state is the the state before the transaction at the position is executed.
    pub fn fork_at<
        BP: StateProviderFactory + EvmEnvProvider + TransactionsProvider,
    >(
        p: &'a BP,
        pos: TxPosition,
    ) -> Result<Self, TxPositionOutOfRangeError> {
        // prepare env
        let mut cfg = CfgEnv::default();
        let mut block_env = BlockEnv::default();
        let pos1 = pos.clone();
        p.fill_env_at(&mut cfg, &mut block_env, pos.block)
            .map_err(|_| TxPositionOutOfRangeError::unknown_block(pos1, p))?;

        // create state
        let bn = pos.get_block_number(p)?;
        let sp = p.state_by_block_id(BlockId::from(bn - 1)).unwrap();
        let wrapped = State::new(sp);
        let state = CacheDB::new(Arc::new(wrapped));

        // create evm
        let mut evm = EVM::new();
        evm.env.cfg = cfg;
        evm.env.block = block_env;
        evm.database(state);

        let mut executor = Self {
            evm,
            block_interval: DEFAULT_TIME_INTERVAL,
        };
        // execute preceeding transactions
        if pos.index > 0 {
            // block must exist because we have checked it in fill_env_at
            let txs = p.transactions_by_block(pos.block).unwrap().unwrap();
            if pos.index >= txs.len() as u64 {
                return Err(TxPositionOutOfRangeError::IndexOverflow((
                    txs.len() as u64,
                    pos.index,
                )));
            }
            for tx in txs.iter().take(pos.index as usize) {
                let tx = Tx::Signed(tx.clone());
                // the transact of historical transaction must be non-error
                executor.transact::<NoInspector>(tx, None).unwrap();
            }
        }
        Ok(executor)
    }
}

impl Executor<FreshBcState> {
    pub fn create(
        initialize: impl Fn(&mut FreshBcState) -> (CfgEnv, BlockEnv),
    ) -> Self {
        let db = EmptyDB {};
        let mut state = CacheDB::new(db);
        let (cfg, block_env) = initialize(&mut state);

        let mut evm = EVM::new();
        evm.env.cfg = cfg;
        evm.env.block = block_env;
        evm.database(state);
        Self {
            evm,
            block_interval: DEFAULT_TIME_INTERVAL,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_create(
        initialize: impl Fn(&mut FreshBcState),
    ) -> Executor<FreshBcState> {
        Executor::create(|state| {
            initialize(state);
            let cfg = CfgEnv {
                disable_block_gas_limit: true,
                disable_base_fee: true,
                ..Default::default()
            };
            let block_env = BlockEnv {
                gas_limit: U256::from(1000000),
                ..Default::default()
            };
            (cfg, block_env)
        })
    }
}

impl<'a, S: Clone> Clone for Executor<S> {
    fn clone(&self) -> Self {
        Self {
            evm: self.evm.clone(),
            block_interval: self.block_interval,
        }
    }
}

impl<S: BcState> Executor<S> {
    fn run<I: Inspector<S>>(
        &mut self,
        tx: Tx<S>,
        inspector: Option<I>,
    ) -> Result<ResultAndState, ExecutorError<S::Error>> {
        if let Tx::Pseudo(tx) = tx {
            // execute pseudo tx
            let changes = tx(self.evm.db.as_mut().unwrap());
            Ok(ResultAndState {
                result: ExecutionResult::Success {
                    reason: Eval::Return,
                    gas_used: 0,
                    gas_refunded: 0,
                    logs: Vec::new(),
                    output: Output::Call(Bytes::new()),
                },
                state: changes,
            })
        } else {
            let sender = tx.sender();
            reth_revm::env::fill_tx_env(&mut self.evm.env.tx, tx, sender);
            // execute tx
            let result;
            if let Some(inspector) = inspector {
                result =
                    self.evm.inspect(inspector).map_err(ExecutorError::Evm);
            } else {
                result = self.evm.transact().map_err(ExecutorError::Evm);
            }
            result
        }
    }
}

impl<S: BcState> Executor<S> {
    pub fn simulate<I: Inspector<S>>(
        &mut self,
        tx: Tx<S>,
        inspector: Option<I>,
    ) -> Result<ExecutionResult, ExecutorError<S::Error>> {
        let ResultAndState { result, state: _ } = self.run(tx, inspector)?;
        Ok(result)
    }

    pub fn transact<I: Inspector<S>>(
        &mut self,
        tx: Tx<S>,
        inspector: Option<I>,
    ) -> Result<ExecutionResult, ExecutorError<S::Error>> {
        let ResultAndState { result, state } = self.run(tx, inspector)?;
        // commit state change
        self.evm.db().as_mut().unwrap().commit(state);
        Ok(result)
    }

    pub fn get_state_mut(&mut self) -> &mut S {
        self.evm.db().unwrap()
    }

    pub fn get_env_mut(&mut self) -> &mut Env {
        &mut self.evm.env
    }

    pub fn set_block_interval(&mut self, interval: u64) {
        self.block_interval = interval;
    }

    pub fn commit_block(
        &mut self,
        cfg: Option<CfgEnv>,
        block_env: Option<BlockEnv>,
    ) {
        if let Some(cfg) = cfg {
            self.evm.env.cfg = cfg;
        }
        if let Some(block_env) = block_env {
            self.evm.env.block = block_env;
        } else {
            let mut blk = self.evm.env.block.clone();
            blk.number += U256::from(1);
            blk.timestamp += U256::from(self.block_interval);
            self.evm.env.block = blk;
        }
    }
}

impl<S: DatabaseRef> Executor<S> {
    pub fn get_state(&self) -> &S {
        self.evm.db.as_ref().unwrap()
    }

    pub fn get_env(&self) -> &Env {
        &self.evm.env
    }
}

#[cfg(test)]
mod tests_nodep {

    use std::sync::Arc;

    use reth_primitives::{Transaction, TransactionKind, TxLegacy};

    use revm::{
        db::{CacheDB, EmptyDB},
        Database,
    };
    use revm_primitives::{
        Account, AccountInfo, Address, Bytecode, Bytes, ExecutionResult, B160,
        U256,
    };

    use crate::{
        config::flags::SoflConfig,
        engine::{
            providers::BcProviderBuilder,
            transaction::{StateChange, Tx, TxPosition},
        },
        utils::cheatcodes,
    };

    use super::{Executor, NoInspector};

    #[test]
    fn test_fresh_state_with_plain_transfer() {
        let spender = Address::from(0);
        let receiver = Address::from(1);
        let mut exe = Executor::test_create(|state| {
            let acc = AccountInfo::new(
                U256::from(1000),
                Default::default(),
                Bytecode::new(),
            );
            state.insert_account_info(spender, acc);
            let acc = AccountInfo::new(
                U256::from(0),
                Default::default(),
                Bytecode::new(),
            );
            state.insert_account_info(receiver, acc);
        });
        let tx = Transaction::Legacy(TxLegacy {
            to: TransactionKind::Call(receiver),
            value: 500,
            gas_limit: 100000,
            ..Default::default()
        });
        let tx: Tx<'_, CacheDB<EmptyDB>> = Tx::Unsigned((spender, tx));

        // simulate
        let result = exe.simulate::<NoInspector>(tx.clone(), None).unwrap();
        assert!(matches!(result, ExecutionResult::Success { .. }));
        let spender_balance =
            cheatcodes::get_ether_balance(&exe, spender).unwrap();
        assert_eq!(
            spender_balance,
            U256::from(1000),
            "spender balance should be unchanged in simulation"
        );
        let receiver_balance =
            cheatcodes::get_ether_balance(&exe, receiver).unwrap();
        assert_eq!(
            receiver_balance,
            U256::from(0),
            "receiver balance should be unchanged in simulation"
        );

        // transact
        let result = exe.transact::<NoInspector>(tx.clone(), None).unwrap();
        assert!(matches!(result, ExecutionResult::Success { .. }));
        let spender_balance =
            cheatcodes::get_ether_balance(&exe, spender).unwrap();
        assert_eq!(
            spender_balance,
            U256::from(500),
            "spender balance should be decreased by 500"
        );
        let receiver_balance =
            cheatcodes::get_ether_balance(&exe, receiver).unwrap();
        assert_eq!(
            receiver_balance,
            U256::from(500),
            "receiver balance should be increased by 500"
        );
    }

    #[test]
    fn test_pseudo_tx() {
        let account = Address::from(0);
        let mut exe = Executor::test_create(|_| {});
        let tx_lambda = |_state: &CacheDB<EmptyDB>| {
            let mut changes = StateChange::default();
            let mut change = Account::new_not_existing();
            change.is_not_existing = false;
            change.info.balance = U256::from(1000);
            change.info.code = Some(Bytecode::new_raw(Bytes::from("0x1234")));
            changes.insert(account, change);
            changes
        };
        let tx = Tx::Pseudo(&tx_lambda);

        let result = exe.transact::<NoInspector>(tx, None).unwrap();
        assert!(matches!(result, ExecutionResult::Success { .. }));
        let balance = cheatcodes::get_ether_balance(&exe, account).unwrap();
        let code = cheatcodes::get_code(&exe, account).unwrap();
        assert_eq!(balance, U256::from(1000), "account balance should be 1000");
        assert_eq!(
            code,
            Some(Bytecode::new_raw(Bytes::from("0x1234"))),
            "account code should be 0x1234"
        );
    }
}

#[cfg(test)]
mod tests_with_db {
    use std::path::Path;

    use reth_provider::{ReceiptProvider, TransactionsProvider};
    use revm_primitives::ExecutionResult;

    use crate::{
        config::flags::SoflConfig,
        engine::{
            executor::{Executor, NoInspector},
            providers::BcProviderBuilder,
            transaction::TxPosition,
        },
    };

    #[test]
    fn test_reproduce_block() {
        let datadir = SoflConfig::load().unwrap().reth.datadir;
        let datadir = Path::new(&datadir);
        let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();
        let fork_at = TxPosition::new(17000000, 0);
        let mut exe = Executor::fork_at(&bp, fork_at.clone()).unwrap();
        let txs = bp.transactions_by_block(fork_at.block).unwrap().unwrap();
        let receipts = bp.receipts_by_block(fork_at.block).unwrap().unwrap();
        let results = txs
            .iter()
            .map(|tx| {
                exe.transact::<NoInspector>(tx.clone().into(), None)
                    .unwrap()
            })
            .collect::<Vec<ExecutionResult>>();
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
    use reth_provider::{ReceiptProvider, TransactionsProvider};

    use crate::{
        config::flags::SoflConfig,
        engine::{providers::BcProviderBuilder, transaction::TxPosition},
        utils::conversion::{Convert, ToPrimitive},
    };

    use super::{Executor, NoInspector};

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
        let mut exe = Executor::fork_at(&bp, fork_at.clone()).unwrap();
        let tx_hash = ToPrimitive::cvt("0xa278205118a242c87943b9ed83aacafe9906002627612ac3672d8ea224e38181");
        let tx = bp.transaction_by_hash(tx_hash).unwrap().unwrap();
        let r = exe
            .simulate::<NoInspector>(tx.clone().into(), None)
            .unwrap();
        assert!(r.is_success());
        let receipt = bp.receipt_by_hash(tx_hash).unwrap().unwrap();
        assert_eq!(receipt.success, r.is_success());
        assert_eq!(receipt.logs.len(), r.logs().len());
        for (log, receipt_log) in r.logs().iter().zip(receipt.logs.iter()) {
            assert_eq!(log.address, receipt_log.address);
            assert_eq!(log.topics, receipt_log.topics);
            assert_eq!(*log.data, *receipt_log.data);
        }
        println!("again");
    }
}
