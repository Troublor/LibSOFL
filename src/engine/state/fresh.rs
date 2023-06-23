use std::convert::Infallible;

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
