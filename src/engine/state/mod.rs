use auto_impl::auto_impl;
use reth_primitives::Address;
use revm_primitives::{AccountInfo, U256};

pub mod env;
pub mod fork;
pub mod fresh;
pub mod state;

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
