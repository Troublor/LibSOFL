use revm::{Database, DatabaseCommit};
use revm_primitives::{Address, U256};
use std::{any::type_name, fmt::Debug};

use crate::{
    engine::{
        cheatcodes::{CheatCodes, ContractType},
        inspectors::no_inspector,
        state::DatabaseEditable,
    },
    error::SoflError,
    unwrap_first_token_value,
    utils::{
        abi::{CURVE_Y_VAULT_ABI, ERC20_ABI},
        addresses::BURNER_ADDRESS,
        conversion::{Convert, ToEthers},
        math::HPMultipler,
    },
};

impl CheatCodes {
    pub fn set_pegged_token_balance<E, S>(
        &mut self,
        state: &mut S,
        token_ty: ContractType,
        token: Address,
        account: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
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
                ContractType::AaveAToken(_) => todo!(),
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

            let func = ERC20_ABI
                .function("transfer")
                .expect("transfer function not found");

            caller.invoke_ignore_return(
                state,
                token,
                func,
                &[
                    ToEthers::cvt(*BURNER_ADDRESS),
                    ToEthers::cvt(balance_after - balance),
                ],
                None,
                no_inspector(),
            )?;

            Ok(Some(balance_before))
        }
    }

    fn __increase_curve_yvault_balance<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
        base_token: Address,
        account: Address,
        amount: U256,
    ) -> Result<(), SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        let mut caller = self.caller.clone();
        caller.address = account;

        let func = CURVE_Y_VAULT_ABI
            .function("pricePerShare")
            .expect("pricePerShare not found");

        let price = unwrap_first_token_value!(
            Uint,
            self.cheat_read(state, token, func, &[])?
        );

        let decimals = self.get_erc20_decimals(state, token)?;

        // we need to convert more amount to the base token
        // amount_in = amount * price / 10^decimals * 100 / 95
        let amount_in: U256 = (HPMultipler::from(amount) * price
            / U256::from(10).pow(decimals)
            * U256::from(100)
            / U256::from(95))
        .into();

        // check depositLimit
        let deposit_limit_func = CURVE_Y_VAULT_ABI
            .function("depositLimit")
            .expect("depositLimit function not found");
        let deposit_limit = unwrap_first_token_value!(
            Uint,
            self.cheat_read(state, token, deposit_limit_func, &[])?
        );

        // check totalasset
        let func = CURVE_Y_VAULT_ABI
            .function("totalAssets")
            .expect("totalAssets function not found");
        let total_assets = unwrap_first_token_value!(
            Uint,
            self.cheat_read(state, token, func, &[])?
        );

        if total_assets + amount_in > deposit_limit {
            // let's try to exceed the deposit limit
            self.cheat_write(
                state,
                token,
                deposit_limit_func,
                &[],
                (total_assets + amount_in) * U256::from(100) / U256::from(95),
            )?;
        }

        self.increase_erc20_balance_by(state, base_token, account, amount_in)?;
        self.set_erc20_allowance(state, base_token, account, token, U256::MAX)?;

        let func = self.parse_abi("deposit(uint256)".to_string())?;
        caller.invoke_ignore_return(
            state,
            token,
            func,
            &[ToEthers::cvt(amount_in)],
            None,
            no_inspector(),
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests_with_dep {
    use revm_primitives::U256;

    use crate::{
        engine::{
            cheatcodes::CheatCodes, state::BcStateBuilder,
            transactions::position::TxPosition,
        },
        utils::{
            conversion::{Convert, ToPrimitive},
            testing::get_testing_bc_provider,
        },
    };

    #[test]
    fn test_set_depegged_token() {
        let bp = get_testing_bc_provider();

        let fork_at = TxPosition::new(14972419, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new()
            .set_caller(&|caller| caller.at_block(&bp, fork_at.block));

        let token =
            ToPrimitive::cvt("0xE537B5cc158EB71037D4125BDD7538421981E6AA");
        let account =
            ToPrimitive::cvt("0x166ed9f7A56053c7c4E77CB0C91a9E46bbC5e8b0");

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
    }
}
