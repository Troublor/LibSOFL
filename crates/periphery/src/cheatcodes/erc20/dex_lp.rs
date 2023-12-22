use std::{any::type_name, fmt::Debug};

use alloy_dyn_abi::{DynSolValue, JsonAbiExt};
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
    addressbook::{UniswapV2PairABI, ADDRESS_BOOK, ERC20ABI},
    cheatcodes::{contract_type::ContractType, CheatCodes},
    math::HPMultipler,
};

impl CheatCodes {
    pub fn set_lp_token_balance<S>(
        &mut self,
        state: &mut S,
        token_ty: ContractType,
        token: Address,
        account: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        let (pool, pool_ty) = token_ty.get_pool(token).ok_or_else(|| {
            SoflError::Custom(format!(
                "{}: try to set lp token balance to a non-lp token",
                type_name::<Self>()
            ))
        })?;

        if token == account || pool == account {
            return Err(SoflError::Custom(format!(
                "{}: try to set lp token balance to the lp token address or the pool address itself", type_name::<Self>()),
            ));
        }

        // first get the current balance of the lp token
        let current_balance = self.get_erc20_balance(state, token, account)?;
        if current_balance == balance {
            return Ok(None);
        }

        if current_balance < balance {
            self.__increase_lp_token_balance_by(
                state,
                pool_ty,
                pool,
                token,
                account,
                balance - current_balance,
            )?;
        } else {
            self.__decrease_lp_token_balance_by(
                state,
                pool_ty,
                pool,
                token,
                account,
                current_balance - balance,
            )?;
        }

        Ok(Some(current_balance))
    }
}

impl CheatCodes {
    fn __increase_lp_token_balance_by<S>(
        &mut self,
        state: &mut S,
        pool_ty: ContractType,
        pool: Address,
        token: Address,
        account: Address,
        amount: U256,
    ) -> Result<(), SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        let mut caller = self.caller.clone();
        caller.address = account;

        let amount_out = match pool_ty {
            ContractType::UniswapV2Pair(token0, token1) => {
                assert!(
                    pool == token,
                    "pool should be the same as token for Uniswap V2"
                );

                let token0_balance = self.get_erc20_balance(state, token0, pool)?;
                let token1_balance = self.get_erc20_balance(state, token1, pool)?;
                let total_supply = self.get_erc20_total_supply(state, token)?;

                // in case of rounding, we hope we can get more than less
                let token0_amount: U256 = (HPMultipler::from(token0_balance) * amount
                    / total_supply
                    * U256::from(100u64)
                    / U256::from(95u64))
                .into();
                let token1_amount: U256 = (HPMultipler::from(token1_balance) * amount
                    / total_supply
                    * U256::from(100u64)
                    / U256::from(95u64))
                .into();

                // set the balance of the pool upon token0/1
                self.set_erc20_balance(state, token0, pool, token0_balance + token0_amount)?;
                self.set_erc20_balance(state, token1, pool, token1_balance + token1_amount)?;

                // mint
                let call = UniswapV2PairABI::mintCall { to: account };
                let calldata = call.abi_encode();
                let ret = caller.call(state, pool, calldata.cvt(), None, no_inspector())?;
                let ret = UniswapV2PairABI::mintCall::abi_decode_returns(&ret, true)
                    .map_err(|e| SoflError::Abi(format!("failed to decoe return value: {}", e)))?;
                ret.liquidity
            }

            ContractType::CurveStableSwap(coins) | ContractType::CurveCryptoSwap(coins) => {
                let total_supply = self.get_erc20_total_supply(state, token)?;
                let mut coin_amounts = vec![U256::ZERO; coins.len()];

                for i in 0..coins.len() {
                    let coin = coins[i];
                    let balance = self.get_erc20_balance(state, coin, pool)?;

                    let coin_amount: U256 =
                        (HPMultipler::from(U256::from(1u64)) * balance * amount / total_supply
                            * U256::from(100)
                            / U256::from(95))
                        .into();
                    coin_amounts[i] = coin_amount;

                    let coin_balance = self.get_erc20_balance(state, coin, account)?;

                    self.set_erc20_balance(state, coin, account, coin_balance + coin_amount)?;

                    self.set_erc20_allowance(state, coin, account, pool, coin_amount)?;
                }

                let token_balance_before = self.get_erc20_balance(state, token, account)?;

                let func = self.parse_abi(
                    format!("add_liquidity(uint256[{}],uint256)", coins.len()).as_str(),
                )?;
                let coin_amounts = DynSolValue::FixedArray(
                    coin_amounts
                        .into_iter()
                        .map(|v| DynSolValue::Uint(v, 256))
                        .collect(),
                );
                let calldata = func
                    .abi_encode_input(&[coin_amounts, amount.into()])
                    .expect("failed to encode input");
                caller.call(state, pool, calldata.cvt(), None, no_inspector())?;

                let token_balance_after = self.get_erc20_balance(state, token, account)?;

                token_balance_after - token_balance_before
            }
            _ => {
                return Err(SoflError::Custom(format!(
                    "{}: increase lp token balance by not supported",
                    type_name::<Self>()
                )))
            }
        };

        if amount_out < amount {
            Err(SoflError::Custom(format!(
                "{}: increase lp token balance by not enough",
                type_name::<Self>()
            )))
        } else {
            // send out the additional lp to dummy
            let call = UniswapV2PairABI::transferCall {
                to: ADDRESS_BOOK.dummy.fixed(),
                value: amount_out - amount,
            };
            let calldata = call.abi_encode();
            caller.call(state, token, calldata.cvt(), None, no_inspector())?;

            Ok(())
        }
    }
}

impl CheatCodes {
    fn __decrease_lp_token_balance_by<S>(
        &mut self,
        state: &mut S,
        _pool_ty: ContractType,
        _pool: Address,
        token: Address,
        account: Address,
        amount: U256,
    ) -> Result<(), SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        // prepare a caller
        let mut caller = self.caller.clone();
        caller.address = account;

        let call = ERC20ABI::transferCall {
            to: ADDRESS_BOOK.dummy.fixed(),
            value: amount,
        };
        let calldata = call.abi_encode();
        caller.call(state, token, calldata.cvt(), None, no_inspector())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests_with_dep {
    use libsofl_core::{
        blockchain::tx_position::TxPosition,
        conversion::ConvertTo,
        engine::types::{Address, U256},
    };

    use crate::{cheatcodes::CheatCodes, test::get_test_bc_provider};

    #[test]
    fn test_increase_lp_token() {
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes =
            CheatCodes::default().set_caller(&|caller| caller.at_block(&bp, fork_at.block));

        {
            let token: Address = "0xBb2b8038a1640196FbE3e38816F3e67Cba72D940".cvt();
            let account: Address = "0x15077e6217E0253fF00917e0bb744047c74195FB".cvt();
            cheatcodes
                .set_erc20_balance(&mut state, token, account, U256::from(5000000000000u64))
                .unwrap();
            assert_eq!(
                cheatcodes
                    .get_erc20_balance(&mut state, token, account)
                    .unwrap(),
                U256::from(5000000000000u64)
            );
        }

        {
            let token = "0xFd2a8fA60Abd58Efe3EeE34dd494cD491dC14900".cvt();
            let account = "0xFCa7C5CF95821f3D45b9949De6E2846D66aF819F".cvt();
            let balance = cheatcodes
                .get_erc20_balance(&mut state, token, account)
                .unwrap();
            assert!(balance < U256::from(5000000000000000u64));

            cheatcodes
                .set_erc20_balance(&mut state, token, account, U256::from(5000000000000000u64))
                .unwrap();
            assert_eq!(
                cheatcodes
                    .get_erc20_balance(&mut state, token, account)
                    .unwrap(),
                U256::from(5000000000000000u64)
            );
        }

        {
            let token = "0xc4AD29ba4B3c580e6D59105FFf484999997675Ff".cvt();
            let account = "0x2A3Be5753E2dc6e602f494a8063404b578ec6941".cvt();
            cheatcodes
                .set_erc20_balance(&mut state, token, account, U256::from(5000000000000u64))
                .unwrap();
            assert_eq!(
                cheatcodes
                    .get_erc20_balance(&mut state, token, account)
                    .unwrap(),
                U256::from(5000000000000u64)
            );
        }
    }

    #[test]
    fn test_decrease_lp_token() {
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes =
            CheatCodes::default().set_caller(&|caller| caller.at_block(&bp, fork_at.block));

        {
            let token = "0xBb2b8038a1640196FbE3e38816F3e67Cba72D940".cvt();
            let account = "0xE8E5f5c4eB430C517C5F266eF9d18994321f1521".cvt();
            cheatcodes
                .set_erc20_balance(&mut state, token, account, U256::from(5000000000000u64))
                .unwrap();
            assert_eq!(
                cheatcodes
                    .get_erc20_balance(&mut state, token, account)
                    .unwrap(),
                U256::from(5000000000000u64)
            );
        }

        {
            let token = "0xFd2a8fA60Abd58Efe3EeE34dd494cD491dC14900".cvt();
            let account = "0xb6e81F0906498171779361Fb4Cc1AC58A1159fCD".cvt();
            let balance = cheatcodes
                .get_erc20_balance(&mut state, token, account)
                .unwrap();
            assert!(balance > U256::from(5000000000000000u64));

            cheatcodes
                .set_erc20_balance(&mut state, token, account, U256::from(5000000000000000u64))
                .unwrap();
            assert_eq!(
                cheatcodes
                    .get_erc20_balance(&mut state, token, account)
                    .unwrap(),
                U256::from(5000000000000000u64)
            );
        }

        {
            let token = "0xc4AD29ba4B3c580e6D59105FFf484999997675Ff".cvt();
            let account = "0x347140c7F001452e6A60131D24b37103D0e34231".cvt();
            let balance = cheatcodes
                .get_erc20_balance(&mut state, token, account)
                .unwrap();
            assert!(balance > U256::from(5000000000000u64));

            cheatcodes
                .set_erc20_balance(&mut state, token, account, U256::from(5000000000000u64))
                .unwrap();
            assert_eq!(
                cheatcodes
                    .get_erc20_balance(&mut state, token, account)
                    .unwrap(),
                U256::from(5000000000000u64)
            );
        }
    }
}
