use std::any::type_name;
use std::{cmp::Ordering, fmt::Debug};

use alloy_sol_types::SolCall;
use libsofl_core::conversion::ConvertTo;
use libsofl_core::engine::state::BcState;
use libsofl_core::engine::types::{Address, U256};
use libsofl_core::error::SoflError;
use libsofl_utils::log::trace;

use crate::addressbook::{
    UniswapV2FactoryABI, UniswapV3FactoryABI, UniswapV3PoolABI, ADDRESS_BOOK,
};
use crate::math::HPMultipler;
use crate::types::Chain;

use super::CheatCodes;

impl CheatCodes {
    pub fn get_price_in_ether<S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        let mut price = U256::ZERO;
        let mut liquidity = U256::ZERO;

        if let Ok((price_v2, liquidity_v2)) =
            self.query_uniswap_v2(state, token)
        {
            trace!("price_v2: {}, liquidity_v2: {}", price_v2, liquidity_v2);
            if liquidity_v2 > liquidity {
                price = price_v2;
                liquidity = liquidity_v2;
            }
        }

        if let Ok((price_v3, liquidity_v3)) =
            self.query_uniswap_v3(state, token)
        {
            trace!("price_v3: {}, liquidity_v3: {}", price_v3, liquidity_v3);
            if liquidity_v3 > liquidity {
                price = price_v3;
                liquidity = liquidity_v3;
            }
        }

        if liquidity == U256::ZERO {
            Err(SoflError::Custom(format!(
                "{}: no liquidity found",
                type_name::<Self>()
            )))
        } else {
            Ok(price)
        }
    }
}

// Uniswap V2
impl CheatCodes {
    fn query_uniswap_v2<S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<(U256, U256), SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        // check whether uniswap v3 exists
        let call = UniswapV2FactoryABI::feeToSetterCall {};
        let _ = self.cheat_read(
            state,
            ADDRESS_BOOK
                .uniswap_v2_factory
                .must_on_chain(Chain::Mainnet),
            call.abi_encode().cvt(),
        )?;

        let weth = ADDRESS_BOOK.weth.must_on_chain(Chain::Mainnet);
        if token == weth {
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

        if best_bs_token != weth {
            let bc_pool =
                self.__get_pair_address_uniswap_v2(state, best_bs_token, weth)?;

            let bs_token_balance_in_pool2 =
                self.get_erc20_balance(state, best_bs_token, bc_pool)?;

            let weth_balance = self.get_erc20_balance(state, weth, bc_pool)?;

            let bs_price =
                HPMultipler::from(weth_balance) / bs_token_balance_in_pool2;

            price /= bs_price;
        }

        Ok((price.into(), best_liquidity))
    }

    fn __get_best_pool_uniswap_v2<S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<(Address, Address, U256), SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        let weth = ADDRESS_BOOK.weth.must_on_chain(Chain::Mainnet);
        let usdt = ADDRESS_BOOK.usdt.must_on_chain(Chain::Mainnet);
        let usdc = ADDRESS_BOOK.usdc.must_on_chain(Chain::Mainnet);
        let dai = ADDRESS_BOOK.dai.must_on_chain(Chain::Mainnet);
        let mainstream_tokens = &[weth, usdt, usdc, dai];

        // iterate through all main stream tokens and fees
        let mut pool = Address::default();
        let mut bs_token = Address::default();
        let mut liquidity = U256::ZERO;

        // a shortcut for mainstream tokens
        if mainstream_tokens.contains(&token) {
            // this cannot be WETH
            pool = self.__get_pair_address_uniswap_v2(state, token, weth)?;
            bs_token = weth;
            liquidity = self.get_erc20_balance(state, token, pool)?;
        } else {
            for ms_token in mainstream_tokens.iter() {
                let cur_pool = self
                    .__get_pair_address_uniswap_v2(state, token, *ms_token)?;

                if cur_pool == Address::ZERO {
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
            Err(SoflError::Custom(format!(
                "{}: no pool found for uniswap v3",
                type_name::<Self>()
            )))
        } else {
            Ok((pool, bs_token, liquidity))
        }
    }

    fn __get_token_balance_uniswap_v2<S>(
        &mut self,
        state: &mut S,
        token: Address,
        pool: Address,
    ) -> Result<U256, SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
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

    fn __get_pair_address_uniswap_v2<S>(
        &mut self,
        state: &mut S,
        token1: Address,
        token2: Address,
    ) -> Result<Address, SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        let call = UniswapV2FactoryABI::getPairCall {
            _0: token1,
            _1: token2,
        };
        let ret = self.cheat_read(
            state,
            ADDRESS_BOOK
                .uniswap_v2_factory
                .must_on_chain(Chain::Mainnet),
            call.abi_encode().cvt(),
        )?;
        let ret =
            UniswapV2FactoryABI::getPairCall::abi_decode_returns(&ret, true)
                .expect("bug: cannot decode getPairCall returns");
        Ok(ret._0)
    }
}

// Uniswap v3
impl CheatCodes {
    fn query_uniswap_v3<S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<(U256, U256), SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        // check whether uniswap v3 exists
        {
            let call = UniswapV3FactoryABI::ownerCall {};
            let _ = self.cheat_read(
                state,
                ADDRESS_BOOK
                    .uniswap_v3_factory
                    .must_on_chain(Chain::Mainnet),
                call.abi_encode().cvt(),
            )?;
        }

        let weth = ADDRESS_BOOK.weth.must_on_chain(Chain::Mainnet);
        let usdt = ADDRESS_BOOK.usdt.must_on_chain(Chain::Mainnet);
        let usdc = ADDRESS_BOOK.usdc.must_on_chain(Chain::Mainnet);
        let dai = ADDRESS_BOOK.dai.must_on_chain(Chain::Mainnet);
        if token == weth {
            return Ok((U256::from(10).pow(U256::from(18)), U256::MAX));
        }

        let (best_pool, best_bs_token, best_liquidity) = self
            .__get_best_pool_uniswap_v3(
                state,
                token,
                &[weth, usdt, usdc, dai],
            )?;

        let mut price = self.__get_token_price_uniswap_v3(
            state,
            token,
            best_bs_token,
            best_pool,
        )?;

        if best_bs_token != weth {
            let (best_bs_pool, _, _) =
                self.__get_best_pool_uniswap_v3(state, best_bs_token, &[weth])?;

            let bs_price = self.__get_token_price_uniswap_v3(
                state,
                best_bs_token,
                weth,
                best_bs_pool,
            )?;

            price *= bs_price;
        }

        price *= U256::from(10).pow(U256::from(18));
        Ok((price.into(), best_liquidity))
    }

    fn __get_token_price_uniswap_v3<S>(
        &mut self,
        state: &mut S,
        token: Address,
        bs_token: Address,
        pool: Address,
    ) -> Result<HPMultipler, SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        let call = UniswapV3PoolABI::slot0Call {};
        // price is Q64.96
        let ret = self.cheat_read(state, pool, call.abi_encode().cvt())?;
        let sqrt_price_x96 =
            UniswapV3PoolABI::slot0Call::abi_decode_returns(&ret, true)
                .expect("bug: cannot decode slot0Call returns")
                .sqrtPriceX96;

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

    fn __get_best_pool_uniswap_v3<S>(
        &mut self,
        state: &mut S,
        token: Address,
        baseline_tokens: &[Address],
    ) -> Result<(Address, Address, U256), SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        let fees = &[500u64, 3000u64, 10000u64];

        // iterate through all main stream tokens and fees
        let mut pool = Address::default();
        let mut bs_token = Address::default();
        let mut liquidity = U256::ZERO;

        let weth = ADDRESS_BOOK.weth.must_on_chain(Chain::Mainnet);
        // a shortcut for mainstream tokens
        if baseline_tokens.contains(&token) {
            // this cannot be WETH
            pool =
                self.__get_pool_address_uniswap_v3(state, token, weth, 500)?;
            bs_token = weth;
            liquidity = self.get_erc20_balance(state, token, pool)?;
        } else {
            for ms_token in baseline_tokens.iter() {
                for fee in fees.iter() {
                    let cur_pool = self.__get_pool_address_uniswap_v3(
                        state, token, *ms_token, *fee,
                    )?;

                    if cur_pool == Address::ZERO {
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
            Err(SoflError::Custom(format!(
                "{}: no pool found for uniswap v3",
                type_name::<Self>(),
            )))
        } else {
            Ok((pool, bs_token, liquidity))
        }
    }

    fn __get_pool_address_uniswap_v3<S>(
        &mut self,
        state: &mut S,
        token1: Address,
        token2: Address,
        fee: u64,
    ) -> Result<Address, SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        let call = UniswapV3FactoryABI::getPoolCall {
            _0: token1,
            _1: token2,
            _2: fee as u32,
        };
        let ret = self.cheat_read(
            state,
            ADDRESS_BOOK
                .uniswap_v3_factory
                .must_on_chain(Chain::Mainnet),
            call.abi_encode().cvt(),
        )?;
        let ret =
            UniswapV3FactoryABI::getPoolCall::abi_decode_returns(&ret, true)
                .expect("bug: cannot decode getPoolCall returns");
        Ok(ret._0)
    }
}

#[cfg(test)]
mod tests_with_dep {
    use libsofl_core::{
        blockchain::{provider::BcStateProvider, tx_position::TxPosition},
        conversion::ConvertTo,
        engine::types::{Address, U256},
    };

    use crate::{cheatcodes::CheatCodes, test::get_test_bc_provider};

    #[test]
    fn test_price_oracle_weth() {
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let weth = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".cvt();
        let price = cheatcodes.get_price_in_ether(&mut state, weth).unwrap();

        assert!(price == U256::from(10).pow(U256::from(18)));
    }

    #[test]
    fn test_price_oracle_wbtc() {
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let wbtc = "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599".cvt();
        let price = cheatcodes.get_price_in_ether(&mut state, wbtc).unwrap();

        // BTC should be at least 5 ETH
        assert!(price > U256::from(5) * U256::from(10).pow(U256::from(18)));
    }

    #[test]
    fn test_price_oracle_stablecoins() {
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let usdc: Address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".cvt();
        let price0 = cheatcodes.get_price_in_ether(&mut state, usdc).unwrap();

        let dai: Address = "0x6b175474e89094c44da98b954eedeac495271d0f".cvt();
        let price1 = cheatcodes.get_price_in_ether(&mut state, dai).unwrap();

        let usdt: Address = "0xdac17f958d2ee523a2206206994597c13d831ec7".cvt();
        let price2 = cheatcodes.get_price_in_ether(&mut state, usdt).unwrap();

        let busd: Address = "0x4fabb145d64652a948d72533023f6e7a623c7c53".cvt();
        let price3 = cheatcodes.get_price_in_ether(&mut state, busd).unwrap();

        let mut prices = [price0, price1, price2, price3];
        prices.sort();

        let delta = prices[3] - prices[0];

        // delta / min_price < 0.01
        assert!(delta * U256::from(100) < prices[0]);
    }
}
