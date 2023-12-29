use revm_primitives::StorageSlot;

use crate::error::SoflError;
pub use revm::Database;
pub use revm::DatabaseCommit;
pub use revm::DatabaseRef;

use super::{
    inspector::EvmInspector,
    transition::TransitionSpec,
    types::{
        Account, AccountInfo, AccountStatus, Address, ExecutionResult,
        StateChange, Storage, U256,
    },
};

/// BcState is a wrapper of revm's Database trait.
/// It provides a set of basic methods to read the state of the blockchain.
pub trait BcState:
    revm::Database<Error = Self::DatabaseErr> + revm::DatabaseCommit
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
        let TransitionSpec { cfg, block, txs } = spec.into();
        let mut evm = revm::EVM::new();
        evm.env.cfg = cfg;
        evm.env.block = block;
        evm.database(self);
        let mut results = Vec::new();
        for tx in txs.into_iter() {
            let insp = &mut inspector;
            evm.env.tx = tx;

            // inspector pre-transaction hook
            if !insp.transaction(
                &evm.env.tx,
                evm.db
                    .as_ref()
                    .expect("impossible: db does not exists while database has been set in evm"),
            ) {
                // return false to skip transaction
                results.push(revm::primitives::ExecutionResult::Halt {
                    reason: revm::primitives::Halt::NotActivated,
                    gas_used: 0,
                });
                continue;
            }

            // execute transaction
            let result = evm.inspect_commit(insp).map_err(|e| match e {
                revm::primitives::EVMError::Transaction(ee) => {
                    SoflError::InvalidTransaction(ee)
                }
                revm::primitives::EVMError::Header(ee) => {
                    SoflError::InvalidHeader(ee)
                }
                revm::primitives::EVMError::Database(ee) => {
                    SoflError::BcState(format!("{:?}", ee))
                }
            })?;

            // inspector post-transaction hook
            (&mut inspector).transaction_end(
                &evm.env.tx,
                evm.db
                    .as_ref()
                    .expect("impossible: db does not exists while database has been set in evm"),
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
        let TransitionSpec { cfg, block, txs } = spec;
        let mut evm = revm::EVM::new();
        evm.env.cfg = cfg;
        evm.env.block = block;
        evm.database(self);
        let mut results = Vec::new();
        for tx in txs.into_iter() {
            evm.env.tx = tx;
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
            })?;
            // inspector post-transaction hook
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
        let TransitionSpec { cfg, block, txs } = spec;
        let mut evm = revm::EVM::new();
        evm.env.cfg = cfg;
        evm.env.block = block;
        evm.database(self);
        let mut results = Vec::new();
        let mut changes = Vec::new();
        for tx in txs.into_iter() {
            evm.env.tx = tx;

            let insp = &mut inspector;
            // inspector pre-transaction hook
            if !insp.transaction(
                &evm.env.tx,
                evm.db
                    .as_ref()
                    .expect("impossible: db does not exists while database has been set in evm"),
            ) {
                // return false to skip transaction
                let r = revm::primitives::ExecutionResult::Halt {
                    reason: revm::primitives::Halt::NotActivated,
                    gas_used: 0,
                };
                results.push(r);
                changes.push(revm::primitives::State::default());
                continue;
            }

            let revm::primitives::ResultAndState { result, state } =
                evm.inspect(insp).map_err(|e| match e {
                    revm::primitives::EVMError::Transaction(ee) => {
                        SoflError::InvalidTransaction(ee)
                    }
                    revm::primitives::EVMError::Header(ee) => {
                        SoflError::InvalidHeader(ee)
                    }
                    revm::primitives::EVMError::Database(ee) => {
                        SoflError::BcState(format!("{:?}", ee))
                    }
                })?;

            // inspector post-transaction hook
            (&mut inspector).transaction_end(
                &evm.env.tx,
                evm.db
                    .as_ref()
                    .expect("impossible: db does not exists while database has been set in evm"),
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
