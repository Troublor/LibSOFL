#![allow(dead_code)]
#![allow(unused_doc_comments)]

use alloy_sol_macro::sol;
use ethers::types::Chain;
use paste::paste;
use std::collections::HashMap;

use libsofl_core::{conversion::ConvertTo, engine::types::Address};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiChainAddress {
    /// A single address that is valid on all chains.
    Fixed(Address),
    /// A specific addresses on specific chains.
    Varying(HashMap<Chain, Address>),
}

impl MultiChainAddress {
    pub(crate) fn from_kv_pairs(kvs: Vec<(Chain, &str)>) -> Self {
        let mut map = HashMap::new();
        for (chain, addr) in kvs {
            map.insert(chain, addr.cvt());
        }
        MultiChainAddress::Varying(map)
    }
}

impl MultiChainAddress {
    pub fn on_chain(&self, chain: impl Into<Chain>) -> Option<Address> {
        match self {
            MultiChainAddress::Fixed(addr) => Some(*addr),
            MultiChainAddress::Varying(addrs) => {
                addrs.get(&chain.into()).copied()
            }
        }
    }

    pub fn must_on_chain(&self, chain: impl Into<Chain>) -> Address {
        self.on_chain(chain).expect("address not found on chain")
    }

    pub fn fixed(&self) -> Address {
        match self {
            MultiChainAddress::Fixed(addr) => *addr,
            MultiChainAddress::Varying(_) => panic!("address is not fixed"),
        }
    }
}

impl From<&str> for MultiChainAddress {
    fn from(addr: &str) -> Self {
        MultiChainAddress::Fixed(addr.cvt())
    }
}

impl From<Address> for MultiChainAddress {
    fn from(addr: Address) -> Self {
        MultiChainAddress::Fixed(addr)
    }
}

#[macro_export(local_inner_macros)]
macro_rules! __address_book_address {
    ({ $( $chain:ident => $addr:literal ),* }) => {
        MultiChainAddress::from_kv_pairs(std::vec![
            $( (Chain::$chain, std::stringify!($addr)) ),*
        ])
    };
    ($addr:literal) => {
        MultiChainAddress::from(std::stringify!($addr))
    };
}

macro_rules! address_book {
    ($( $contract:ident $( ( $abi:expr ) )? = $payload:tt ),*) => {
        pub struct AddressBook {
            $(pub $contract: MultiChainAddress),*
        }

        lazy_static! {
            pub static ref ADDRESS_BOOK: AddressBook = {
                AddressBook {
                    $($contract: __address_book_address!($payload)),*
                }
            };
        }

        $(
            $(
                paste! {
                    sol!([<$contract:camel ABI>], $abi);
                }
            )?
        )*
    };
}

/// Address book for common contracts.
/// The macro will generate a static `ADDRESS_BOOK` variable of type `AddressBook`, which contains fields for each contract.
/// The key to each address is declared below as the field name of the contract (in snake case) in `ADDRESS_BOOK` static object.
/// For example, `ADDRESS_BOOK.weth` will get the `MultiChainAddress` of the WETH contract.
/// If a ABI json file is provided, a corresponding ABI type will be generated, named as the camel case of the contract name with a `ABI` suffix.
/// For example, the `uniswap_v2_factory` contract will have a corresponding `UniswapV2FactoryABI` type.
address_book! {
    dummy = 0x0123456789abcDEF0123456789abCDef01234567,
    default_caller = 0x4354bB7C9dad5b0299199c0084E6ae386afD636C,
    eth = 0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE,

    // ERC20 tokens
    weth("abi/weth.abi.json") = {
        Mainnet => 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2
    },
    wbtc("abi/erc20.abi.json") = {
        Mainnet => 0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599
    },
    usdc("abi/erc20.abi.json") = {
        Mainnet => 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
    },
    usdt("abi/erc20.abi.json") = {
        Mainnet => 0xdAC17F958D2ee523a2206206994597C13D831ec7
    },
    dai("abi/erc20.abi.json") = {
        Mainnet => 0x6B175474E89094C44Da98b954EedeAC495271d0F
    },

    // Uniswap
    uniswap_v3_factory("abi/uniswap_v3_factory.abi.json") = {
        Mainnet => 0x1F98431c8aD98523631AE4a59f267346ea31F984
    },
    uniswap_v2_factory("abi/uniswap_v2_factory.abi.json") = {
        Mainnet => 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f
    },

    // Curve.FI
    curve_address_provider = {
        Mainnet => 0x0000000022D53366457F9d5E68Ec105046FC4383
    },
    curve_registry("abi/curve_registry.abi.json") = {
        Mainnet => 0x90E00ACe148ca3b23Ac1bC8C240C2a7Dd9c2d7f5
    },
    curve_exchange("abi/curve_exchange.abi.json") = {
        Mainnet => 0x8e764bE4288B842791989DB5b8ec067279829809
    },
    curve_crypto_registry("abi/curve_crypto_registry.abi.json") = {
        Mainnet => 0x8F942C20D02bEfc377D41445793068908E2250D0
    },

    // AAVE
    aave_lending_pool_v2("abi/aave_lending_pool_v2.abi.json") = {
        Mainnet => 0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9
    }
}

// Common ABIs
sol!(ERC20ABI, "abi/erc20.abi.json");
sol!(AaveAtokenV2ABI, "abi/aave_atoken_v2.abi.json");
sol!(CurveCryptoPoolABI, "abi/curve_crypto_pool.abi.json");
sol!(CurvePoolABI, "abi/curve_pool.abi.json");
sol!(CurveYVaultABI, "abi/curve_y_vault.abi.json");
sol!(ERC721ABI, "abi/erc721.abi.json");
sol!(ERC777ABI, "abi/erc777.abi.json");
sol!(ERC1155ABI, "abi/erc1155.abi.json");
sol!(ERC4626ABI, "abi/erc4626.abi.json");
sol!(
    InverseLendingComptrollerABI,
    "abi/inverse_lending_comptroller.abi.json"
);
sol!(InverseLendingPoolABI, "abi/inverse_lending_pool.abi.json");
sol!(UniswapV2PairABI, "abi/uniswap_v2_pair.abi.json");
sol!(UniswapV2Router02ABI, "abi/uniswap_v2_router02.abi.json");
sol!(UniswapV3PoolABI, "abi/uniswap_v3_pool.abi.json");

#[cfg(test)]
mod tests {

    use crate::types::Chain;
    use libsofl_core::{conversion::ConvertTo, engine::types::Address};

    use super::ADDRESS_BOOK;

    #[test]
    fn test_address_book() {
        let addr: Address = ADDRESS_BOOK.eth.must_on_chain(Chain::Mainnet);
        let expected: Address =
            "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".cvt();
        assert_eq!(addr, expected);
        let addr: Address = ADDRESS_BOOK.weth.must_on_chain(Chain::Mainnet);
        let expected: Address =
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".cvt();
        assert_eq!(addr, expected);
    }
}
