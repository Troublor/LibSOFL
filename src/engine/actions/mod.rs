use reth_primitives::Address;
use serde::{Deserialize, Serialize};

pub mod swap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fn is_eth(&self) -> bool {
        matches!(self, TokenAddress::Eth)
    }
}
