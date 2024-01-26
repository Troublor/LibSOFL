use libsofl_core::engine::types::{
    AccountInfo, Address, BcStateRef, Bytecode, DatabaseRef, B256, U256,
};
use reth_provider::{ProviderError, StateProviderBox};
use reth_revm::{
    database::StateProviderDatabase, DatabaseRef as reth_DatabaseRef,
};

#[derive(Debug, Clone)]
pub struct RethBcStateRef {
    pub reth_db: StateProviderDatabase<StateProviderBox>,
}

impl DatabaseRef for RethBcStateRef {
    #[doc = r" The database error type."]
    type Error = ProviderError;

    #[doc = r" Get basic account information."]
    fn basic_ref(
        &self,
        address: Address,
    ) -> Result<Option<AccountInfo>, Self::Error> {
        let account = self.reth_db.basic_ref(address)?;
        if let Some(account) = account {
            Ok(Some(account))
        } else {
            Ok(None)
        }
    }

    #[doc = r" Get account code by its hash."]
    fn code_by_hash_ref(
        &self,
        code_hash: B256,
    ) -> Result<Bytecode, Self::Error> {
        todo!()
    }

    #[doc = r" Get storage value of address at index."]
    fn storage_ref(
        &self,
        address: Address,
        index: U256,
    ) -> Result<U256, Self::Error> {
        todo!()
    }

    #[doc = r" Get block hash by block number."]
    fn block_hash_ref(&self, number: U256) -> Result<B256, Self::Error> {
        todo!()
    }
}
