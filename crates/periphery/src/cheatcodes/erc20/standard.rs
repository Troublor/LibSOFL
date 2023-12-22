use std::{any::type_name, fmt::Debug};

use alloy_sol_types::SolCall;
use libsofl_core::{
    conversion::ConvertTo,
    engine::{
        inspector::no_inspector,
        state::{BcState, BcStateEditable},
        types::{Address, U256},
    },
    error::SoflError,
};

use crate::{
    addressbook::{ADDRESS_BOOK, ERC20ABI},
    cheatcodes::CheatCodes,
    types::Chain,
};

impl CheatCodes {
    pub fn get_erc20_balance<S>(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
    ) -> Result<U256, SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        // signature: balanceOf(address) -> 0x70a08231
        let call = ERC20ABI::balanceOfCall { owner: account };
        let calldata = call.abi_encode();
        let ret = self.cheat_read(state, token, calldata.cvt())?;
        ERC20ABI::balanceOfCall::abi_decode_returns(&ret, true)
            .map(|r| r.balance)
            .map_err(|e| SoflError::Abi(format!("failed to decode balanceOf return: {}", e)))
    }

    pub fn get_erc20_total_supply<S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        // signature: totalSupply() -> 0x18160ddd
        let call = ERC20ABI::totalSupplyCall {};
        let calldata = call.abi_encode();
        let ret = self.cheat_read(state, token, calldata.cvt())?;
        ERC20ABI::totalSupplyCall::abi_decode_returns(&ret, true)
            .map(|r| r._0)
            .map_err(|e| SoflError::Abi(format!("failed to decode totalSupply return: {}", e)))
    }

    pub fn get_erc20_decimals<S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        // signature: decimals() -> 0x313ce567
        let call = ERC20ABI::decimalsCall {};
        let calldata = call.abi_encode();
        let ret = self.cheat_read(state, token, calldata.cvt())?;
        ERC20ABI::decimalsCall::abi_decode_returns(&ret, true)
            .map(|r| r._0.cvt())
            .map_err(|e| SoflError::Abi(format!("failed to decode decimals return: {}", e)))
    }

    pub fn get_erc20_allowance<S>(
        &mut self,
        state: &mut S,
        token: Address,
        owner: Address,
        spender: Address,
    ) -> Result<U256, SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        // signature: allowance(address,address) -> 0xdd62ed3e
        let call = ERC20ABI::allowanceCall {
            owner: owner,
            spender: spender,
        };
        let calldata = call.abi_encode();
        let ret = self.cheat_read(state, token, calldata.cvt())?;
        ERC20ABI::allowanceCall::abi_decode_returns(&ret, true)
            .map(|r| r._0)
            .map_err(|e| SoflError::Abi(format!("failed to decode allowance return: {}", e)))
    }

    // return the old allowance if updated
    pub fn set_erc20_allowance<S>(
        &mut self,
        state: &mut S,
        token: Address,
        owner: Address,
        spender: Address,
        allowance: U256,
    ) -> Result<Option<U256>, SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        // signature: allowance(address,address) -> 0xdd62ed3e
        let call = ERC20ABI::allowanceCall {
            owner: owner,
            spender: spender,
        };
        let calldata = call.abi_encode();
        self.cheat_write(state, token, calldata.cvt(), allowance)
    }

    pub fn steal_erc20<S>(
        &mut self,
        state: &mut S,
        token: Address,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Result<(), SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        // signature: transferFrom(address,address,uint256) -> 0x23b872dd
        let call = ERC20ABI::transferCall { to, value: amount };
        let calldata = call.abi_encode();

        // get the balance of the sender
        let balance_before = self.get_erc20_balance(state, token, from)?;

        let mut caller = self.caller.clone();
        caller.address = from;
        caller.call(state, token, calldata.cvt(), None, no_inspector())?;

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

    pub fn increase_erc20_balance_by<S>(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
        amount: U256,
    ) -> Result<Option<U256>, SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        let balance_before = self.get_erc20_balance(state, token, account)?;
        let balance_after = balance_before + amount;
        self.set_erc20_balance(state, token, account, balance_after)
    }

    // return the old balance if updated
    pub fn set_erc20_balance<S>(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        // first check whether the token is a LP token for a DEX pool
        let token_ty = self.get_contract_type(state, token)?;
        if token_ty.is_lp_token() {
            return self.set_lp_token_balance(state, token_ty, token, account, balance);
        } else if token_ty.is_pegged_token() {
            return self.set_pegged_token_balance(state, token_ty, token, account, balance);
        }

        // signature: balanceOf(address) -> 0x70a08231
        let call = ERC20ABI::balanceOfCall { owner: account };
        if let Some(old_balance) =
            self.cheat_write(state, token, call.abi_encode().cvt(), balance)?
        {
            // we need to update total supply
            let total_supply = self.get_erc20_total_supply(state, token)?;

            let weth = ADDRESS_BOOK.weth.must_on_chain(Chain::Mainnet);
            if token == weth {
                self.set_balance(state, weth, total_supply + balance - old_balance)?;
            } else {
                // signature: totalSupply() -> 0x18160ddd
                let call = ERC20ABI::totalSupplyCall {};
                self.cheat_write(
                    state,
                    token,
                    call.abi_encode().cvt(),
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
mod tests_with_dep {
    use alloy_sol_types::SolCall;
    use libsofl_core::{
        blockchain::tx_position::TxPosition,
        conversion::ConvertTo,
        engine::{
            inspector::no_inspector,
            types::{Address, U256},
        },
    };

    use crate::{
        addressbook::{ADDRESS_BOOK, ERC20ABI},
        caller::HighLevelCaller,
        cheatcodes::CheatCodes,
        test::get_test_bc_provider,
        types::Chain,
    };

    fn eval(account: Address, token: Address, decimals: U256) {
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let balance_before = cheatcodes
            .get_erc20_balance(&mut state, token, account)
            .unwrap();

        let total_supply_before = cheatcodes
            .get_erc20_total_supply(&mut state, token)
            .unwrap();

        assert!(cheatcodes.get_erc20_decimals(&mut state, token).unwrap() == decimals);

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
        assert!(total_supply_after == total_supply_before - balance_before + balance_after);

        let spender = "0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a".cvt();
        cheatcodes
            .set_erc20_allowance(&mut state, token, account, spender, U256::from(7654321))
            .unwrap();

        let allowance_after = cheatcodes
            .get_erc20_allowance(&mut state, token, account, spender)
            .unwrap();

        assert!(allowance_after == U256::from(7654321));
    }

    #[test]
    fn test_erc20() {
        let account = "0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a".cvt();

        let usdc = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".cvt();

        eval(account, usdc, U256::from(6));
    }

    #[test]
    fn test_weth() {
        let account = "0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a".cvt();

        let weth = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".cvt();

        eval(account, weth, U256::from(18));
    }

    #[test]
    fn test_usdt() {
        let account1 = "0xF977814e90dA44bFA03b6295A0616a897441acee".cvt();
        let account2 = "0xD51a44d3FaE010294C616388b506AcdA1bfAAE45".cvt();

        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(14972421, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let usdt = ADDRESS_BOOK.usdt.must_on_chain(Chain::Mainnet);
        let balance1 = cheatcodes
            .get_erc20_balance(&mut state, usdt, account1)
            .unwrap();
        assert_eq!(balance1, U256::from(0));

        cheatcodes
            .set_erc20_balance(&mut state, usdt, account1, U256::from(10u64.pow(12)))
            .unwrap();
        let balance2 = cheatcodes
            .get_erc20_balance(&mut state, usdt, account1)
            .unwrap();
        assert_eq!(balance2, U256::from(10u64.pow(12)));

        cheatcodes
            .set_erc20_allowance(&mut state, usdt, account1, account2, U256::MAX)
            .unwrap();

        let call = ERC20ABI::transferFromCall {
            from: account1,
            to: account2,
            value: U256::from(10u64.pow(11)),
        };
        let caller = HighLevelCaller::new(account2)
            .bypass_check()
            .at_block(&bp, fork_at.block);
        caller
            .call(
                &mut state,
                usdt,
                call.abi_encode().cvt(),
                None,
                no_inspector(),
            )
            .unwrap();

        let balance3 = cheatcodes
            .get_erc20_balance(&mut state, usdt, account1)
            .unwrap();
        assert_eq!(balance3, U256::from(10u64.pow(12) - 10u64.pow(11)));

        let balance4 = cheatcodes
            .get_erc20_balance(&mut state, usdt, account2)
            .unwrap();
        assert_eq!(balance4, U256::from(10u64.pow(11)));
    }

    #[test]
    fn test_steal() {
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(14972421, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let deposit_amount = 300479464706193878654u128.cvt();
        let yv_curve_3crypto_token = "0xE537B5cc158EB71037D4125BDD7538421981E6AA".cvt();
        let yv_curve_3crypto_richer = "0xA67EC8737021A7e91e883A3277384E6018BB5776".cvt();
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
