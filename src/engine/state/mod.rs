use std::fmt::Debug;

use revm::{
    inspectors::NoOpInspector, Database, DatabaseCommit, Inspector, EVM,
};
use revm_primitives::{
    db::DatabaseRef, BlockEnv, Bytes, Eval, ExecutionResult, Output,
    ResultAndState,
};

use crate::error::SoflError;

use super::{config::EngineConfig, transaction::TxOrPseudo};

pub mod fork;
pub mod fresh;

/// NoInspector is used as a placeholder for type parameters when no inspector is needed.
pub type NoInspector = NoOpInspector;

pub static mut NO_INSPECTOR: NoInspector = NoInspector {};
pub fn no_inspector() -> &'static mut NoInspector {
    // unsafe is ok here since NoInspector is essential a no-op inspector
    unsafe { &mut NO_INSPECTOR }
}

// Abstration of the forked state from which the blockchain state is built upon.
pub trait BcStateGround<E = reth_interfaces::Error>:
    DatabaseRef<Error = E> + Sized
{
}

// Auto implement BcStateGround for all types that implement DatabaseRef
impl<E, T: DatabaseRef<Error = E> + Sized> BcStateGround<E> for T {}

// Abstraction of the readonly blockchain state
pub trait ReadonlyBcState<E = reth_interfaces::Error>:
    Database<Error = E> + Sized
{
}

// Auto implement ReadonlyBcState for all types that implement Database
impl<E, T: Database<Error = E> + Sized> ReadonlyBcState<E> for T {}

// Abstraction of blockchain state
pub trait BcState<E = reth_interfaces::Error>:
    Database<Error = E> + DatabaseCommit + Sized + Debug
{
    fn transact_with_evm<'a, S, I, T>(
        cfg: &EngineConfig,
        mut evm: EVM<S>,
        tx: T,
        inspector: I,
    ) -> Result<(EVM<S>, ResultAndState), SoflError<S::Error>>
    where
        S: BcState<E> + 'a,
        I: Inspector<S>,
        T: Into<TxOrPseudo<'a, S>>,
    {
        let tx = tx.into();
        let db = evm
            .db
            .as_ref()
            .expect("EVM must have a database already set");
        match tx {
            TxOrPseudo::Pseudo(tx) => {
                // execute pseudo transaction
                let changes = tx(db);
                Ok((
                    evm,
                    ResultAndState {
                        result: ExecutionResult::Success {
                            reason: Eval::Return,
                            gas_used: 0,
                            gas_refunded: 0,
                            logs: Vec::new(),
                            output: Output::Call(Bytes::new()),
                        },
                        state: changes,
                    },
                ))
            }
            TxOrPseudo::Tx(tx) => {
                let sender = tx.from();
                reth_revm::env::fill_tx_env(&mut evm.env.tx, &tx, sender);
                if cfg.disable_nonce_check {
                    evm.env.tx.nonce = None;
                }
                let result = evm.inspect(inspector).map_err(SoflError::Evm)?;
                Ok((evm, result))
            }
        }
    }

    fn transact<'a, 'b: 'a, C, I, T>(
        &'b mut self,
        cfg: C,
        block_env: BlockEnv,
        tx: T,
        inspector: I,
    ) -> Result<ResultAndState, SoflError<Self::Error>>
    where
        C: Into<EngineConfig>,
        I: Inspector<&'a mut Self>,
        T: Into<TxOrPseudo<'a, &'a mut Self>>,
    {
        let cfg = cfg.into();
        let tx = tx.into();
        let mut evm = EVM::new();
        if !tx.is_pseudo() {
            evm.env.cfg = (*cfg).clone();
            evm.env.block = block_env;
        }
        evm.database(self);
        let (_, r) = Self::transact_with_evm(&cfg, evm, tx, inspector)?;
        Ok(r)
    }

    fn transit<'a, C, I, T>(
        self,
        cfg: C,
        block_env: BlockEnv,
        txs: Vec<T>,
        mut inspector: &mut I,
    ) -> Result<(Self, Vec<ExecutionResult>), SoflError<Self::Error>>
    where
        Self: 'a,
        C: Into<EngineConfig>,
        I: Inspector<Self>,
        T: Into<TxOrPseudo<'a, Self>>,
    {
        let cfg = cfg.into();
        let mut results = Vec::new();
        let mut evm = EVM::new();
        evm.env.cfg = (*cfg).clone();
        evm.env.block = block_env;
        evm.database(self);
        for tx in txs {
            let inspector = &mut inspector;
            let result;
            (evm, result) = Self::transact_with_evm(&cfg, evm, tx, inspector)?;
            // evm.db must exist since we called evm.database(state) above
            evm.db.as_mut().unwrap().commit(result.state);
            results.push(result.result);
        }
        // evm.db must exist since we called evm.database(state) above
        let db = evm.db.unwrap();
        Ok((db, results))
    }

    fn transit_one<'a, C, I, T>(
        self,
        cfg: C,
        block_env: BlockEnv,
        tx: T,
        inspector: &'a mut I,
    ) -> Result<(Self, ExecutionResult), SoflError<Self::Error>>
    where
        Self: 'a,
        C: Into<EngineConfig>,
        I: Inspector<Self>,
        T: Into<TxOrPseudo<'a, Self>>,
    {
        let (this, mut results) =
            self.transit(cfg, block_env, vec![tx], inspector)?;
        Ok((this, results.remove(0)))
    }

    fn transit_inplace<'a, C, I, T>(
        &'a mut self,
        cfg: C,
        block_env: BlockEnv,
        txs: Vec<T>,
        mut inspector: &mut I,
    ) -> Result<Vec<ExecutionResult>, SoflError<Self::Error>>
    where
        C: Into<EngineConfig>,
        I: Inspector<&'a mut Self>,
        T: Into<TxOrPseudo<'a, &'a mut Self>>,
    {
        let cfg = cfg.into();
        let mut results = Vec::new();
        let mut evm = EVM::new();
        evm.env.cfg = (*cfg).clone();
        evm.env.block = block_env;
        evm.database(self);
        for tx in txs {
            let inspector = &mut inspector;
            let result;
            (evm, result) = Self::transact_with_evm(&cfg, evm, tx, inspector)?;
            // evm.db must exist since we called evm.database(state) above
            evm.db.as_mut().unwrap().commit(result.state);
            results.push(result.result);
        }

        // evm.db must exist since we called evm.database(state) above
        Ok(results)
    }
    fn transit_one_inplace<'a, C, I, T>(
        &'a mut self,
        cfg: C,
        block_env: BlockEnv,
        tx: T,
        inspector: &mut I,
    ) -> Result<ExecutionResult, SoflError<Self::Error>>
    where
        Self: 'a,
        C: Into<EngineConfig>,
        I: Inspector<&'a mut Self>,
        T: Into<TxOrPseudo<'a, &'a mut Self>>,
    {
        let mut results =
            self.transit_inplace(cfg, block_env, vec![tx], inspector)?;
        Ok(results.remove(0))
    }
}

// Auto implement BcState for all types that implement Database and DatabaseCommit
impl<E, T: Database<Error = E> + DatabaseCommit + Sized + Debug> BcState<E>
    for T
{
}
