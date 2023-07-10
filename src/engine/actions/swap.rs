use std::str::FromStr;

use ethers::abi::Token;
use reth_primitives::Address;
use revm::{Database, DatabaseCommit};
use revm_primitives::U256;
use serde::{Deserialize, Serialize};

use crate::engine::inspectors::no_inspector;
use crate::engine::utils::HighLevelCaller;
use crate::utils::abi::UNISWAP_V2_ROUTER02_ABI;
use crate::utils::conversion::{Convert, ToEthers, ToPrimitive};

use super::TokenAddress;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniswapV2Swap {
    pub path: Vec<TokenAddress>,
    pub amount: U256, // swap from token_pair.0 to token_pair.1
}

impl UniswapV2Swap {
    pub fn router02_address() -> Address {
        Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
            .expect("failed to parse uniswap v2 router02 address")
    }

    pub fn factory_address() -> Address {
        Address::from_str("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")
            .expect("failed to parse uniswap v2 factory address")
    }

    pub fn router02_transact<BS>(
        &self,
        state: &mut BS,
        caller: HighLevelCaller,
    ) -> U256
    where
        BS: Database + DatabaseCommit,
        BS::Error: std::fmt::Debug,
    {
        let path_error_msg = "path must have at least 2 elements";
        let swap_path: Vec<Token> = self
            .path
            .iter()
            .map(|t| if t.is_eth() { TokenAddress::weth() } else { *t })
            .map(|t| Token::Address(ToEthers::cvt(&Address::from(t))))
            .collect();
        let (func, args, value): (&ethers::abi::Function, Vec<Token>, U256) =
            match self.path {
                _ if self.path.first().expect(path_error_msg).is_weth()
                    && self.path.last().expect(path_error_msg).is_weth() =>
                {
                    panic!("cannot swap eth for eth")
                }
                _ if self.path.first().expect(path_error_msg).is_weth() => (
                    UNISWAP_V2_ROUTER02_ABI
                        .function("swapExactETHForTokens")
                        .expect("failed to get swapExactETHForTokens function"),
                    vec![
                        Token::Uint(0.into()),
                        Token::Array(swap_path),
                        Token::Address(ToEthers::cvt(&caller.address)),
                        Token::Uint(ethers::types::U256::MAX),
                    ],
                    self.amount,
                ),
                _ if self.path.last().expect(path_error_msg).is_weth() => (
                    UNISWAP_V2_ROUTER02_ABI
                        .function("swapExactTokensForETH")
                        .expect("failed to get swapExactTokensForETH function"),
                    vec![
                        Token::Uint(self.amount.into()),
                        Token::Uint(0.into()),
                        Token::Array(swap_path),
                        Token::Address(ToEthers::cvt(&caller.address)),
                        Token::Uint(ethers::types::U256::MAX),
                    ],
                    U256::default(),
                ),
                _ => (
                    UNISWAP_V2_ROUTER02_ABI
                        .function("swapExactTokensForTokens")
                        .expect(
                            "failed to get swapExactTokensForTokens function",
                        ),
                    vec![
                        Token::Uint(self.amount.into()),
                        Token::Uint(0.into()),
                        Token::Array(swap_path),
                        Token::Address(ToEthers::cvt(&caller.address)),
                        Token::Uint(ethers::types::U256::MAX),
                    ],
                    U256::default(),
                ),
            };
        let rets = caller
            .invoke(
                state,
                Self::router02_address(),
                func,
                &args,
                Some(value),
                no_inspector(),
            )
            .expect("failed to invoke swap");
        let Token::Array(ret) =
            rets.first().expect("should have one return value")
        else {
            panic!("the return value should be an array")
        };
        let Token::Uint(output) =
            ret.last().expect("should have at least one return value")
        else {
            panic!("the return value should be an uint")
        };
        ToPrimitive::cvt(output)
    }
}

#[cfg(test)]
mod tests_with_dep {

    use ethers::utils::parse_ether;
    use reth_primitives::Address;

    use crate::{
        engine::{
            state::BcStateBuilder, transactions::position::TxPosition,
            utils::HighLevelCaller,
        },
        utils::{
            conversion::{Convert, ToPrimitive},
            testing::get_testing_bc_provider,
        },
    };

    #[test]
    fn test_swap() {
        let p = get_testing_bc_provider();
        let mut state =
            BcStateBuilder::fork_at(&p, TxPosition::new(16000000, 0)).unwrap();
        let swap = super::UniswapV2Swap {
            path: vec![
                super::TokenAddress::weth(),
                super::TokenAddress::usdc(),
            ],
            amount: ToPrimitive::cvt(parse_ether("1").unwrap()),
        };
        let addr: Address = ToPrimitive::cvt(1);
        let caller = HighLevelCaller::from(addr).bypass_check();
        let out = swap.router02_transact(&mut state, caller);
        assert_eq!(out, ToPrimitive::cvt(1_205_296_412));
    }
}
