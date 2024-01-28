use std::sync::Arc;

use revm::db::CacheDB;

use super::types::{
    AccountInfo, Address, BcStateRef, Bytecode, Hash, StateChange, U256,
};

/// In-memory BcState implementation, using revm's CacheDB.
#[derive(
    Debug, Clone, derive_more::AsRef, derive_more::Deref, derive_more::DerefMut,
)]
pub struct MemoryBcState<S: BcStateRef>(
    #[as_ref]
    #[deref]
    #[deref_mut]
    revm::db::CacheDB<Arc<S>>,
);

impl<S: BcStateRef> revm::Database for MemoryBcState<S> {
    type Error = S::Error;

    #[doc = " Get basic account information."]
    fn basic(
        &mut self,
        address: Address,
    ) -> Result<Option<AccountInfo>, Self::Error> {
        self.0.basic(address)
    }

    #[doc = " Get account code by its hash."]
    fn code_by_hash(
        &mut self,
        code_hash: Hash,
    ) -> Result<Bytecode, Self::Error> {
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

    #[doc = " Get block hash by block number."]
    fn block_hash(&mut self, number: U256) -> Result<Hash, Self::Error> {
        self.0.block_hash(number)
    }
}

impl<S: BcStateRef> revm::DatabaseCommit for MemoryBcState<S> {
    #[doc = " Commit changes to the database."]
    fn commit(&mut self, changes: StateChange) {
        self.0.commit(changes)
    }
}

// impl<S: revm::DatabaseRef> BcState for MemoryBcState<S>
// where
//     S::Error: std::fmt::Debug
// {
//     type DatabaseErr = S::Error;
// }

impl<S: BcStateRef> MemoryBcState<S> {
    pub fn new(state_ref: S) -> Self {
        let state_ref = Arc::new(state_ref);
        Self(revm::db::CacheDB::new(state_ref))
    }
}

pub type EmptyMemoryBcState = MemoryBcState<revm::db::EmptyDB>;

impl MemoryBcState<revm::db::EmptyDB> {
    pub fn fresh() -> MemoryBcState<revm::db::EmptyDB> {
        let empty = revm::db::EmptyDB::default();
        MemoryBcState::new(empty)
    }
}

impl<S: BcStateRef> MemoryBcState<S> {
    /// fork a new MemoryBcState from the current state.
    pub fn fork(&self) -> MemoryBcState<CacheDB<Arc<S>>> {
        let c = self.0.clone();
        MemoryBcState::new(c)
    }
}

#[cfg(test)]
mod tests {
    use revm::Database;

    use crate::{
        conversion::ConvertTo,
        engine::{
            inspector::no_inspector,
            memory::MemoryBcState,
            state::BcState,
            transition::TransitionSpecBuilder,
            types::{
                AccountInfo, Address, BlockEnv, Bytecode, CfgEnv,
                ExecutionResult, TransactTo, TxEnv, U256,
            },
        },
    };

    #[test]
    fn test_fresh_state_with_plain_transfer() {
        let spender: Address = 0.cvt();
        let receiver: Address = 1.cvt();

        // set cfg and env
        let mut cfg = CfgEnv::default();
        cfg.disable_block_gas_limit = true;
        cfg.disable_base_fee = true;

        let block_env = BlockEnv {
            gas_limit: U256::from(1000000),
            ..Default::default()
        };

        // create state
        let mut state = MemoryBcState::fresh();
        {
            let acc = AccountInfo::new(
                U256::from(1000),
                Default::default(),
                Default::default(),
                Bytecode::new(),
            );
            state.insert_account_info(spender, acc);
            let acc = AccountInfo::new(
                U256::from(0),
                Default::default(),
                Default::default(),
                Bytecode::new(),
            );
            state.insert_account_info(receiver, acc);
        }

        let mut tx = TxEnv::default();
        tx.caller = spender;
        tx.transact_to = TransactTo::Call(receiver);
        tx.value = 500u128.cvt();
        tx.gas_limit = 100000;

        // simulate
        let spec = TransitionSpecBuilder::new()
            .set_cfg(cfg.clone())
            .set_block(block_env.clone())
            .append_tx_env(tx.clone())
            .build();
        let (_, mut results) = state.simulate(spec, no_inspector()).unwrap();
        let result = results.pop().unwrap();

        assert!(matches!(result, ExecutionResult::Success { .. }));
        let spender_balance = state.basic(spender).unwrap().unwrap().balance;
        assert_eq!(
            spender_balance,
            U256::from(1000),
            "spender balance should be unchanged in simulation"
        );
        let receiver_balance = state.basic(receiver).unwrap().unwrap().balance;
        assert_eq!(
            receiver_balance,
            U256::from(0),
            "receiver balance should be unchanged in simulation"
        );

        // transact
        let spec = TransitionSpecBuilder::new()
            .set_cfg(cfg)
            .set_block(block_env)
            .append_tx_env(tx)
            .build();
        let mut result = state.transit_without_inspector(spec).unwrap();
        let result = result.pop().unwrap();

        assert!(matches!(result, ExecutionResult::Success { .. }));
        let spender_balance = state.basic(spender).unwrap().unwrap().balance;
        assert_eq!(
            spender_balance,
            U256::from(500),
            "spender balance should be decreased by 500"
        );
        let receiver_balance = state.basic(receiver).unwrap().unwrap().balance;
        assert_eq!(
            receiver_balance,
            U256::from(500),
            "receiver balance should be increased by 500"
        );
    }
}
