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

// Dummy Address
define_address!(BURNER_ADDRESS, "0x0123456789abcDEF0123456789abCDef01234567");

// Default Caller Address
define_address!(
    DEFAULT_CALLER_ADDRESS,
    "0x4354bB7C9dad5b0299199c0084E6ae386afD636C"
);
