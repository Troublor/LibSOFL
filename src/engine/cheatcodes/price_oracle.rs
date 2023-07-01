use std::{cmp::Ordering, str::FromStr};

use ethers::abi::{ParamType, Token};
use reth_downloaders::bodies::task::BODIES_TASK_BUFFER_SIZE;
use reth_primitives::Address;
use revm_primitives::{B256, U256};

use crate::{
    engine::state::BcState,
    error::SoflError,
    utils::conversion::{Convert, ToElementary, ToEthers, ToPrimitive},
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

pub trait PriceOracleCheat<S: BcState> {
    fn get_price_in_ether(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<S::DbErr>>;
}

impl<S: BcState> PriceOracleCheat<S> for CheatCodes<S> {
    fn get_price_in_ether(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<S::DbErr>> {
        let (price, liquidity) = self.query_uniswap_v2(state, token)?;
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
impl<S: BcState> CheatCodes<S> {
    fn query_uniswap_v2(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<(U256, U256), SoflError<S::DbErr>> {
        // check whether uniswap v3 exists
        let _ = self.cheat_read(
            state,
            *UNISWAP_V2_FACTORY,
            0x094b7415, /* feeToSetter */
            &[],
            &[ParamType::Address],
        )?;

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
    ) -> Result<(Address, Address, U256), SoflError<S::DbErr>> {
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
    ) -> Result<U256, SoflError<S::DbErr>> {
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
    ) -> Result<Address, SoflError<S::DbErr>> {
        let token_pair = self.cheat_read(
            state,
            *UNISWAP_V2_FACTORY,
            0xe6a43905, // getPair
            &[Token::Address(token1.into()), Token::Address(token2.into())],
            &[ParamType::Address],
        )?;

        Ok(ToPrimitive::cvt(
            token_pair[0].clone().into_address().expect("cannot fail"),
        ))
    }
}

// Uniswap v3
impl<S: BcState> CheatCodes<S> {
    fn query_uniswap_v3(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<(U256, U256), SoflError<S::DbErr>> {
        // check whether uniswap v3 exists
        let _ = self.cheat_read(
            state,
            *UNISWAP_V3_FACTORY,
            0x8da5cb5bu32, /* owner() */
            &[],
            &[ParamType::Address],
        )?;

        if token == *WETH {
            return Ok((U256::from(10).pow(U256::from(18)), U256::MAX));
        }

        // iterate through all main stream tokens and fees
        let mut best_pool = Address::default();
        let mut best_ms_token = Address::default();
        let mut best_liquidity = U256::ZERO;

        // a shortcut for mainstream tokens
        if MAINSTREAM_TOKENS.contains(&token) {
            // this cannot be WETH
            best_pool = ToPrimitive::cvt(
                &self.cheat_read(
                    state,
                    *UNISWAP_V3_FACTORY,
                    0x1698ee82u32, // getPool
                    &[
                        Token::Address(token.into()),
                        Token::Address((*WETH).into()),
                        Token::Uint(ethers::types::U256::from(500)), // WETH-USD pool
                    ],
                    &[ParamType::Address],
                )?[0]
                    .clone()
                    .into_address()
                    .expect("cannot fail"),
            );
            best_ms_token = *WETH;
            best_liquidity = self.get_erc20_balance(state, token, best_pool)?;
        } else {
            for ms_token in MAINSTREAM_TOKENS.iter() {
                for fee in UNISWAP_V3_FEES.iter() {
                    let pool: Address = ToPrimitive::cvt(
                        &self.cheat_read(
                            state,
                            *UNISWAP_V3_FACTORY,
                            0x1698ee82u32, // getPool
                            &[
                                Token::Address(token.into()),
                                Token::Address((*ms_token).into()),
                                Token::Uint(ethers::types::U256::from(*fee)),
                            ],
                            &[ParamType::Address],
                        )?[0]
                            .clone()
                            .into_address()
                            .expect("cannot fail"),
                    );
                    if pool == Address::from(0) {
                        continue;
                    }

                    if let Ok(token_liquidity) =
                        self.get_erc20_balance(state, token, pool)
                    {
                        if token_liquidity > best_liquidity {
                            best_liquidity = token_liquidity;
                            best_pool = pool;
                            best_ms_token = *ms_token;
                        }
                    }
                }
            }
        }

        // if no pool found, return error
        if best_pool == Address::default() {
            return Err(SoflError::Custom(
                "No pool found for uniswap v3".to_string(),
            ));
        }

        todo!()
    }
}
