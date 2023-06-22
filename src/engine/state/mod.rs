use reth_primitives::Address;
use revm::{Database, DatabaseCommit, Inspector, EVM};
use revm_primitives::{
    db::DatabaseRef, BlockEnv, Bytes, CfgEnv, Eval, ExecutionResult, Output,
    ResultAndState,
};

use crate::error::SoflError;

use super::transaction::Tx;

pub mod fork;
pub mod fresh;

// Abstration of the forked state from which the blockchain state is built upon.
pub trait BcStateGround: DatabaseRef + Sized {}

// Auto implement BcStateGround for all types that implement DatabaseRef
impl<T: DatabaseRef + Sized> BcStateGround for T {}

// Abstraction of the readonly blockchain state
pub trait ReadonlyBcState: Database + Sized {}

// Auto implement ReadonlyBcState for all types that implement Database
impl<T: Database + Sized> ReadonlyBcState for T {}

// Abstraction of blockchain state
pub trait BcState: Database + DatabaseCommit + Sized {
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
        self.transact_with_env(&mut evm, tx, inspector)
    }

    fn transact_with_env<'a, 'b: 'a, I: Inspector<&'a mut Self>>(
        &'b mut self,
        evm: &mut EVM<&'a mut Self>,
        tx: Tx<'_, Self>,
        inspector: Option<I>,
    ) -> Result<ResultAndState, SoflError<Self::Error>> {
        if let Tx::Pseudo(tx) = tx {
            // execute pseudo transaction
            let changes = tx(self);
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
            evm.database(self);
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
        result = self.transact_with_env(&mut evm, tx, inspector)?;
        evm.db.as_mut().unwrap().commit(result.state);
        Ok(result.result)
    }

    fn transit_with_evm<'a, 'b: 'a, I: Inspector<&'a mut Self>>(
        &'b mut self,
        evm: &mut EVM<&'a mut Self>,
        tx: Tx<'_, Self>,
        inspector: Option<I>,
    ) -> Result<ExecutionResult, SoflError<Self::Error>> {
        let result = self.transact_with_env(evm, tx, inspector)?;
        evm.db.as_mut().unwrap().commit(result.state);
        Ok(result.result)
    }
}

// Auto implement BcState for all types that implement Database and DatabaseCommit
impl<T: Database + DatabaseCommit + Sized> BcState for T {}

struct MyStruct {
    value: i32,
}

impl MyStruct {
    fn method1<'a>(&'a mut self) {
        // Do something with `self`
        self.value += 1;
    }

    fn method2<'a>(&'a mut self) {
        // Do something else with `self`
        self.value *= 2;
    }

    fn call_methods<'a>(&'a mut self) {
        self.method1(); // First method call
        self.method2(); // Second method call
    }
}
