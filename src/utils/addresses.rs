use std::str::FromStr;

use reth_primitives::Address;

macro_rules! define_address {
    ($name:ident, $addr:expr) => {
        lazy_static! {
            pub static ref $name: Address = Address::from_str($addr)
                .expect(concat!("failed to parse ", stringify!($name)));
        }
    };
}

// Tokens
define_address!(WETH, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
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
