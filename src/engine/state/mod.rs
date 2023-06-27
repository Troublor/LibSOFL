use std::{error::Error, fmt::Debug};

use auto_impl::auto_impl;
use reth_primitives::Address;
use revm::{
    interpreter::InstructionResult, Database, DatabaseCommit, Inspector, EVM,
};
use revm_primitives::{
    db::DatabaseRef, AccountInfo, BlockEnv, Bytes, Eval, ExecutionResult,
    Output, ResultAndState, U256,
};

use crate::error::SoflError;

use super::{
    config::EngineConfig, inspectors::MultiTxInspector,
    transactions::TxOrPseudo,
};

pub mod fork;
pub mod fresh;

// Abstration of the forked state from which the blockchain state is built upon.
pub trait BcStateGround: DatabaseRef + Sized {}

// Auto implement BcStateGround for all types that implement DatabaseRef
impl<T: DatabaseRef + Sized> BcStateGround for T {}

// Abstraction of the readonly blockchain state
pub trait ReadonlyBcState: Database + Sized {}

#[auto_impl(& mut, Box)]
pub trait DatabaseEditable {
    type Error;

    fn insert_account_storage(
        &mut self,
        address: Address,
        slot: U256,
        value: U256,
    ) -> Result<(), Self::Error>;

    fn insert_account_info(&mut self, address: Address, info: AccountInfo);
}

// Auto implement ReadonlyBcState for all types that implement Database
impl<T: Database + Sized> ReadonlyBcState for T {}

// Abstraction of blockchain state
pub trait BcState:
    Database<Error = Self::DbErr>
    + DatabaseEditable<Error = Self::DbErr>
    + DatabaseCommit
    + Sized
    + Debug
{
    type DbErr: Debug + Error;

    #[deprecated]
    fn transact_with_tx_filled<'a, S, I>(
        evm: &mut EVM<S>,
        inspector: I,
    ) -> Result<ResultAndState, SoflError<S::DbErr>>
    where
        S: BcState + 'a,
        I: MultiTxInspector<S>,
    {
        evm.inspect(inspector).map_err(SoflError::Evm)
    }

    #[deprecated]
    fn transact_with_evm<'a, S, I, T>(
        cfg: &EngineConfig,
        mut evm: EVM<S>,
        tx: T,
        inspector: I,
    ) -> Result<(EVM<S>, ResultAndState), SoflError<S::DbErr>>
    where
        S: BcState + 'a,
        I: MultiTxInspector<S>,
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
                let result =
                    Self::transact_with_tx_filled(&mut evm, inspector)?;
                Ok((evm, result))
            }
        }
    }
    #[deprecated]
    fn transact<'a, 'b: 'a, C, I, T>(
        &'b mut self,
        cfg: C,
        block_env: BlockEnv,
        tx: T,
        inspector: I,
    ) -> Result<ResultAndState, SoflError<Self::DbErr>>
    where
        C: Into<EngineConfig>,
        I: MultiTxInspector<&'a mut Self>,
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
    ) -> Result<(Self, Vec<ExecutionResult>), SoflError<Self::DbErr>>
    where
        Self: 'a,
        C: Into<EngineConfig>,
        I: MultiTxInspector<Self>,
        T: Into<TxOrPseudo<'a, Self>>,
    {
        let cfg = cfg.into();
        let mut results = Vec::new();
        let mut evm = EVM::new();
        evm.env.cfg = (*cfg).clone();
        evm.env.block = block_env;
        evm.database(self);
        for tx in txs {
            let tx = tx.into();
            let mut inspector = &mut inspector;
            let bc_state_ref = evm.db.as_mut().unwrap();
            // run inspector hook
            if inspector.transaction(&tx, bc_state_ref)
                != InstructionResult::Continue
            {
                continue;
            }
            let result;
            (evm, result) =
                Self::transact_with_evm(&cfg, evm, tx.clone(), &mut inspector)?;
            // evm.db must exist since we called evm.database(state) above
            let bc_state_ref = evm.db.as_mut().unwrap();
            bc_state_ref.commit(result.state);
            // run inspector hook
            inspector.transaction_end(&tx, bc_state_ref, &result.result);
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
    ) -> Result<(Self, ExecutionResult), SoflError<Self::DbErr>>
    where
        Self: 'a,
        C: Into<EngineConfig>,
        I: MultiTxInspector<Self>,
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
    ) -> Result<Vec<ExecutionResult>, SoflError<Self::DbErr>>
    where
        C: Into<EngineConfig>,
        I: MultiTxInspector<&'a mut Self>,
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
    ) -> Result<ExecutionResult, SoflError<Self::DbErr>>
    where
        Self: 'a,
        C: Into<EngineConfig>,
        I: MultiTxInspector<&'a mut Self>,
        T: Into<TxOrPseudo<'a, &'a mut Self>>,
    {
        let mut results =
            self.transit_inplace(cfg, block_env, vec![tx], inspector)?;
        Ok(results.remove(0))
    }
}

// Auto implement BcState for all types that implement Database and DatabaseCommit
impl<T: BcState> BcState for &mut T {
    type DbErr = T::DbErr;
}
