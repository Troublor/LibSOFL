use ethers::abi::{ParamType, Token};
use reth_primitives::Address;
use revm_primitives::U256;

use crate::{engine::state::BcState, error::SoflError};

use super::CheatCodes;

pub trait ERC20Cheat<S: BcState> {
    fn get_erc20_balance(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
    ) -> Result<U256, SoflError<S::DbErr>>;

    fn get_erc20_total_supply(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<S::DbErr>>;

    fn get_erc20_decimals(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<S::DbErr>>;

    fn set_erc20_balance(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError<S::DbErr>>;
}

// cheatcodes: get functions
impl<S: BcState> ERC20Cheat<S> for CheatCodes<S> {
    fn get_erc20_balance(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
    ) -> Result<U256, SoflError<S::DbErr>> {
        // signature: balanceOf(address) -> 0x70a08231
        let result = self.cheat_read(
            state,
            token,
            0x70a08231u32,
            &[Token::Address(account.into())],
            &[ParamType::Uint(256)],
        )?;

        Ok(result[0].clone().into_uint().expect("cannot fail").into())
    }

    fn get_erc20_total_supply(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<S::DbErr>> {
        // signature: totalSupply() -> 0x18160ddd
        let result = self.cheat_read(
            state,
            token,
            0x18160dddu32,
            &[],
            &[ParamType::Uint(256)],
        )?;

        Ok(result[0].clone().into_uint().expect("cannot fail").into())
    }

    fn get_erc20_decimals(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<S::DbErr>> {
        // signature: decimals() -> 0x313ce567
        let result = self.cheat_read(
            state,
            token,
            0x313ce567u32,
            &[],
            &[ParamType::Uint(256)],
        )?;

        Ok(result[0].clone().into_uint().expect("cannot fail").into())
    }

    // return the old balance if updated
    fn set_erc20_balance(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError<S::DbErr>> {
        // signature: balanceOf(address) -> 0x70a08231
        if let Some(old_balance) = self.cheat_write(
            state,
            token,
            0x70a08231u32,
            &[Token::Address(account.into())],
            balance,
        )? {
            // we need to update total supply
            let total_supply = self.get_erc20_total_supply(state, token)?;

            // signature: totalSupply() -> 0x18160ddd
            self.cheat_write(
                state,
                token,
                0x18160dddu32,
                &[],
                total_supply + balance - old_balance,
            )?;

            Ok(Some(old_balance))
        } else {
            Ok(None)
        }
    }
}
