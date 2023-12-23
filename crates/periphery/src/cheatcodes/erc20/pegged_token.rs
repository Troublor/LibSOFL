use std::{any::type_name, fmt::Debug};

use alloy_dyn_abi::JsonAbiExt;
use alloy_sol_types::SolCall;
use libsofl_core::{
    conversion::ConvertTo,
    engine::{
        inspector::no_inspector,
        state::BcState,
        types::{Address, U256},
    },
    error::SoflError,
};

use crate::{
    addressbook::{
        AaveLendingPoolV2ABI, CurveYVaultABI, ADDRESS_BOOK, ERC20ABI,
    },
    cheatcodes::{contract_type::ContractType, CheatCodes},
    math::HPMultipler,
    types::Chain,
};

impl CheatCodes {
    pub fn set_pegged_token_balance<S>(
        &mut self,
        state: &mut S,
        token_ty: ContractType,
        token: Address,
        account: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        let balance_before = self.get_erc20_balance(state, token, account)?;

        if balance_before == balance {
            return Ok(None);
        }
        if balance_before < balance {
            match token_ty {
                ContractType::CurveYVault(base_token) => self
                    .__increase_curve_yvault_balance(
                        state,
                        token,
                        base_token,
                        account,
                        balance - balance_before,
                    )?,
                ContractType::AaveAToken(base_token) => self
                    .__increase_aave_atoken_v2_balance(
                        state,
                        token,
                        base_token,
                        account,
                        balance - balance_before,
                    )?,
                _ => {}
            }
        }

        let balance_after = self.get_erc20_balance(state, token, account)?;
        if balance_after < balance {
            Err(SoflError::Custom(format!(
                "{}: balance not enough",
                type_name::<Self>()
            )))
        } else {
            let mut caller = self.caller.clone();
            caller.address = account;

            let call = ERC20ABI::transferCall {
                to: ADDRESS_BOOK.dummy.fixed(),
                value: balance_after - balance,
            };
            let calldata = call.abi_encode();
            caller.call(state, token, calldata.cvt(), None, no_inspector())?;

            Ok(Some(balance_before))
        }
    }

    fn __increase_aave_atoken_v2_balance<S>(
        &mut self,
        state: &mut S,
        _token: Address,
        base_token: Address,
        account: Address,
        amount: U256,
    ) -> Result<(), SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        let mut caller = self.caller.clone();
        caller.address = account;

        let amount_in = (HPMultipler::from(amount) * U256::from(100)
            / U256::from(95))
        .into();

        self.increase_erc20_balance_by(state, base_token, account, amount_in)?;
        self.set_erc20_allowance(
            state,
            base_token,
            account,
            ADDRESS_BOOK
                .aave_lending_pool_v2
                .must_on_chain(Chain::Mainnet),
            U256::MAX,
        )?;

        let call = AaveLendingPoolV2ABI::depositCall {
            asset: base_token,
            amount: amount_in,
            onBehalfOf: account,
            referralCode: 0,
        };
        let calldata = call.abi_encode();
        caller.call(
            state,
            ADDRESS_BOOK
                .aave_lending_pool_v2
                .must_on_chain(Chain::Mainnet),
            calldata.cvt(),
            None,
            no_inspector(),
        )?;

        Ok(())
    }

    fn __increase_curve_yvault_balance<S>(
        &mut self,
        state: &mut S,
        token: Address,
        base_token: Address,
        account: Address,
        amount: U256,
    ) -> Result<(), SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        let mut caller = self.caller.clone();
        caller.address = account;

        let call = CurveYVaultABI::pricePerShareCall {};
        let calldata = call.abi_encode();
        let ret = self.cheat_read(state, token, calldata.cvt())?;
        let ret =
            CurveYVaultABI::pricePerShareCall::abi_decode_returns(&ret, true)
                .map_err(|e| SoflError::Abi(format!("{:?}", e)))?;
        let price = ret._0;

        let decimals = self.get_erc20_decimals(state, token)?;

        // we need to convert more amount to the base token
        // amount_in = amount * price / 10^decimals * 100 / 95
        let amount_in: U256 = (HPMultipler::from(amount) * price
            / U256::from(10).pow(decimals)
            * U256::from(100)
            / U256::from(95))
        .into();

        // check depositLimit
        let deposit_limit_func = CurveYVaultABI::depositLimitCall {};
        let ret = self.cheat_read(
            state,
            token,
            deposit_limit_func.abi_encode().cvt(),
        )?;
        let ret =
            CurveYVaultABI::depositLimitCall::abi_decode_returns(&ret, true)
                .map_err(|e| SoflError::Abi(format!("{:?}", e)))?;
        let deposit_limit = ret._0;

        // check totalasset
        let call = CurveYVaultABI::totalAssetsCall {};
        let ret = self.cheat_read(state, token, call.abi_encode().cvt())?;
        let ret =
            CurveYVaultABI::totalAssetsCall::abi_decode_returns(&ret, true)
                .map_err(|e| SoflError::Abi(format!("{:?}", e)))?;
        let total_assets = ret._0;

        if total_assets + amount_in > deposit_limit {
            // let's try to exceed the deposit limit
            self.cheat_write(
                state,
                token,
                deposit_limit_func.abi_encode().cvt(),
                (total_assets + amount_in) * U256::from(100) / U256::from(95),
            )?;
        }

        self.increase_erc20_balance_by(state, base_token, account, amount_in)?;
        self.set_erc20_allowance(state, base_token, account, token, U256::MAX)?;

        let func = self.parse_abi("deposit(uint256)")?;
        let calldata = func
            .abi_encode_input(&[amount_in.into()])
            .expect("encode failed");
        caller.call(state, token, calldata.cvt(), None, no_inspector())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests_with_dep {
    use libsofl_core::{
        blockchain::{provider::BcStateProvider, tx_position::TxPosition},
        conversion::ConvertTo,
        engine::types::U256,
    };

    use crate::{
        cheatcodes::CheatCodes, math::approx_eq, test::get_test_bc_provider,
    };

    #[test]
    fn test_set_depegged_token() {
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(14972419, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new()
            .set_caller(&|caller| caller.at_block(&bp, fork_at.block));

        let token = "0xE537B5cc158EB71037D4125BDD7538421981E6AA".cvt();
        let account = "0x166ed9f7A56053c7c4E77CB0C91a9E46bbC5e8b0".cvt();

        cheatcodes
            .set_erc20_balance(
                &mut state,
                token,
                account,
                U256::from(10).pow(U256::from(20)),
            )
            .unwrap();
        assert_eq!(
            cheatcodes
                .get_erc20_balance(&mut state, token, account)
                .unwrap(),
            U256::from(10).pow(U256::from(20))
        );

        // let's try to exceed the deposit limit
        cheatcodes
            .set_erc20_balance(
                &mut state,
                token,
                account,
                U256::from(10).pow(U256::from(24)),
            )
            .unwrap();
        assert_eq!(
            cheatcodes
                .get_erc20_balance(&mut state, token, account)
                .unwrap(),
            U256::from(10).pow(U256::from(24))
        );

        // let's try aave
        let token = "0xbcca60bb61934080951369a648fb03df4f96263c".cvt();
        cheatcodes
            .set_erc20_balance(
                &mut state,
                token,
                account,
                U256::from(10).pow(U256::from(13)),
            )
            .unwrap();
        // AAVE may have some precision loss
        assert!(approx_eq(
            cheatcodes
                .get_erc20_balance(&mut state, token, account)
                .unwrap(),
            U256::from(10).pow(U256::from(13)),
            None
        ));
    }
}
