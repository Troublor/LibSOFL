use revm::{inspector_handle_register, GetInspector, Inspector};
use revm_primitives::StorageSlot;

use crate::error::SoflError;

use super::types::{Bytecode, Database, Env, SpecId};
use super::{
    inspector::EvmInspector,
    transition::TransitionSpec,
    types::{
        Account, AccountInfo, AccountStatus, Address, ExecutionResult,
        StateChange, Storage, U256,
    },
};

struct InspectorWrapper<'a, S: BcState>(Box<dyn EvmInspector<S> + 'a>);

impl<'a, S: BcState> InspectorWrapper<'a, S> {
    pub fn new<I: EvmInspector<S> + 'a>(inspector: I) -> Self {
        Self(Box::new(inspector))
    }

    pub fn get_evm_inspector(&mut self) -> &mut dyn EvmInspector<S> {
        &mut self.0
    }
}

impl<'a, S: BcState> GetInspector<'a, S> for InspectorWrapper<'a, S> {
    fn get_inspector(&mut self) -> &mut dyn Inspector<S> {
        &mut self.0
    }
}

/// BcState is a wrapper of revm's Database trait.
/// It provides a set of basic methods to read the state of the blockchain.
pub trait BcState:
    Database<Error = Self::DatabaseErr> + revm::DatabaseCommit
{
    type DatabaseErr: std::fmt::Debug;

    fn transit<'a, I>(
        &'a mut self,
        spec: TransitionSpec,
        mut inspector: &mut I,
    ) -> Result<Vec<ExecutionResult>, SoflError>
    where
        <Self as revm::Database>::Error: std::fmt::Debug,
        Self: 'a,
        I: EvmInspector<&'a mut Self>,
    {
        let envs: Vec<Env> = spec.into();
        let mut results = Vec::new();
        let mut evm = revm::EvmBuilder::default()
            .with_db(self)
            .with_external_context(InspectorWrapper::new(&mut inspector))
            .spec_id(SpecId::LATEST)
            .append_handler_register(inspector_handle_register)
            .build();
        for env in envs.into_iter() {
            evm = revm::EvmBuilder::new(evm)
                .modify_env(|e| {
                    e.cfg = env.cfg;
                    e.block = env.block;
                    e.tx = env.tx;
                })
                .build();

            // inspector pre-transaction hook
            let insp = evm.context.external.get_evm_inspector();
            if !insp.transaction(&evm.context.evm.env.tx, &evm.context.evm.db) {
                // return false to skip transaction
                results.push(revm::primitives::ExecutionResult::Halt {
                    reason: revm::primitives::HaltReason::NotActivated,
                    gas_used: 0,
                });
                continue;
            }

            // execute transaction
            let result = evm.transact_commit().map_err(|e| match e {
                revm::primitives::EVMError::Transaction(ee) => {
                    SoflError::InvalidTransaction(ee)
                }
                revm::primitives::EVMError::Header(ee) => {
                    SoflError::InvalidHeader(ee)
                }
                revm::primitives::EVMError::Database(ee) => {
                    SoflError::BcState(format!("{:?}", ee))
                }
                revm::primitives::EVMError::Custom(ee) => {
                    SoflError::BcState(format!("{:?}", ee))
                }
            })?;

            // inspector post-transaction hook
            let insp = evm.context.external.get_evm_inspector();
            insp.transaction_end(
                &evm.context.evm.env.tx,
                &evm.context.evm.db,
                &result,
            );

            results.push(result);
        }
        Ok(results)
    }

    /// transit without inspector
    /// NOTE: this is more efficient than using `transit` with no_inspector().
    fn transit_without_inspector<'a>(
        &'a mut self,
        spec: TransitionSpec,
    ) -> Result<Vec<ExecutionResult>, SoflError>
    where
        Self::Error: std::fmt::Debug,
    {
        let envs: Vec<Env> = spec.into();
        let mut results = Vec::new();
        let mut evm = revm::EvmBuilder::default()
            .with_db(self)
            .spec_id(SpecId::LATEST)
            .build();

        for env in envs.into_iter() {
            evm = revm::EvmBuilder::new(evm)
                .modify_env(|e| {
                    e.cfg = env.cfg;
                    e.block = env.block;
                    e.tx = env.tx;
                })
                .build();

            // execute transaction
            let result = evm.transact_commit().map_err(|e| match e {
                revm::primitives::EVMError::Transaction(ee) => {
                    SoflError::InvalidTransaction(ee)
                }
                revm::primitives::EVMError::Header(ee) => {
                    SoflError::InvalidHeader(ee)
                }
                revm::primitives::EVMError::Database(ee) => {
                    SoflError::BcState(format!("{:?}", ee))
                }
                revm::primitives::EVMError::Custom(ee) => {
                    SoflError::BcState(format!("{:?}", ee))
                }
            })?;

            results.push(result);
        }
        Ok(results)
    }

    /// Simulate state transition without modifying the state.
    /// Returns the state modification.
    /// Function apply_changes() can be used to apply the changes to the state.
    fn simulate<'a, I>(
        &'a mut self,
        spec: TransitionSpec,
        mut inspector: &mut I,
    ) -> Result<(Vec<StateChange>, Vec<ExecutionResult>), SoflError>
    where
        Self::Error: std::fmt::Debug,
        I: EvmInspector<&'a mut Self>,
    {
        let envs: Vec<Env> = spec.into();
        let mut results = Vec::new();
        let mut changes = Vec::new();
        let mut evm = revm::EvmBuilder::default()
            .with_db(self)
            .with_external_context(InspectorWrapper::new(&mut inspector))
            .spec_id(SpecId::LATEST)
            .append_handler_register(inspector_handle_register)
            .build();

        for env in envs.into_iter() {
            evm = revm::EvmBuilder::new(evm)
                .modify_env(|e| {
                    e.cfg = env.cfg;
                    e.block = env.block;
                    e.tx = env.tx;
                })
                .build();

            // inspector pre-transaction hook
            let insp = evm.context.external.get_evm_inspector();
            if !insp.transaction(&evm.context.evm.env.tx, &evm.context.evm.db) {
                // return false to skip transaction
                results.push(revm::primitives::ExecutionResult::Halt {
                    reason: revm::primitives::HaltReason::NotActivated,
                    gas_used: 0,
                });
                continue;
            }

            // execute
            let revm::primitives::ResultAndState { result, state } =
                evm.transact().map_err(|e| match e {
                    revm::primitives::EVMError::Transaction(ee) => {
                        SoflError::InvalidTransaction(ee)
                    }
                    revm::primitives::EVMError::Header(ee) => {
                        SoflError::InvalidHeader(ee)
                    }
                    revm::primitives::EVMError::Database(ee) => {
                        SoflError::BcState(format!("{:?}", ee))
                    }
                    revm_primitives::EVMError::Custom(ee) => {
                        SoflError::BcState(format!("{:?}", ee))
                    }
                })?;

            // inspector post-transaction hook
            let insp = evm.context.external.get_evm_inspector();
            insp.transaction_end(
                &evm.context.evm.env.tx,
                &evm.context.evm.db,
                &result,
            );

            results.push(result);
            changes.push(state);
        }

        Ok((changes, results))
    }

    fn apply_changes<'a>(&'a mut self, changes: Vec<StateChange>) {
        changes.into_iter().for_each(|c| self.commit(c));
    }

    fn get_account_code(
        &mut self,
        address: Address,
    ) -> Result<Bytecode, SoflError> {
        let account = self
            .basic(address)
            .map_err(|e| {
                SoflError::BcState(format!(
                    "failed to get account basic: {:?}",
                    e
                ))
            })?
            .unwrap_or_default();
        Ok(account.code.unwrap_or_default())
    }

    /// Set a new value to a storage slot of an account.
    fn insert_account_storage(
        &mut self,
        address: Address,
        slot: U256,
        value: U256,
    ) -> Result<(), SoflError>
    where
        <Self as revm::Database>::Error: std::fmt::Debug,
    {
        let account = self
            .basic(address)
            .map_err(|e| {
                SoflError::BcState(format!(
                    "failed to get account basic: {:?}",
                    e
                ))
            })?
            .unwrap_or_default();
        let mut changes = StateChange::default();
        let mut storage_change = Storage::default();
        storage_change.insert(slot, StorageSlot::new(value));
        let account = Account {
            info: account,
            storage: storage_change,
            status: AccountStatus::Touched,
        };
        changes.insert(address, account);
        self.commit(changes);
        Ok(())
    }

    /// Insert an account into the state.
    fn insert_account_info(&mut self, address: Address, info: AccountInfo) {
        let mut changes = StateChange::default();
        changes.insert(
            address,
            Account {
                info,
                storage: Default::default(),
                status: AccountStatus::Touched,
            },
        );
        self.commit(changes);
    }

    fn add_ether_balance(
        &mut self,
        address: Address,
        amount: U256,
    ) -> Result<(), SoflError> {
        let mut account_info = self
            .basic(address)
            .map_err(|e| {
                SoflError::BcState(format!(
                    "failed to get account basic: {:?}",
                    e
                ))
            })?
            .unwrap_or_default();
        account_info.balance += amount;
        let account = Account {
            info: account_info,
            storage: Default::default(),
            status: AccountStatus::Touched,
        };
        let mut changes = StateChange::new();
        changes.insert(address, account);
        self.commit(changes);
        Ok(())
    }

    /// Replace the code of an account.
    /// Returns the original code.
    fn replace_account_code(
        &mut self,
        address: Address,
        code: Bytecode,
    ) -> Result<Bytecode, SoflError> {
        let mut account_info = self
            .basic(address)
            .map_err(|e| {
                SoflError::BcState(format!(
                    "failed to get account basic: {:?}",
                    e
                ))
            })?
            .unwrap_or_default();
        let original_code = account_info.code.unwrap_or_default();
        account_info.code_hash = code.hash_slow();
        account_info.code = Some(code);
        let account = Account {
            info: account_info,
            storage: Default::default(),
            status: AccountStatus::Touched,
        };
        let mut changes = StateChange::new();
        changes.insert(address, account);
        self.commit(changes);
        Ok(original_code)
    }
}

/// Any type that implements revm::Database auto-implements BcState.
impl<T: revm::Database + revm::DatabaseCommit> BcState for T
where
    T::Error: std::fmt::Debug,
{
    type DatabaseErr = T::Error;
}

// /// BcState wraps revm's DatabaseCommit trait.
// /// It provides a set of basic methods to edit the state of the blockchain.
// pub trait BcStateEditable: BcState + revm::DatabaseCommit
// where
//     Self::Error: std::fmt::Debug,
// {
// }

// /// Any type that implements BcState and revm::DatabaseCommit auto-implements BcStateEditable.
// impl<T: revm::DatabaseCommit + BcState> BcStateEditable for T where T::Error: std::fmt::Debug {}
