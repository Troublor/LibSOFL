use std::fmt::Debug;

use revm::{
    inspectors::NoOpInspector, Database, DatabaseCommit, Inspector, EVM,
};
use revm_primitives::{
    db::DatabaseRef, BlockEnv, Bytes, CfgEnv, Eval, ExecutionResult, Output,
    ResultAndState,
};

use crate::error::SoflError;

use super::transaction::Tx;

pub mod fork;
pub mod fresh;

/// NoInspector is used as a placeholder for type parameters when no inspector is needed.
pub type NoInspector = NoOpInspector;

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
    fn transact<'a, 'b: 'a, I: Inspector<&'a mut Self>>(
        &'b mut self,
        evm_cfg: CfgEnv,
        block_env: BlockEnv,
        tx: Tx<'_, Self>,
        inspector: Option<I>,
    ) -> Result<ResultAndState, SoflError<Self::Error>> {
        let mut evm = EVM::new();
        if !tx.is_pseudo() {
            evm.env.cfg = evm_cfg;
            evm.env.block = block_env;
        }
        self.transact_with_evm(&mut evm, tx, inspector)
    }

    fn transact_with_evm<'a, 'b: 'a, I: Inspector<&'a mut Self>>(
        &'b mut self,
        evm: &mut EVM<&'a mut Self>,
        tx: Tx<'_, Self>,
        inspector: Option<I>,
    ) -> Result<ResultAndState, SoflError<Self::Error>> {
        evm.database(self);
        if let Tx::Pseudo(tx) = tx {
            // execute pseudo transaction
            let db = evm.db.as_ref().unwrap();
            let changes = tx(db);
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
            reth_revm::env::fill_tx_env(&mut evm.env.tx, tx, sender);
            let result;
            if let Some(inspector) = inspector {
                result = evm.inspect(inspector).map_err(SoflError::Evm)?;
            } else {
                result = evm.transact().map_err(SoflError::Evm)?;
            }
            Ok(result)
        }
    }

    fn transit<I: Inspector<Self>>(
        self,
        evm_cfg: CfgEnv,
        block_env: BlockEnv,
        txs: Vec<Tx<'_, Self>>,
        inspector: Option<&mut I>,
    ) -> Result<(Self, Vec<ExecutionResult>), SoflError<Self::Error>> {
        let mut results = Vec::new();
        let mut evm = EVM::new();
        evm.env.cfg = evm_cfg;
        evm.env.block = block_env;
        evm.database(self);
        if let Some(mut inspector) = inspector {
            for tx in txs {
                let sender = tx.sender();
                reth_revm::env::fill_tx_env(&mut evm.env.tx, tx, sender);
                let inspector = &mut inspector;
                let result = evm.inspect(inspector).map_err(SoflError::Evm)?;
                // evm.db must exist since we called evm.database(state) above
                let db = evm.db.as_mut().unwrap();
                db.commit(result.state);
                results.push(result.result);
            }
        } else {
            for tx in txs {
                let sender = tx.sender();
                reth_revm::env::fill_tx_env(&mut evm.env.tx, tx, sender);
                let result = evm.transact().map_err(SoflError::Evm)?;
                // evm.db must exist since we called evm.database(state) above
                let db = evm.db.as_mut().unwrap();
                db.commit(result.state);
                results.push(result.result);
            }
        }
        // evm.db must exist since we called evm.database(state) above
        let db = evm.db.unwrap();
        Ok((db, results))
    }

    fn transit_one<I: Inspector<Self>>(
        self,
        evm_cfg: CfgEnv,
        block_env: BlockEnv,
        tx: Tx<'_, Self>,
        inspector: Option<&mut I>,
    ) -> Result<(Self, ExecutionResult), SoflError<Self::Error>> {
        let (this, mut results) =
            self.transit(evm_cfg, block_env, vec![tx], inspector)?;
        Ok((this, results.remove(0)))
    }
}

// Auto implement BcState for all types that implement Database and DatabaseCommit
impl<E, T: Database<Error = E> + DatabaseCommit + Sized + Debug> BcState<E>
    for T
{
}
