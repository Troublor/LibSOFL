use std::fmt::Debug;

use ethers::abi::Token;
use reth_primitives::Address;
use revm::{Database, DatabaseCommit};
use revm_primitives::U256;

use crate::{
    engine::state::DatabaseEditable, error::SoflError, utils::abi::ERC20_ABI,
};

use super::CheatCodes;

pub trait ERC20Cheat<
    E,
    S: DatabaseEditable<Error = E> + Database<Error = E> + Database,
>
{
    fn get_erc20_balance(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
    ) -> Result<U256, SoflError<E>>;

    fn get_erc20_total_supply(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<E>>;

    fn get_erc20_decimals(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<E>>;

    fn set_erc20_balance(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError<E>>;
}

// cheatcodes: get functions
impl<
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    > ERC20Cheat<E, S> for CheatCodes<S>
{
    fn get_erc20_balance(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
    ) -> Result<U256, SoflError<E>> {
        // signature: balanceOf(address) -> 0x70a08231
        let func = ERC20_ABI
            .function("balanceOf")
            .expect("bug: cannot find balanceOf function in ERC20 ABI");
        let result = self.cheat_read(
            state,
            token,
            func,
            &[Token::Address(account.into())],
        )?;

        Ok(result[0].clone().into_uint().expect("cannot fail").into())
    }

    fn get_erc20_total_supply(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<E>> {
        // signature: totalSupply() -> 0x18160ddd
        let func = ERC20_ABI
            .function("totalSupply")
            .expect("bug: cannot find totalSupply function in ERC20 ABI");
        let result = self.cheat_read(state, token, func, &[])?;

        Ok(result[0].clone().into_uint().expect("cannot fail").into())
    }

    fn get_erc20_decimals(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<E>> {
        // signature: decimals() -> 0x313ce567
        let func = ERC20_ABI
            .function("decimals")
            .expect("bug: cannot find decimals function in ERC20 ABI");
        let result = self.cheat_read(state, token, func, &[])?;

        Ok(result[0].clone().into_uint().expect("cannot fail").into())
    }

    // return the old balance if updated
    fn set_erc20_balance(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError<E>> {
        // signature: balanceOf(address) -> 0x70a08231
        let func = ERC20_ABI
            .function("balanceOf")
            .expect("bug: cannot find balanceOf function in ERC20 ABI");
        if let Some(old_balance) = self.cheat_write(
            state,
            token,
            func,
            &[Token::Address(account.into())],
            balance,
        )? {
            // we need to update total supply
            let total_supply = self.get_erc20_total_supply(state, token)?;

            // signature: totalSupply() -> 0x18160ddd
            let funct = ERC20_ABI
                .function("totalSupply")
                .expect("bug: cannot find totalSupply function in ERC20 ABI");
            self.cheat_write(
                state,
                token,
                func,
                &[],
                total_supply + balance - old_balance,
            )?;

            Ok(Some(old_balance))
        } else {
            Ok(None)
        }
    }
}
