use std::{fmt::Debug, rc::Rc, sync::Arc};

use revm::{
    inspectors::{self, NoOpInspector},
    Database, DatabaseCommit, Inspector, EVM,
};
use revm_primitives::{
    db::DatabaseRef, BlockEnv, Bytes, CfgEnv, Eval, ExecutionResult, Output,
    ResultAndState,
};

use crate::{error::SoflError, fuzzing::corpus::tx};

use self::fork::ForkedBcState;

use super::transaction::Tx;

pub mod fork;
pub mod fresh;

/// NoInspector is used as a placeholder for type parameters when no inspector is needed.
pub type NoInspector = NoOpInspector;

// Abstration of the forked state from which the blockchain state is built upon.
pub trait BcStateGround:
    DatabaseRef<Error = reth_interfaces::Error> + Sized
{
}

// Auto implement BcStateGround for all types that implement DatabaseRef
impl<T: DatabaseRef<Error = reth_interfaces::Error> + Sized> BcStateGround
    for T
{
}

// Abstraction of the readonly blockchain state
pub trait ReadonlyBcState:
    Database<Error = reth_interfaces::Error> + Sized
{
}

// Auto implement ReadonlyBcState for all types that implement Database
impl<T: Database<Error = reth_interfaces::Error> + Sized> ReadonlyBcState
    for T
{
}

// Abstraction of blockchain state
pub trait BcState:
    Database<Error = reth_interfaces::Error> + DatabaseCommit + Sized + Debug
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
            let changes = tx(self);
            evm.database(self);
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

    fn transit<'a, 'b: 'a, I: Inspector<&'a mut Self>>(
        &'b mut self,
        evm_cfg: CfgEnv,
        block_env: BlockEnv,
        tx: Tx<'_, Self>,
        inspector: Option<I>,
    ) -> Result<ExecutionResult, SoflError<Self::Error>> {
        let result;
        let mut evm = EVM::new();
        evm.env.cfg = evm_cfg;
        evm.env.block = block_env;
        result = self.transact_with_evm(&mut evm, tx, inspector)?;
        evm.db.as_mut().unwrap().commit(result.state);
        Ok(result.result)
    }

    fn transit_with_evm<'a, 'b: 'a, I: Inspector<&'a mut Self>>(
        &'b mut self,
        evm: &mut EVM<&'a mut Self>,
        tx: Tx<'_, Self>,
        inspector: Option<I>,
    ) -> Result<ExecutionResult, SoflError<Self::Error>> {
        let result = self.transact_with_evm(evm, tx, inspector)?;
        evm.db.as_mut().unwrap().commit(result.state);
        Ok(result.result)
    }

    fn transit_fn<I: Inspector<Self>>(
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
}

// Auto implement BcState for all types that implement Database and DatabaseCommit
impl<
        T: Database<Error = reth_interfaces::Error>
            + DatabaseCommit
            + Sized
            + Debug,
    > BcState for T
{
}
