use reth_primitives::{BlockHashOrNumber, BlockId};
use reth_provider::{EvmEnvProvider, StateProviderBox, StateProviderFactory, TransactionsProvider};
use reth_revm::database::{State, SubState};
use revm::{
    db::{CacheDB, EmptyDB},
    inspectors::NoOpInspector,
    Database, DatabaseCommit, Inspector, EVM,
};
use revm_primitives::{
    BlockEnv, Bytes, CfgEnv, EVMError, Env, Eval, ExecutionResult, Output, ResultAndState, U256,
};

use super::transaction::{Tx, TxPosition, TxPositionOutOfRangeError};

#[derive(Debug)]
pub enum ExecutorError<DBERR> {
    Evm(EVMError<DBERR>),
}

macro_rules! trait_alias {
    ($name:ident = $base1:ident + $($base2:ident +)+) => {
        pub trait $name: $base1 $(+ $base2)+ { }
        impl<T: $base1 $(+ $base2)+> $name for T { }
    };
}

trait_alias!(BcState = Database + DatabaseCommit +);

type NoInspector = NoOpInspector;

pub struct Executor<S> {
    evm: EVM<S>,
}

impl<'a> Executor<SubState<StateProviderBox<'a>>> {
    /// Create an executor with fork state from a transaction position.
    /// The forked state is the the state after the transaction at the position is executed.
    pub fn fork_from<BP: StateProviderFactory + EvmEnvProvider + TransactionsProvider>(
        p: &'a BP,
        pos: TxPosition,
    ) -> Result<Self, TxPositionOutOfRangeError> {
        let mut pos = pos;
        pos.shift(p, 1)?;
        Self::fork_at(p, pos)
    }

    /// Create an executor with fork state from a transaction position.
    /// The forked state is the the state before the transaction at the position is executed.
    pub fn fork_at<BP: StateProviderFactory + EvmEnvProvider + TransactionsProvider>(
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
        let bn = match pos.block {
            BlockHashOrNumber::Hash(h) => p
                .block_number(h)
                .unwrap()
                .ok_or(TxPositionOutOfRangeError::UnknownHash(h))?,
            BlockHashOrNumber::Number(n) => n,
        };
        let sp = p.state_by_block_id(BlockId::from(bn - 1)).unwrap();
        let wrapped = State::new(sp);
        let state = CacheDB::new(wrapped);

        // create evm
        let mut evm = EVM::new();
        evm.env.cfg = cfg;
        evm.env.block = block_env;
        evm.database(state);

        let mut executor = Self { evm };
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

impl Executor<CacheDB<EmptyDB>> {
    pub fn create(initialize: impl Fn(&mut CacheDB<EmptyDB>) -> (CfgEnv, BlockEnv)) -> Self {
        let db = EmptyDB {};
        let mut state = CacheDB::new(db);
        let (cfg, block_env) = initialize(&mut state);

        let mut evm = EVM::new();
        evm.env.cfg = cfg;
        evm.env.block = block_env;
        evm.database(state);
        Self { evm }
    }

    pub(crate) fn test_create(
        initialize: impl Fn(&mut CacheDB<EmptyDB>),
    ) -> Executor<CacheDB<EmptyDB>> {
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

impl<S: BcState + Clone> Clone for Executor<S> {
    fn clone(&self) -> Self {
        Self {
            evm: self.evm.clone(),
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
                result = self.evm.inspect(inspector).map_err(ExecutorError::Evm);
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

    pub fn state(&mut self) -> &mut S {
        self.evm.db().unwrap()
    }

    pub fn env(&self) -> &Env {
        &self.evm.env
    }

    pub fn commit_block(&mut self, cfg: Option<CfgEnv>, block_env: Option<BlockEnv>) {
        if let Some(cfg) = cfg {
            self.evm.env.cfg = cfg;
        }
        if let Some(block_env) = block_env {
            self.evm.env.block = block_env;
        } else {
            let mut blk = self.evm.env.block.clone();
            blk.number += U256::from(1);
            blk.timestamp += U256::from(1);
            self.evm.env.block = blk;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use reth_primitives::{Transaction, TransactionKind, TxLegacy};
    use reth_provider::{ReceiptProvider, TransactionsProvider};
    use revm::{
        db::{CacheDB, EmptyDB},
        Database,
    };
    use revm_primitives::{
        Account, AccountInfo, Address, Bytecode, Bytes, ExecutionResult, B160, U256,
    };

    use crate::{
        config::flags::SeeFuzzConfig,
        engine::{
            providers::BcProviderBuilder,
            transaction::{StateChange, Tx, TxPosition},
        },
    };

    use super::{Executor, NoInspector};

    #[test]
    fn test_fresh_state_with_plain_transfer() {
        let spender = Address::from(0);
        let receiver = Address::from(1);
        let mut exe = Executor::test_create(|state| {
            let acc = AccountInfo::new(U256::from(1000), Default::default(), Bytecode::new());
            state.insert_account_info(spender, acc);
            let acc = AccountInfo::new(U256::from(0), Default::default(), Bytecode::new());
            state.insert_account_info(receiver, acc);
        });
        let tx = Transaction::Legacy(TxLegacy {
            to: TransactionKind::Call(receiver),
            value: 500,
            gas_limit: 100000,
            ..Default::default()
        });
        let tx = Tx::Unsigned((spender, tx));

        // simulate
        let result = exe.simulate::<NoInspector>(tx.clone(), None).unwrap();
        assert!(matches!(result, ExecutionResult::Success { .. }));
        let state = exe.state();
        let spender_info = state.basic(spender).unwrap().unwrap();
        assert_eq!(
            spender_info.balance,
            U256::from(1000),
            "spender balance should be unchanged in simulation"
        );
        let receiver_info = state.basic(receiver).unwrap().unwrap();
        assert_eq!(
            receiver_info.balance,
            U256::from(0),
            "receiver balance should be unchanged in simulation"
        );

        // transact
        let result = exe.transact::<NoInspector>(tx.clone(), None).unwrap();
        assert!(matches!(result, ExecutionResult::Success { .. }));
        let state = exe.state();
        let spender_info = state.basic(spender).unwrap().unwrap();
        assert_eq!(
            spender_info.balance,
            U256::from(500),
            "spender balance should be decreased by 500"
        );
        let receiver_info = state.basic(receiver).unwrap().unwrap();
        assert_eq!(
            receiver_info.balance,
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
        let state = exe.state();
        let account_info = state.basic(account).unwrap().unwrap();
        assert_eq!(
            account_info.balance,
            U256::from(1000),
            "account balance should be 1000"
        );
        assert_eq!(
            account_info.code,
            Some(Bytecode::new_raw(Bytes::from("0x1234"))),
            "account code should be 0x1234"
        );
    }

    #[test]
    fn test_reproduce_block() {
        let datadir = SeeFuzzConfig::load().unwrap().reth.datadir;
        let datadir = Path::new(&datadir);
        let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();
        let fork_at = TxPosition::new(17000000, 0);
        let mut exe = Executor::fork_at(&bp, fork_at.clone()).unwrap();
        let txs = bp.transactions_by_block(fork_at.block).unwrap().unwrap();
        let receipts = bp.receipts_by_block(fork_at.block).unwrap().unwrap();
        let results = txs
            .iter()
            .map(|tx| {
                exe.transact::<NoInspector>(Tx::Signed(tx.clone()), None)
                    .unwrap()
            })
            .collect::<Vec<ExecutionResult>>();
        assert_eq!(results.len(), receipts.len());
        for (result, receipt) in results.iter().zip(receipts.iter()) {
            match result {
                ExecutionResult::Success { logs, .. } => {
                    assert!(receipt.success);
                    assert_eq!(receipt.logs.len(), logs.len());
                    for (log, receipt_log) in logs.iter().zip(receipt.logs.iter()) {
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
