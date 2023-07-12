use std::{any::type_name, fmt::Debug};

use ethers::abi::Token;
use reth_primitives::Address;
use revm::{Database, DatabaseCommit};
use revm_primitives::U256;

use crate::{
    engine::{inspectors::no_inspector, state::DatabaseEditable},
    error::SoflError,
    unwrap_first_token_value,
    utils::{abi::ERC20_ABI, addresses::WETH},
};

use crate::engine::cheatcodes::CheatCodes;

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

    pub fn steal_erc20<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Result<(), SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // signature: transferFrom(address,address,uint256) -> 0x23b872dd
        let func = ERC20_ABI
            .function("transfer")
            .expect("bug: cannot find transferFrom function in ERC20 ABI");

        // get the balance of the sender
        let balance_before = self.get_erc20_balance(state, token, from)?;

        let mut caller = self.caller.clone();
        caller.address = from;
        caller.invoke_ignore_return(
            state,
            token,
            func,
            &[Token::Address(to.into()), Token::Uint(amount.into())],
            None,
            no_inspector(),
        )?;

        // get the balance of the sender
        let balance_after = self.get_erc20_balance(state, token, from)?;

        if balance_after != balance_before - amount {
            Err(SoflError::Custom(format!(
                "{}: cannot steal ERC20",
                type_name::<Self>()
            )))
        } else {
            Ok(())
        }
    }

    pub fn increase_erc20_balance_by<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
        amount: U256,
    ) -> Result<Option<U256>, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        let balance_before = self.get_erc20_balance(state, token, account)?;
        let balance_after = balance_before + amount;
        self.set_erc20_balance(state, token, account, balance_after)
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
        // first check whether the token is a LP token for a DEX pool
        let token_ty = self.get_contract_type(state, token)?;
        if token_ty.is_lp_token() {
            return self.set_lp_token_balance(
                state, token_ty, token, account, balance,
            );
        } else if token_ty.is_pegged_token() {
            return self.set_pegged_token_balance(
                state, token_ty, token, account, balance,
            );
        }

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
mod tests_with_dep {
    use std::str::FromStr;

    use ethers::abi::Token;
    use reth_primitives::Address;
    use revm_primitives::U256;

    use crate::engine::cheatcodes::CheatCodes;
    use crate::engine::inspectors::no_inspector;
    use crate::engine::state::BcStateBuilder;
    use crate::engine::transactions::position::TxPosition;
    use crate::engine::utils::HighLevelCaller;
    use crate::utils::abi::ERC20_ABI;
    use crate::utils::addresses::USDT;
    use crate::utils::conversion::{Convert, ToPrimitive};
    use crate::utils::testing::get_testing_bc_provider;

    fn eval(account: Address, token: Address, decimals: U256) {
        let bp = get_testing_bc_provider();

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

    #[test]
    fn test_usdt() {
        let account1 =
            Address::from_str("0xF977814e90dA44bFA03b6295A0616a897441acee")
                .unwrap();
        let account2 =
            Address::from_str("0xD51a44d3FaE010294C616388b506AcdA1bfAAE45")
                .unwrap();

        let bp = get_testing_bc_provider();

        let fork_at = TxPosition::new(14972421, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let balance1 = cheatcodes
            .get_erc20_balance(&mut state, *USDT, account1)
            .unwrap();
        assert_eq!(balance1, U256::from(0));

        cheatcodes
            .set_erc20_balance(
                &mut state,
                *USDT,
                account1,
                U256::from(10u64.pow(12)),
            )
            .unwrap();
        let balance2 = cheatcodes
            .get_erc20_balance(&mut state, *USDT, account1)
            .unwrap();
        assert_eq!(balance2, U256::from(10u64.pow(12)));

        cheatcodes
            .set_erc20_allowance(
                &mut state,
                *USDT,
                account1,
                account2,
                U256::MAX,
            )
            .unwrap();

        let func = ERC20_ABI.function("transferFrom").unwrap();
        let caller = HighLevelCaller::new(account2)
            .bypass_check()
            .at_block(&bp, fork_at.block);
        caller
            .invoke_ignore_return(
                &mut state,
                *USDT,
                func,
                &[
                    Token::Address(account1.into()),
                    Token::Address(account2.into()),
                    Token::Uint((10u64.pow(11)).into()),
                ],
                None,
                no_inspector(),
            )
            .unwrap();

        let balance3 = cheatcodes
            .get_erc20_balance(&mut state, *USDT, account1)
            .unwrap();
        assert_eq!(balance3, U256::from(10u64.pow(12) - 10u64.pow(11)));

        let balance4 = cheatcodes
            .get_erc20_balance(&mut state, *USDT, account2)
            .unwrap();
        assert_eq!(balance4, U256::from(10u64.pow(11)));
    }

    #[test]
    fn test_steal() {
        let bp = get_testing_bc_provider();

        let fork_at = TxPosition::new(14972421, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let deposit_amount = ToPrimitive::cvt(300479464706193878654u128);
        let yv_curve_3crypto_token =
            ToPrimitive::cvt("0xE537B5cc158EB71037D4125BDD7538421981E6AA");
        let yv_curve_3crypto_richer =
            ToPrimitive::cvt("0xA67EC8737021A7e91e883A3277384E6018BB5776");
        cheatcodes
            .steal_erc20(
                &mut state,
                yv_curve_3crypto_token,
                yv_curve_3crypto_richer,
                cheatcodes.caller.address,
                deposit_amount,
            )
            .unwrap();
    }
}
