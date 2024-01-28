use libsofl_core::engine::types::{
    AccountInfo, Address, Bytecode, DatabaseRef, B256, U256,
};
use reth_provider::{ProviderError, StateProviderBox};
use reth_revm::{
    database::StateProviderDatabase, DatabaseRef as reth_DatabaseRef,
};

use crate::conversion::ConvertTo;

pub struct RethBcStateRef {
    pub reth_db: StateProviderDatabase<StateProviderBox>,
}

impl From<StateProviderDatabase<StateProviderBox>> for RethBcStateRef {
    fn from(reth_db: StateProviderDatabase<StateProviderBox>) -> Self {
        Self { reth_db }
    }
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
            Ok(Some(account.cvt()))
        } else {
            Ok(None)
        }
    }

    #[doc = r" Get account code by its hash."]
    fn code_by_hash_ref(
        &self,
        code_hash: B256,
    ) -> Result<Bytecode, Self::Error> {
        let code = self.reth_db.code_by_hash_ref(code_hash)?;
        Ok(code.cvt())
    }

    #[doc = r" Get storage value of address at index."]
    fn storage_ref(
        &self,
        address: Address,
        index: U256,
    ) -> Result<U256, Self::Error> {
        let storage = self.reth_db.storage_ref(address, index)?;
        Ok(storage.cvt())
    }

    #[doc = r" Get block hash by block number."]
    fn block_hash_ref(&self, number: U256) -> Result<B256, Self::Error> {
        let block_hash = self.reth_db.block_hash_ref(number)?;
        Ok(block_hash.cvt())
    }
}
