use std::str::FromStr;

use reth_primitives::Address;
use serde::{Deserialize, Serialize};

pub mod swap;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize,
)]
pub enum TokenAddress {
    #[default]
    Eth,
    ERC20(Address),
    ERC721(Address),
    ERC777(Address),
    ERC1155(Address),
}

impl From<TokenAddress> for Address {
    fn from(value: TokenAddress) -> Self {
        match value {
            TokenAddress::Eth => Address::from_slice(&[0xee; 20]),
            TokenAddress::ERC20(addr) => addr,
            TokenAddress::ERC721(addr) => addr,
            TokenAddress::ERC777(addr) => addr,
            TokenAddress::ERC1155(addr) => addr,
        }
    }
}

impl From<&TokenAddress> for Address {
    fn from(value: &TokenAddress) -> Self {
        match value {
            TokenAddress::Eth => Address::from_slice(&[0xee; 20]),
            TokenAddress::ERC20(addr) => *addr,
            TokenAddress::ERC721(addr) => *addr,
            TokenAddress::ERC777(addr) => *addr,
            TokenAddress::ERC1155(addr) => *addr,
        }
    }
}

impl TokenAddress {
    pub fn weth() -> Self {
        TokenAddress::ERC20(
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                .expect("failed to parse WETH address"),
        )
    }

    pub fn usdc() -> Self {
        TokenAddress::ERC20(
            Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
                .expect("failed to parse USDC address"),
        )
    }
}

impl TokenAddress {
    pub fn is_eth(&self) -> bool {
        matches!(self, TokenAddress::Eth)
    }

    pub fn is_weth(&self) -> bool {
        self == &Self::weth()
    }
}
