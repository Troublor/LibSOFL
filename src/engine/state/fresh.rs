use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};

use reth_primitives::Address;
use revm::{
    db::{CacheDB, EmptyDB},
    Database, DatabaseCommit,
};
use revm_primitives::{
    db::DatabaseRef, Account, AccountInfo, Bytecode, HashMap, B160, B256, U256,
};

/// A blockchain state that is empty and complete in memory.
#[derive(Debug)]
pub struct FreshBcState(CacheDB<EmptyDB>);

impl FreshBcState {
    /// Create a new empty blockchain state.
    pub fn new() -> Self {
        Self(CacheDB::new(EmptyDB::default()))
    }
}

impl Default for FreshBcState {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for FreshBcState {
    type Target = CacheDB<EmptyDB>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FreshBcState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<CacheDB<EmptyDB>> for FreshBcState {
    fn as_ref(&self) -> &CacheDB<EmptyDB> {
        &self.0
    }
}

impl AsMut<CacheDB<EmptyDB>> for FreshBcState {
    fn as_mut(&mut self) -> &mut CacheDB<EmptyDB> {
        todo!()
    }
}

impl Database for FreshBcState {
    type Error = Infallible;

    #[doc = " Get basic account information."]
    fn basic(
        &mut self,
        address: Address,
    ) -> Result<Option<AccountInfo>, Self::Error> {
        self.0.basic(address)
    }

    #[doc = " Get account code by its hash"]
    fn code_by_hash(
        &mut self,
        code_hash: B256,
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

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        self.0.block_hash(number)
    }
}

impl DatabaseCommit for FreshBcState {
    fn commit(&mut self, changes: HashMap<B160, Account>) {
        self.0.commit(changes)
    }
}

impl DatabaseRef for FreshBcState {
    type Error = Infallible;

    #[doc = " Whether account at address exists."]
    #[doc = " Get basic account information."]
    fn basic(&self, address: B160) -> Result<Option<AccountInfo>, Self::Error> {
        self.0.basic(address)
    }

    #[doc = " Get account code by its hash"]
    fn code_by_hash(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.0.code_by_hash(code_hash)
    }

    #[doc = " Get storage value of address at index."]
    fn storage(&self, address: B160, index: U256) -> Result<U256, Self::Error> {
        self.0.storage(address, index)
    }

    fn block_hash(&self, number: U256) -> Result<B256, Self::Error> {
        self.0.block_hash(number)
    }
}

#[cfg(test)]
mod tests_nodep {

    use reth_primitives::{Transaction, TransactionKind, TxLegacy};

    use revm::Database;
    use revm_primitives::{
        Account, AccountInfo, Address, BlockEnv, Bytecode, Bytes, CfgEnv,
        ExecutionResult, U256,
    };

    use crate::engine::{
        state::{no_inspector, BcState},
        transaction::{StateChange, Tx, TxOrPseudo},
    };

    use super::FreshBcState;

    #[test]
    fn test_fresh_state_with_plain_transfer() {
        let spender = Address::from(0);
        let receiver = Address::from(1);

        // set cfg and env
        let cfg = CfgEnv {
            disable_block_gas_limit: true,
            disable_base_fee: true,
            ..Default::default()
        };
        let block_env = BlockEnv {
            gas_limit: U256::from(1000000),
            ..Default::default()
        };

        // create state
        let mut state = FreshBcState::new();
        {
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
        }

        let tx_inner = Transaction::Legacy(TxLegacy {
            to: TransactionKind::Call(receiver),
            value: 500,
            gas_limit: 100000,
            ..Default::default()
        });
        let tx = Tx::Unsigned((spender, tx_inner.clone()));

        // simulate
        let result = state
            .transact(cfg.clone(), block_env.clone(), tx, no_inspector())
            .unwrap()
            .result;

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
        let tx = Tx::Unsigned((spender, tx_inner));
        let (mut state, result) = state
            .transit_one(cfg, block_env, tx, no_inspector())
            .unwrap();

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

    #[test]
    fn test_pseudo_tx() {
        let account = Address::from(0);
        let tx_lambda = |_state: &FreshBcState| {
            let mut changes = StateChange::default();
            let mut change = Account::new_not_existing();
            change.is_not_existing = false;
            change.info.balance = U256::from(1000);
            change.info.code = Some(Bytecode::new_raw(Bytes::from("0x1234")));
            changes.insert(account, change);
            changes
        };
        let tx = TxOrPseudo::Pseudo(&tx_lambda);

        let state = FreshBcState::new();
        let (mut state, result) = state
            .transit_one(
                CfgEnv::default(),
                BlockEnv::default(),
                tx,
                no_inspector(),
            )
            .unwrap();

        assert!(matches!(result, ExecutionResult::Success { .. }));
        let balance = state.basic(account).unwrap().unwrap().balance;
        let code = state.basic(account).unwrap().unwrap().code;
        assert_eq!(balance, U256::from(1000), "account balance should be 1000");
        assert_eq!(
            code,
            Some(Bytecode::new_raw(Bytes::from("0x1234"))),
            "account code should be 0x1234"
        );
    }
}
