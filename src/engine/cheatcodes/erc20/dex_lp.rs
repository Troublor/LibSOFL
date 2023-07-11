use std::fmt::Debug;

use revm::{Database, DatabaseCommit};
use revm_primitives::{Address, U256};

use crate::{
    engine::{
        cheatcodes::{CheatCodes, ContractType},
        inspectors::no_inspector,
        state::DatabaseEditable,
    },
    error::SoflError,
    unwrap_first_token_value,
    utils::{
        abi::UNISWAP_V2_PAIR_ABI,
        addresses::DUMMY_ADDRESS,
        conversion::{Convert, ToEthers},
        math::HPMultipler,
    },
};

impl CheatCodes {
    pub fn set_lp_token_balance<E, S>(
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
        let (pool, pool_ty) = token_ty.get_pool(token).ok_or_else(|| {
            SoflError::Custom(
                "try to set lp token balance to a non-lp token".to_string(),
            )
        })?;

        if token == account || pool == account {
            return Err(SoflError::Custom(
                "try to set lp token balance to the lp token address or the pool address itself"
                    .to_string(),
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
    fn __increase_lp_token_balance_by<E, S>(
        &mut self,
        state: &mut S,
        pool_ty: ContractType,
        pool: Address,
        token: Address,
        account: Address,
        amount: U256,
    ) -> Result<(), SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        let mut caller = self.caller.clone();
        caller.address = account;

        let amount_out = match pool_ty {
            ContractType::UniswapV2Pair(token0, token1) => {
                assert!(
                    pool == token,
                    "pool should be the same as token for Uniswap V2"
                );

                let token0_balance =
                    self.get_erc20_balance(state, token0, pool)?;
                let token1_balance =
                    self.get_erc20_balance(state, token1, pool)?;
                let total_supply = self.get_erc20_total_supply(state, token)?;

                // in case of rounding, we hope we can get more than less
                let token0_amount: U256 =
                    (HPMultipler::from(token0_balance) * amount / total_supply
                        * U256::from(100u64)
                        / U256::from(95u64))
                    .into();
                let token1_amount: U256 =
                    (HPMultipler::from(token1_balance) * amount / total_supply
                        * U256::from(100u64)
                        / U256::from(95u64))
                    .into();

                // set the balance of the pool upon token0/1
                self.set_erc20_balance(
                    state,
                    token0,
                    pool,
                    token0_balance + token0_amount,
                )?;
                self.set_erc20_balance(
                    state,
                    token1,
                    pool,
                    token1_balance + token1_amount,
                )?;

                // mint
                let func = UNISWAP_V2_PAIR_ABI
                    .function("mint")
                    .expect("mint function must exist");
                unwrap_first_token_value!(
                    Uint,
                    caller.invoke(
                        state,
                        pool,
                        func,
                        &[ToEthers::cvt(account)],
                        None,
                        no_inspector(),
                    )?
                )
            }

            ContractType::CurveStableSwap(coins)
            | ContractType::CurveCryptoSwap(coins) => {
                let total_supply = self.get_erc20_total_supply(state, token)?;
                let mut coin_amounts = vec![U256::ZERO; coins.len()];

                for i in 0..coins.len() {
                    let coin = coins[i];
                    let balance = self.get_erc20_balance(state, coin, pool)?;

                    let coin_amount: U256 =
                        (HPMultipler::from(U256::from(1u64))
                            * balance
                            * amount
                            / total_supply
                            * U256::from(100)
                            / U256::from(95))
                        .into();
                    coin_amounts[i] = coin_amount;

                    let coin_balance =
                        self.get_erc20_balance(state, coin, account)?;

                    self.set_erc20_balance(
                        state,
                        coin,
                        account,
                        coin_balance + coin_amount,
                    )?;

                    self.set_erc20_allowance(
                        state,
                        coin,
                        account,
                        pool,
                        coin_amount,
                    )?;
                }

                let token_balance_before =
                    self.get_erc20_balance(state, token, account)?;

                let func = self.parse_abi(format!(
                    "add_liquidity(uint256[{}],uint256)",
                    coins.len()
                ))?;

                caller.invoke_ignore_return(
                    state,
                    pool,
                    func,
                    &[ToEthers::cvt(coin_amounts), ToEthers::cvt(amount)],
                    None,
                    no_inspector(),
                )?;

                let token_balance_after =
                    self.get_erc20_balance(state, token, account)?;

                token_balance_after - token_balance_before
            }
            _ => {
                return Err(SoflError::Custom(
                    "increase lp token balance by not supported".to_string(),
                ))
            }
        };

        if amount_out < amount {
            Err(SoflError::Custom(
                "increase lp token balance by not enough".to_string(),
            ))
        } else {
            // send out the additional lp to somehere
            let func = UNISWAP_V2_PAIR_ABI
                .function("transfer")
                .expect("transfer function must exist");
            caller.invoke(
                state,
                token,
                func,
                &[
                    ToEthers::cvt(*DUMMY_ADDRESS),
                    ToEthers::cvt(amount_out - amount),
                ],
                None,
                no_inspector(),
            )?;

            Ok(())
        }
    }
}

impl CheatCodes {
    fn __decrease_lp_token_balance_by<E, S>(
        &mut self,
        state: &mut S,
        pool_ty: ContractType,
        pool: Address,
        token: Address,
        account: Address,
        amount: U256,
    ) -> Result<(), SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // prepare a caller
        let mut caller = self.caller.clone();
        caller.address = account;

        match pool_ty {
            ContractType::UniswapV2Pair(_, _) => {
                assert!(
                    pool == token,
                    "pool should be the same as token for Uniswap V2"
                );

                // first transfer to the pair
                let func = UNISWAP_V2_PAIR_ABI
                    .function("transfer")
                    .expect("transfer function should exist");
                caller.invoke_ignore_return(
                    state,
                    pool,
                    func,
                    &[ToEthers::cvt(token), ToEthers::cvt(amount)],
                    None,
                    no_inspector(),
                )?;

                // burn the lp token
                let func = UNISWAP_V2_PAIR_ABI
                    .function("burn")
                    .expect("burn function should exist");
                caller.invoke_ignore_return(
                    state,
                    pool,
                    func,
                    &[ToEthers::cvt(account)],
                    None,
                    no_inspector(),
                )?;
            }
            ContractType::CurveStableSwap(coins)
            | ContractType::CurveCryptoSwap(coins) => {
                // CurveStableSwap and CurveCryptoSwap are similar
                let func = self.parse_abi(format!(
                    "remove_liquidity(uint256,uint256[{}])",
                    coins.len()
                ))?;

                caller.invoke_ignore_return(
                    state,
                    pool,
                    func,
                    &[
                        ToEthers::cvt(amount),
                        ToEthers::cvt(vec![U256::ZERO, U256::ZERO, U256::ZERO]),
                    ],
                    None,
                    no_inspector(),
                )?;
            }
            _ => {
                return Err(SoflError::Custom(
                    "not a ERC20-backed dex".to_string(),
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests_with_dep {
    use revm_primitives::U256;

    use crate::engine::cheatcodes::CheatCodes;
    use crate::engine::state::BcStateBuilder;
    use crate::engine::transactions::position::TxPosition;
    use crate::utils::conversion::{Convert, ToPrimitive};
    use crate::utils::testing::get_testing_bc_provider;

    #[test]
    fn test_increase_lp_token() {
        let bp = get_testing_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::default()
            .set_caller(&|caller| caller.at_block(&bp, fork_at.block));

        {
            let token =
                ToPrimitive::cvt("0xBb2b8038a1640196FbE3e38816F3e67Cba72D940");
            let account =
                ToPrimitive::cvt("0x15077e6217E0253fF00917e0bb744047c74195FB");
            cheatcodes
                .set_erc20_balance(
                    &mut state,
                    token,
                    account,
                    U256::from(5000000000000u64),
                )
                .unwrap();
            assert_eq!(
                cheatcodes
                    .get_erc20_balance(&mut state, token, account)
                    .unwrap(),
                U256::from(5000000000000u64)
            );
        }

        if false {
            let token =
                ToPrimitive::cvt("0xFd2a8fA60Abd58Efe3EeE34dd494cD491dC14900");
            let account =
                ToPrimitive::cvt("0xFCa7C5CF95821f3D45b9949De6E2846D66aF819F");
            let balance = cheatcodes
                .get_erc20_balance(&mut state, token, account)
                .unwrap();
            assert!(balance < U256::from(5000000000000000u64));

            cheatcodes
                .set_erc20_balance(
                    &mut state,
                    token,
                    account,
                    U256::from(5000000000000000u64),
                )
                .unwrap();
            assert_eq!(
                cheatcodes
                    .get_erc20_balance(&mut state, token, account)
                    .unwrap(),
                U256::from(5000000000000000u64)
            );
        }

        {
            let token =
                ToPrimitive::cvt("0xc4AD29ba4B3c580e6D59105FFf484999997675Ff");
            let account =
                ToPrimitive::cvt("0x2A3Be5753E2dc6e602f494a8063404b578ec6941");
            cheatcodes
                .set_erc20_balance(
                    &mut state,
                    token,
                    account,
                    U256::from(5000000000000u64),
                )
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
        let bp = get_testing_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::default()
            .set_caller(&|caller| caller.at_block(&bp, fork_at.block));

        {
            let token =
                ToPrimitive::cvt("0xBb2b8038a1640196FbE3e38816F3e67Cba72D940");
            let account =
                ToPrimitive::cvt("0xE8E5f5c4eB430C517C5F266eF9d18994321f1521");
            cheatcodes
                .set_erc20_balance(
                    &mut state,
                    token,
                    account,
                    U256::from(5000000000000u64),
                )
                .unwrap();
            assert_eq!(
                cheatcodes
                    .get_erc20_balance(&mut state, token, account)
                    .unwrap(),
                U256::from(5000000000000u64)
            );
        }

        {
            let token =
                ToPrimitive::cvt("0xFd2a8fA60Abd58Efe3EeE34dd494cD491dC14900");
            let account =
                ToPrimitive::cvt("0xb6e81F0906498171779361Fb4Cc1AC58A1159fCD");
            let balance = cheatcodes
                .get_erc20_balance(&mut state, token, account)
                .unwrap();
            assert!(balance > U256::from(5000000000000000u64));

            cheatcodes
                .set_erc20_balance(
                    &mut state,
                    token,
                    account,
                    U256::from(5000000000000000u64),
                )
                .unwrap();
            assert_eq!(
                cheatcodes
                    .get_erc20_balance(&mut state, token, account)
                    .unwrap(),
                U256::from(5000000000000000u64)
            );
        }

        {
            let token =
                ToPrimitive::cvt("0xc4AD29ba4B3c580e6D59105FFf484999997675Ff");
            let account =
                ToPrimitive::cvt("0x347140c7F001452e6A60131D24b37103D0e34231");
            let balance = cheatcodes
                .get_erc20_balance(&mut state, token, account)
                .unwrap();
            assert!(balance > U256::from(5000000000000u64));

            cheatcodes
                .set_erc20_balance(
                    &mut state,
                    token,
                    account,
                    U256::from(5000000000000u64),
                )
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
