use std::str::FromStr;

use ethers::types::Chain;
use reth_primitives::Address;
use revm_primitives::HashMap;

macro_rules! define_address {
    ($name:ident, $addr:expr) => {
        lazy_static! {
            pub static ref $name: Address = Address::from_str($addr)
                .expect(concat!("failed to parse ", stringify!($name)));
        }
    };
}

// Convensions
define_address!(ETH, "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE");

// Tokens
define_address!(WETH, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
define_address!(WBTC, "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599");
define_address!(USDC, "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
define_address!(USDT, "0xdAC17F958D2ee523a2206206994597C13D831ec7");
define_address!(DAI, "0x6B175474E89094C44Da98b954EedeAC495271d0F");

// Uniswap
define_address!(
    UNISWAP_V3_FACTORY,
    "0x1F98431c8aD98523631AE4a59f267346ea31F984"
);
define_address!(
    UNISWAP_V2_FACTORY,
    "0x5c69bee701ef814a2b6a3edd4b1652cb9cc5aa6f"
);

// Curve.FI
define_address!(
    CURVE_ADDRESS_PROVIDER,
    "0x0000000022D53366457F9d5E68Ec105046FC4383"
);
// The main registry contract. Used to locate pools and query information about them.
define_address!(CURVE_REGISTRY, "0x90E00ACe148ca3b23Ac1bC8C240C2a7Dd9c2d7f5");
// Generalized swap contract. Used for finding rates and performing exchanges.
define_address!(CURVE_EXCHANGE, "0x8e764bE4288B842791989DB5b8ec067279829809");
// The cryptoswap factory.
define_address!(
    CURVE_CRYPTO_REGISTRY,
    "0x8F942C20D02bEfc377D41445793068908E2250D0"
);

// Inverse Finance
define_address!(
    INVERSE_LENDING_COMPTROLLER,
    "0x4dCf7407AE5C07f8681e1659f626E114A7667339"
);

// Aave
define_address!(
    AAVE_LENDING_POOL_V2,
    "0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9"
);

// Dummy Address
define_address!(BURNER_ADDRESS, "0x0123456789abcDEF0123456789abCDef01234567");

// Default Caller Address
define_address!(
    DEFAULT_CALLER_ADDRESS,
    "0x4354bB7C9dad5b0299199c0084E6ae386afD636C"
);

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::AsRef,
    derive_more::AsMut,
    derive_more::From,
)]
pub struct MultiChainAddress {
    default: Option<Address>,

    #[deref]
    #[deref_mut]
    #[as_ref]
    #[as_mut]
    addresses: HashMap<Chain, Address>,
}

impl MultiChainAddress {
    pub fn on_chain(&self, chain: Chain) -> Option<Address> {
        let addr = self.addresses.get(&chain);
        if addr.is_none() && self.default.is_none() {
            self.default
        } else {
            addr.copied()
        }
    }

    pub fn must_on_chain(&self, chain: Chain) -> Address {
        self.on_chain(chain).expect("address not found on chain")
    }
}

impl From<Address> for MultiChainAddress {
    fn from(addr: Address) -> Self {
        Self {
            default: Some(addr),
            addresses: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AddressBook {
    // misc
    pub eth: MultiChainAddress,
    pub dummy: MultiChainAddress,
    pub default_caller: MultiChainAddress,

    // ERC20 tokens
    pub weth: MultiChainAddress,
    pub wbtc: MultiChainAddress,
    pub usdc: MultiChainAddress,
    pub usdt: MultiChainAddress,
    pub dai: MultiChainAddress,

    // Uniswap
    pub uniswap_v3_factory: MultiChainAddress,
    pub uniswap_v2_factory: MultiChainAddress,

    // Curve.FI
    pub curve_address_provider: MultiChainAddress,
    pub curve_registry: MultiChainAddress,
    pub curve_exchange: MultiChainAddress,
    pub curve_crypto_registry: MultiChainAddress,

    // Aave
    pub aave_lending_pool_v2: MultiChainAddress,
}

macro_rules! decl_address {
    ($book:tt, $contract:ident, $($chain:tt => $a:tt),*) => {{
        let book = &mut $book;
        $(
            let addr = <crate::utils::conversion::ToPrimitive as crate::utils::conversion::Convert::<&str, reth_primitives::Address>>::cvt(stringify!($a));
            book.$contract.insert(ethers::types::Chain::$chain, addr);
        )*
    }};
}

macro_rules! decl_address_fixed {
    ($book:tt, $contract:ident, $addr:tt) => {{
        let book = &mut $book;
        let addr = <crate::utils::conversion::ToPrimitive as crate::utils::conversion::Convert::<&str, reth_primitives::Address>>::cvt(stringify!($addr));
        book.$contract.default = Some(addr);
    }};
}

lazy_static! {
    pub static ref ADDRESS_BOOK: AddressBook = {
        let mut book = AddressBook::default();

        // misc
        decl_address_fixed! {
            book,
            eth,
            0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE
        };
        decl_address_fixed! {
            book,
            dummy,
            0x0123456789abcDEF0123456789abCDef01234567
        };
        decl_address_fixed! {
            book,
            default_caller,
            0x4354bB7C9dad5b0299199c0084E6ae386afD636C
        };

        // ERC20 tokens
        decl_address! {
            book,
            weth,
            Mainnet => 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2
        };
        decl_address! {
            book,
            wbtc,
            Mainnet => 0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599
        };
        decl_address! {
            book,
            usdc,
            Mainnet => 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
        };
        decl_address! {
            book,
            usdt,
            Mainnet => 0xdAC17F958D2ee523a2206206994597C13D831ec7
        };
        decl_address! {
            book,
            dai,
            Mainnet => 0x6B175474E89094C44Da98b954EedeAC495271d0F
        };

        // Uniswap
        decl_address! {
            book,
            uniswap_v3_factory,
            Mainnet => 0x1F98431c8aD98523631AE4a59f267346ea31F984
        };
        decl_address! {
            book,
            uniswap_v2_factory,
            Mainnet => 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f
        };

        // Curve.FI
        decl_address!(
            book,
            curve_address_provider,
            Mainnet => 0x0000000022D53366457F9d5E68Ec105046FC4383
        );
        decl_address!(
            book,
            curve_registry,
            Mainnet => 0x90E00ACe148ca3b23Ac1bC8C240C2a7Dd9c2d7f5
        );
        decl_address!(
            book,
            curve_exchange,
            Mainnet => 0x8e764bE4288B842791989DB5b8ec067279829809
        );
        decl_address!(
            book,
            curve_crypto_registry,
            Mainnet => 0x8F942C20D02bEfc377D41445793068908E2250D0
        );

        // Aave
        decl_address!(
            book,
            aave_lending_pool_v2,
            Mainnet => 0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9
        );
        book
    };
}
