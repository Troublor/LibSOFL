use std::{cmp::Ordering, fmt::Debug};

use ethers::abi::Token;
use reth_primitives::Address;
use revm::{Database, DatabaseCommit};
use revm_primitives::U256;

use crate::engine::state::DatabaseEditable;
use crate::error::SoflError;
use crate::unwrap_first_token_value;
use crate::utils::abi::{
    UNISWAP_V2_FACTORY_ABI, UNISWAP_V3_FACTORY_ABI, UNISWAP_V3_POOL_ABI,
};
use crate::utils::addresses::{
    DAI, UNISWAP_V2_FACTORY, UNISWAP_V3_FACTORY, USDC, USDT, WETH,
};
use crate::utils::math::HPMultipler;

use super::{CheatCodes, ERC20Cheat};

pub trait PriceOracleCheat<
    E,
    S: DatabaseEditable<Error = E> + Database<Error = E>,
>
{
    fn get_price_in_ether(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<E>>;
}

impl<E, S> PriceOracleCheat<E, S> for CheatCodes<S>
where
    E: Debug,
    S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
{
    fn get_price_in_ether(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<E>> {
        let (price_v2, liquidity_v2) = self.query_uniswap_v2(state, token)?;
        let (price_v3, liquidity_v3) = self.query_uniswap_v3(state, token)?;

        println!("price_v2: {}, liquidity_v2: {}", price_v2, liquidity_v2);
        println!("price_v3: {}, liquidity_v3: {}", price_v3, liquidity_v3);

        if liquidity_v3 > liquidity_v2 {
            Ok(price_v3)
        } else {
            Ok(price_v2)
        }
    }
}

// Uniswap V2
impl<E, S> CheatCodes<S>
where
    E: Debug,
    S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
{
    fn query_uniswap_v2(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<(U256, U256), SoflError<E>> {
        // check whether uniswap v3 exists
        let func = UNISWAP_V2_FACTORY_ABI.function("feeToSetter").expect(
            "bug: cannot find feeToSetter function in UniswapV2Factory ABI",
        );
        let _ = self.cheat_read(state, *UNISWAP_V2_FACTORY, func, &[])?;

        if token == *WETH {
            return Ok((U256::from(10).pow(U256::from(18)), U256::MAX));
        }

        let (best_pool, best_bs_token, best_liquidity) =
            self.__get_best_pool_uniswap_v2(state, token)?;

        let token_balance =
            self.__get_token_balance_uniswap_v2(state, token, best_pool)?;

        let bs_token_balance_in_pool1 =
            self.get_erc20_balance(state, best_bs_token, best_pool)?;

        // we need to update the decimals of the token to 18
        let mut price = HPMultipler::from(U256::from(10).pow(U256::from(18)));

        // (bs_token_balance1 / token_balance) * (weth_balance / bs_token_balance2) * (10^18)
        price *= bs_token_balance_in_pool1;
        price /= token_balance;

        if best_bs_token != *WETH {
            let bc_pool = self.__get_pair_address_uniswap_v2(
                state,
                best_bs_token,
                *WETH,
            )?;

            let bs_token_balance_in_pool2 =
                self.get_erc20_balance(state, best_bs_token, bc_pool)?;

            let weth_balance = self.get_erc20_balance(state, *WETH, bc_pool)?;

            let bs_price =
                HPMultipler::from(weth_balance) / bs_token_balance_in_pool2;

            price /= bs_price;
        }

        Ok((price.into(), best_liquidity))
    }

    fn __get_best_pool_uniswap_v2(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<(Address, Address, U256), SoflError<E>> {
        let mainstream_tokens = &[*WETH, *USDT, *USDC, *DAI];

        // iterate through all main stream tokens and fees
        let mut pool = Address::default();
        let mut bs_token = Address::default();
        let mut liquidity = U256::ZERO;

        // a shortcut for mainstream tokens
        if mainstream_tokens.contains(&token) {
            // this cannot be WETH
            pool = self.__get_pair_address_uniswap_v2(state, token, *WETH)?;
            bs_token = *WETH;
            liquidity = self.get_erc20_balance(state, token, pool)?;
        } else {
            for ms_token in mainstream_tokens.iter() {
                let cur_pool = self
                    .__get_pair_address_uniswap_v2(state, token, *ms_token)?;

                if cur_pool == Address::from(0) {
                    continue;
                }

                if let Ok(token_liquidity) =
                    self.get_erc20_balance(state, token, cur_pool)
                {
                    if token_liquidity > liquidity {
                        liquidity = token_liquidity;
                        pool = cur_pool;
                        bs_token = *ms_token;
                    }
                }
            }
        }

        // if no pool found, return error
        if pool == Address::default() {
            Err(SoflError::Custom(
                "No pool found for uniswap v3".to_string(),
            ))
        } else {
            Ok((pool, bs_token, liquidity))
        }
    }

    fn __get_token_balance_uniswap_v2(
        &mut self,
        state: &mut S,
        token: Address,
        pool: Address,
    ) -> Result<U256, SoflError<E>> {
        let token_decimals = self.get_erc20_decimals(state, token)?;
        let raw_balance = self.get_erc20_balance(state, token, pool)?;

        let token_balance = match token_decimals.cmp(&U256::from(18)) {
            Ordering::Less => {
                raw_balance
                    * U256::from(10).pow(
                        U256::from(18)
                            .checked_sub(U256::from(token_decimals))
                            .unwrap(),
                    )
            }
            Ordering::Equal => raw_balance,
            Ordering::Greater => {
                raw_balance
                    / U256::from(10).pow(
                        U256::from(token_decimals)
                            .checked_sub(U256::from(18))
                            .unwrap(),
                    )
            }
        };

        Ok(token_balance)
    }

    fn __get_pair_address_uniswap_v2(
        &mut self,
        state: &mut S,
        token1: Address,
        token2: Address,
    ) -> Result<Address, SoflError<E>> {
        let func = UNISWAP_V2_FACTORY_ABI.function("getPair").expect(
            "bug: cannot find getPair function in UniswapV2Factory ABI",
        );
        let mut token_pair = self.cheat_read(
            state,
            *UNISWAP_V2_FACTORY,
            func,
            &[Token::Address(token1.into()), Token::Address(token2.into())],
        )?;

        Ok(unwrap_first_token_value!(Address, token_pair))
    }
}

// Uniswap v3
impl<E, S> CheatCodes<S>
where
    E: Debug,
    S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
{
    fn query_uniswap_v3(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<(U256, U256), SoflError<E>> {
        // check whether uniswap v3 exists
        {
            let func = UNISWAP_V3_FACTORY_ABI.function("owner").expect(
                "bug: cannot find owner function in UniswapV3Factory ABI",
            );
            let _ = self.cheat_read(state, *UNISWAP_V3_FACTORY, func, &[])?;
        }

        if token == *WETH {
            return Ok((U256::from(10).pow(U256::from(18)), U256::MAX));
        }

        let (best_pool, best_bs_token, best_liquidity) = self
            .__get_best_pool_uniswap_v3(
                state,
                token,
                &[*WETH, *USDT, *USDC, *DAI],
            )?;

        let mut price = self.__get_token_price_uniswap_v3(
            state,
            token,
            best_bs_token,
            best_pool,
        )?;
        if best_bs_token != *WETH {
            let (best_bs_pool, _, _) = self.__get_best_pool_uniswap_v3(
                state,
                best_bs_token,
                &[*WETH],
            )?;

            let bs_price = self.__get_token_price_uniswap_v3(
                state,
                best_bs_pool,
                *WETH,
                best_bs_pool,
            )?;

            price /= bs_price;
        }

        price *= U256::from(10).pow(U256::from(18));
        Ok((price.into(), best_liquidity))
    }

    fn __get_token_price_uniswap_v3(
        &mut self,
        state: &mut S,
        token: Address,
        bs_token: Address,
        pool: Address,
    ) -> Result<HPMultipler, SoflError<E>> {
        let func = UNISWAP_V3_POOL_ABI
            .function("slot0")
            .expect("bug: cannot find slot0 function in UniswapV3Pool ABI");

        // price is Q64.96
        let mut res = self.cheat_read(state, pool, func, &[])?;
        let sqrt_price_x96 = unwrap_first_token_value!(Uint, res);

        let mut result = HPMultipler::from(sqrt_price_x96);

        // convert from Q64.96 to U256
        result /= HPMultipler::from(U256::from(2).pow(U256::from(96)));

        // if token is smaller than base token, divide by result
        if token > bs_token {
            result = result.reciprocal();
        }

        // square the result
        result = result.pow(2);

        // consider the decimal
        let token_decimals = self.get_erc20_decimals(state, token)?;
        let bs_token_decimals = self.get_erc20_decimals(state, bs_token)?;
        result = match token_decimals.cmp(&bs_token_decimals) {
            Ordering::Less => {
                result / U256::from(10).pow(bs_token_decimals - token_decimals)
            }
            Ordering::Equal => result,
            Ordering::Greater => {
                result * U256::from(10).pow(token_decimals - bs_token_decimals)
            }
        };

        Ok(result)
    }

    fn __get_best_pool_uniswap_v3(
        &mut self,
        state: &mut S,
        token: Address,
        baseline_tokens: &[Address],
    ) -> Result<(Address, Address, U256), SoflError<E>> {
        let fees = &[500u64, 3000u64, 10000u64];

        // iterate through all main stream tokens and fees
        let mut pool = Address::default();
        let mut bs_token = Address::default();
        let mut liquidity = U256::ZERO;

        // a shortcut for mainstream tokens
        if baseline_tokens.contains(&token) {
            // this cannot be WETH
            pool =
                self.__get_pool_address_uniswap_v3(state, token, *WETH, 500)?;
            bs_token = *WETH;
            liquidity = self.get_erc20_balance(state, token, pool)?;
        } else {
            for ms_token in baseline_tokens.iter() {
                for fee in fees.iter() {
                    let cur_pool = self.__get_pool_address_uniswap_v3(
                        state, token, *ms_token, *fee,
                    )?;

                    if cur_pool == Address::from(0) {
                        continue;
                    }

                    if let Ok(token_liquidity) =
                        self.get_erc20_balance(state, token, cur_pool)
                    {
                        if token_liquidity > liquidity {
                            liquidity = token_liquidity;
                            pool = cur_pool;
                            bs_token = *ms_token;
                        }
                    }
                }
            }
        }

        // if no pool found, return error
        if pool == Address::default() {
            Err(SoflError::Custom(
                "No pool found for uniswap v3".to_string(),
            ))
        } else {
            Ok((pool, bs_token, liquidity))
        }
    }

    fn __get_pool_address_uniswap_v3(
        &mut self,
        state: &mut S,
        token1: Address,
        token2: Address,
        fee: u64,
    ) -> Result<Address, SoflError<E>> {
        let func = UNISWAP_V3_FACTORY_ABI.function("getPool").expect(
            "bug: cannot find getPool function in UniswapV3Factory ABI",
        );
        let mut token_pair = self.cheat_read(
            state,
            *UNISWAP_V3_FACTORY,
            func,
            &[
                Token::Address(token1.into()),
                Token::Address(token2.into()),
                Token::Uint(fee.into()),
            ],
        )?;

        Ok(unwrap_first_token_value!(Address, token_pair))
    }
}
