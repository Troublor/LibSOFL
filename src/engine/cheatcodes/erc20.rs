use std::fmt::Debug;

use ethers::abi::Token;
use reth_primitives::Address;
use revm::{Database, DatabaseCommit};
use revm_primitives::U256;

use crate::{
    engine::state::DatabaseEditable,
    error::SoflError,
    unwrap_first_token_value,
    utils::{abi::ERC20_ABI, addresses::WETH},
};

use super::CheatCodes;

impl CheatCodes {
    pub fn get_erc20_balance<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
    ) -> Result<U256, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // signature: balanceOf(address) -> 0x70a08231
        let func = ERC20_ABI
            .function("balanceOf")
            .expect("bug: cannot find balanceOf function in ERC20 ABI");
        Ok(unwrap_first_token_value!(
            Uint,
            self.cheat_read(
                state,
                token,
                func,
                &[Token::Address(account.into())],
            )?
        ))
    }

    pub fn get_erc20_total_supply<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // signature: totalSupply() -> 0x18160ddd
        let func = ERC20_ABI
            .function("totalSupply")
            .expect("bug: cannot find totalSupply function in ERC20 ABI");
        Ok(unwrap_first_token_value!(
            Uint,
            self.cheat_read(state, token, func, &[])?
        ))
    }

    pub fn get_erc20_decimals<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // signature: decimals() -> 0x313ce567
        let func = ERC20_ABI
            .function("decimals")
            .expect("bug: cannot find decimals function in ERC20 ABI");
        Ok(unwrap_first_token_value!(
            Uint,
            self.cheat_read(state, token, func, &[])?
        ))
    }

    pub fn get_erc20_allowance<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
        owner: Address,
        spender: Address,
    ) -> Result<U256, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // signature: allowance(address,address) -> 0xdd62ed3e
        let func = ERC20_ABI
            .function("allowance")
            .expect("bug: cannot find allowance function in ERC20 ABI");
        Ok(unwrap_first_token_value!(
            Uint,
            self.cheat_read(
                state,
                token,
                func,
                &[Token::Address(owner.into()), Token::Address(spender.into())]
            )?
        ))
    }

    // return the old allowance if updated
    pub fn set_erc20_allowance<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
        owner: Address,
        spender: Address,
        allowance: U256,
    ) -> Result<Option<U256>, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // signature: allowance(address,address) -> 0xdd62ed3e
        let func = ERC20_ABI
            .function("allowance")
            .expect("bug: cannot find allowance function in ERC20 ABI");
        self.cheat_write(
            state,
            token,
            func,
            &[Token::Address(owner.into()), Token::Address(spender.into())],
            allowance,
        )
    }

    // return the old balance if updated
    pub fn set_erc20_balance<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
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

            if token == *WETH {
                self.set_balance(
                    state,
                    *WETH,
                    total_supply + balance - old_balance,
                )?;
            } else {
                // signature: totalSupply() -> 0x18160ddd
                let func = ERC20_ABI.function("totalSupply").expect(
                    "bug: cannot find totalSupply function in ERC20 ABI",
                );
                self.cheat_write(
                    state,
                    token,
                    func,
                    &[],
                    total_supply + balance - old_balance,
                )?;
            }

            Ok(Some(old_balance))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests_with_db {
    use std::{path::Path, str::FromStr};

    use reth_primitives::Address;
    use revm_primitives::U256;

    use crate::engine::cheatcodes::CheatCodes;
    use crate::engine::state::BcStateBuilder;
    use crate::{
        config::flags::SoflConfig,
        engine::{
            providers::BcProviderBuilder, transactions::position::TxPosition,
        },
    };

    fn eval(account: Address, token: Address, decimals: U256) {
        let datadir = SoflConfig::load().unwrap().reth.datadir;
        let datadir = Path::new(&datadir);
        let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let balance_before = cheatcodes
            .get_erc20_balance(&mut state, token, account)
            .unwrap();

        let total_supply_before = cheatcodes
            .get_erc20_total_supply(&mut state, token)
            .unwrap();

        assert!(
            cheatcodes.get_erc20_decimals(&mut state, token).unwrap()
                == decimals
        );

        cheatcodes
            .set_erc20_balance(&mut state, token, account, U256::from(1234567))
            .unwrap();
        let balance_after = cheatcodes
            .get_erc20_balance(&mut state, token, account)
            .unwrap();

        let total_supply_after = cheatcodes
            .get_erc20_total_supply(&mut state, token)
            .unwrap();

        assert!(balance_after == U256::from(1234567));
        assert!(
            total_supply_after
                == total_supply_before - balance_before + balance_after
        );

        let spender =
            Address::from_str("0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a")
                .unwrap();
        cheatcodes
            .set_erc20_allowance(
                &mut state,
                token,
                account,
                spender,
                U256::from(7654321),
            )
            .unwrap();
        let allowance_after = cheatcodes
            .get_erc20_allowance(&mut state, token, account, spender)
            .unwrap();
        assert!(allowance_after == U256::from(7654321));
    }

    #[test]
    fn test_erc20() {
        let account =
            Address::from_str("0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a")
                .unwrap();

        let usdc =
            Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
                .unwrap();

        eval(account, usdc, U256::from(6));
    }

    #[test]
    fn test_weth() {
        let account =
            Address::from_str("0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a")
                .unwrap();

        let weth =
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                .unwrap();

        eval(account, weth, U256::from(18));
    }
}

#[cfg(test)]
mod tests_with_jsonrpc {
    use std::str::FromStr;

    use reth_primitives::Address;
    use revm_primitives::U256;

    use crate::engine::cheatcodes::CheatCodes;
    use crate::engine::providers::rpc::JsonRpcBcProvider;
    use crate::engine::state::BcStateBuilder;
    use crate::engine::transactions::position::TxPosition;

    fn eval(account: Address, token: Address, decimals: U256) {
        let bp = JsonRpcBcProvider::default();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let balance_before = cheatcodes
            .get_erc20_balance(&mut state, token, account)
            .unwrap();

        let total_supply_before = cheatcodes
            .get_erc20_total_supply(&mut state, token)
            .unwrap();

        assert!(
            cheatcodes.get_erc20_decimals(&mut state, token).unwrap()
                == decimals
        );

        cheatcodes
            .set_erc20_balance(&mut state, token, account, U256::from(1234567))
            .unwrap();
        let balance_after = cheatcodes
            .get_erc20_balance(&mut state, token, account)
            .unwrap();

        let total_supply_after = cheatcodes
            .get_erc20_total_supply(&mut state, token)
            .unwrap();

        assert!(balance_after == U256::from(1234567));
        assert!(
            total_supply_after
                == total_supply_before - balance_before + balance_after
        );

        let spender =
            Address::from_str("0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a")
                .unwrap();
        cheatcodes
            .set_erc20_allowance(
                &mut state,
                token,
                account,
                spender,
                U256::from(7654321),
            )
            .unwrap();

        let allowance_after = cheatcodes
            .get_erc20_allowance(&mut state, token, account, spender)
            .unwrap();

        assert!(allowance_after == U256::from(7654321));
    }

    #[test]
    fn test_erc20() {
        let account =
            Address::from_str("0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a")
                .unwrap();

        let usdc =
            Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
                .unwrap();

        eval(account, usdc, U256::from(6));
    }

    #[test]
    fn test_weth() {
        let account =
            Address::from_str("0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a")
                .unwrap();

        let weth =
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                .unwrap();

        eval(account, weth, U256::from(18));
    }
}
