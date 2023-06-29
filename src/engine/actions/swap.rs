use ethers::abi::Token;
use reth_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::engine::transactions::{builder::TxBuilder, Tx};
use crate::utils::conversion::{Convert, ToEthers};

use super::TokenAddress;

lazy_static! {
    static ref UNISWAP_V2_ROUTER02_ABI: ethers::abi::Contract = {
        ethers::abi::parse_abi_str(include_str!(
            "../../../assets/uniswap_v2_router02.abi.json"
        ))
        .expect("failed to parse ERC20 ABI")
    };
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Swap {
    pub caller: Address,
    pub path: Vec<TokenAddress>,
    pub amount: u128, // swap from token_pair.0 to token_pair.1
}

impl Swap {
    pub fn to_tx(&self, contract: Address) -> Tx {
        let path_error_msg = "path must have at least 2 elements";
        let (func, args, value): (&ethers::abi::Function, Vec<Token>, u128) =
            match self.path {
                _ if self.path.first().expect(path_error_msg).is_eth()
                    && self.path.last().expect(path_error_msg).is_eth() =>
                {
                    panic!("cannot swap eth for eth")
                }
                _ if self.path.first().expect(path_error_msg).is_eth() => (
                    UNISWAP_V2_ROUTER02_ABI
                        .function("swapExactETHForTokens")
                        .expect("failed to get swapExactETHForTokens function"),
                    vec![
                        Token::Uint(0.into()),
                        Token::Array(
                            self.path
                                .iter()
                                .map(|t| {
                                    Token::Address(ToEthers::cvt(
                                        &Address::from(t),
                                    ))
                                })
                                .collect(),
                        ),
                        Token::Address(ToEthers::cvt(&self.caller)),
                        Token::Uint(ethers::types::U256::MAX),
                    ],
                    self.amount,
                ),
                _ if self.path.last().expect(path_error_msg).is_eth() => (
                    UNISWAP_V2_ROUTER02_ABI
                        .function("swapExactTokensForETH")
                        .expect("failed to get swapExactTokensForETH function"),
                    vec![
                        Token::Uint(self.amount.into()),
                        Token::Uint(0.into()),
                        Token::Array(
                            self.path
                                .iter()
                                .map(|t| {
                                    Token::Address(ToEthers::cvt(
                                        &Address::from(t),
                                    ))
                                })
                                .collect(),
                        ),
                        Token::Address(ToEthers::cvt(&self.caller)),
                        Token::Uint(ethers::types::U256::MAX),
                    ],
                    0,
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
                        Token::Array(
                            self.path
                                .iter()
                                .map(|t| {
                                    Token::Address(ToEthers::cvt(
                                        &Address::from(t),
                                    ))
                                })
                                .collect(),
                        ),
                        Token::Address(ToEthers::cvt(&self.caller)),
                        Token::Uint(ethers::types::U256::MAX),
                    ],
                    0,
                ),
            };
        TxBuilder::new()
            .set_to(contract)
            .set_input_with_high_level_call(func, &args)
            .set_value(value)
            .build()
    }
}
