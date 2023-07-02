use std::{cmp::Ordering, fmt::Debug, str::FromStr};

use ethers::abi::Token;
use reth_primitives::Address;
use revm::{Database, DatabaseCommit};
use revm_primitives::U256;

use crate::{
    engine::state::DatabaseEditable,
    error::SoflError,
    utils::{
        abi::{UNISWAP_V2_FACTORY_ABI, UNISWAP_V3_FACTORY_ABI},
        conversion::{Convert, ToPrimitive},
    },
};

use super::{CheatCodes, ERC20Cheat};

lazy_static! {
    static ref WETH: Address = Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();

    static ref UNISWAP_V3_FACTORY: Address =
        Address::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984")
            .unwrap();
    static ref UNISWAP_V3_FEES: Vec<u64> = vec![500, 3000, 10000];

    static ref UNISWAP_V2_FACTORY: Address =
        Address::from_str("0x5c69bee701ef814a2b6a3edd4b1652cb9cc5aa6f")
            .unwrap();
    static ref UNISWAP_V2_FEES: Vec<u64> = vec![500, 3000, 10000];

    static ref MAINSTREAM_TOKENS: Vec<Address> =
        vec![
            // WETH
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
            // USDC
            Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
            // USDT
            Address::from_str("0xdAC17F958D2ee523a2206206994597C13D831ec7").unwrap(),
            // DAI
            Address::from_str("0x6B175474E89094C44Da98b954EedeAC495271d0F").unwrap(),
        ];
}

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
        let (price, _liquidity) = self.query_uniswap_v2(state, token)?;
        Ok(price)
    }
}

// return price in ether (we use GCD to reduce the fraction)
// price = (bs_token_balance1 / token_balance) * (weth_balance / bs_token_balance2) * (10^18)
//       = bs_token_balance1 * weth_balance * 10^18 / (token_balance * bs_token_balance2)
fn token_price_in_ether(
    mut token_balance: U256,
    mut bs_token_balance1: U256,
    mut bs_token_balance2: U256,
    mut weth_balance: U256,
) -> U256 {
    // let's do this one step at a time
    let mut gcd;
    let mut multiplier = U256::from(10).pow(U256::from(18));

    gcd = multiplier.gcd(bs_token_balance2);
    multiplier /= gcd;
    bs_token_balance2 /= gcd;

    gcd = multiplier.gcd(token_balance);
    multiplier /= gcd;
    token_balance /= gcd;

    gcd = bs_token_balance1.gcd(token_balance);
    token_balance /= gcd;
    bs_token_balance1 /= gcd;

    gcd = bs_token_balance1.gcd(bs_token_balance2);
    bs_token_balance1 /= gcd;
    bs_token_balance2 /= gcd;

    gcd = weth_balance.gcd(bs_token_balance2);
    bs_token_balance2 /= gcd;
    weth_balance /= gcd;

    gcd = weth_balance.gcd(token_balance);
    token_balance /= gcd;
    weth_balance /= gcd;

    bs_token_balance1 * weth_balance * multiplier
        / (token_balance * bs_token_balance2)
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
            self.__get_best_baseline_token(state, token)?;

        let token_balance =
            self.__get_token_balance(state, token, best_pool)?;

        let bs_token_balance_in_pool1 =
            self.get_erc20_balance(state, best_bs_token, best_pool)?;

        // we need to update the decimals of the token to 18
        if best_bs_token == *WETH {
            Ok((
                token_price_in_ether(
                    token_balance,
                    bs_token_balance_in_pool1,
                    U256::from(10).pow(U256::from(18)),
                    U256::from(10).pow(U256::from(18)),
                ),
                best_liquidity,
            ))
        } else {
            let bc_pool =
                self.__get_pair_address(state, best_bs_token, *WETH)?;

            let bs_token_balance_in_pool2 =
                self.get_erc20_balance(state, best_bs_token, bc_pool)?;

            let weth_balance = self.get_erc20_balance(state, *WETH, bc_pool)?;

            Ok((
                token_price_in_ether(
                    token_balance,
                    bs_token_balance_in_pool1,
                    bs_token_balance_in_pool2,
                    weth_balance,
                ),
                best_liquidity,
            ))
        }
    }

    fn __get_best_baseline_token(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<(Address, Address, U256), SoflError<E>> {
        // iterate through all main stream tokens and fees
        let mut pool = Address::default();
        let mut bs_token = Address::default();
        let mut liquidity = U256::ZERO;

        // a shortcut for mainstream tokens
        if MAINSTREAM_TOKENS.contains(&token) {
            // this cannot be WETH
            pool = self.__get_pair_address(state, token, *WETH)?;
            bs_token = *WETH;
            liquidity = self.get_erc20_balance(state, token, pool)?;
        } else {
            for ms_token in MAINSTREAM_TOKENS.iter() {
                let cur_pool =
                    self.__get_pair_address(state, token, *ms_token)?;

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

    fn __get_token_balance(
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

    fn __get_pair_address(
        &mut self,
        state: &mut S,
        token1: Address,
        token2: Address,
    ) -> Result<Address, SoflError<E>> {
        let func = UNISWAP_V2_FACTORY_ABI.function("getPair").expect(
            "bug: cannot find getPair function in UniswapV2Factory ABI",
        );
        let token_pair = self.cheat_read(
            state,
            *UNISWAP_V2_FACTORY,
            func,
            &[Token::Address(token1.into()), Token::Address(token2.into())],
        )?;

        Ok(ToPrimitive::cvt(
            token_pair[0].clone().into_address().expect("cannot fail"),
        ))
    }
}

// Uniswap v3
impl<E, S> CheatCodes<S>
where
    E: Debug,
    S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
{
    fn _query_uniswap_v3(
        &mut self,
        state: &mut S,
        _token: Address,
    ) -> Result<(U256, U256), SoflError<E>> {
        // check whether uniswap v3 exists
        let func = UNISWAP_V3_FACTORY_ABI
            .function("owner")
            .expect("bug: cannot find owner function in UniswapV3Factory ABI");
        let _ = self.cheat_read(state, *UNISWAP_V3_FACTORY, func, &[])?;

        todo!()
    }
}
