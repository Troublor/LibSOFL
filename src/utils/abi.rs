macro_rules! define_contract {
    ($name:ident, $path:expr) => {
        lazy_static! {
            pub static ref $name: ethers::abi::Contract = {
                ethers::abi::Abi::load(std::io::Cursor::new(include_str!(
                    $path
                )))
                .expect(concat!(
                    "failed to parse ",
                    stringify!($name),
                    " ABI"
                ))
            };
        }
    };
}

define_contract!(
    UNISWAP_V2_ROUTER02_ABI,
    "../../assets/uniswap_v2_router02.abi.json"
);
define_contract!(ERC20_ABI, "../../assets/erc20.abi.json");
define_contract!(ERC721_ABI, "../../assets/erc721.abi.json");
define_contract!(ERC777_ABI, "../../assets/erc777.abi.json");
define_contract!(ERC1155_ABI, "../../assets/erc1155.abi.json");
define_contract!(WETH_ABI, "../../assets/weth.abi.json");

// Uniswap
define_contract!(UNISWAP_V2_PAIR_ABI, "../../assets/uniswap_v2_pair.abi.json");
define_contract!(
    UNISWAP_V2_FACTORY_ABI,
    "../../assets/uniswap_v2_factory.abi.json"
);
define_contract!(
    UNISWAP_V3_FACTORY_ABI,
    "../../assets/uniswap_v3_factory.abi.json"
);
define_contract!(UNISWAP_V3_POOL_ABI, "../../assets/uniswap_v3_pool.abi.json");
define_contract!(CURVE_REGISTRY_ABI, "../../assets/curve_registry.abi.json");

// Curve.FI
define_contract!(
    CURVE_CRYPTO_REGISTRY_ABI,
    "../../assets/curve_crypto_registry.abi.json"
);
define_contract!(CURVE_POOL_ABI, "../../assets/curve_pool.abi.json");
define_contract!(
    CURVE_CRYPTO_POOL_ABI,
    "../../assets/curve_crypto_pool.abi.json"
);
define_contract!(CURVE_EXCHANGE_ABI, "../../assets/curve_exchange.abi.json");
define_contract!(CURVE_Y_VAULT_ABI, "../../assets/curve_y_vault.abi.json");

// Inverse Finance
define_contract!(
    INVERSE_LENDING_POOL_ABI,
    "../../assets/inverse_lending_pool.abi.json"
);
define_contract!(
    INVERSE_LENDING_COMPTROLLER_ABI,
    "../../assets/inverse_lending_comptroller.abi.json"
);

#[cfg(test)]
pub mod test_utils {
    define_contract!(
        YV_CRV3_CRYPTO_FEED,
        "../../assets/test/yvcrv3cryptofeed.abi.json"
    );
    define_contract!(
        YV_CRV3_CRYPTO,
        "../../assets/test/yvcrv3crypto_token.abi.json"
    );
}

pub(crate) mod macros {

    #[macro_export]
    macro_rules! convert_to_primitive {
        ($v: expr, $f: ty, $t: ty) => {
            <$crate::utils::conversion::ToPrimitive as $crate::utils::conversion::Convert<$f, $t>>::cvt($v)
        };
    }

    #[macro_export]
    macro_rules! unwrap_first_token_value {
        (Address, $v:expr) => {
            $crate::convert_to_primitive!(
                $v.remove(0)
                    .into_address()
                    .expect("impossible: return value is not address"),
                ethers::types::Address,
                reth_primitives::Address
            )
        };
        (Vec<u8>, $v:expr) => {
            (match $v.remove(0) {
                ethers::abi::Token::FixedBytes(v) => Some(v),
                ethers::abi::Token::Bytes(v) => Some(v),
                _ => panic!(
                    "impossible: return value is not bytes or fixed_bytes"
                ),
            })
            .expect("impossible: return value is not fixed_byte")
        };
        (Int, $v:expr) => {
            ethers::types::I256::from_raw(
                $v.remove(0)
                    .into_int()
                    .expect("impossible: return value is not int"),
            )
        };
        (Uint, $v:expr) => {
            $crate::convert_to_primitive!(
                $v.remove(0)
                    .into_uint()
                    .expect("impossible: return value is not uint"),
                ethers::types::U256,
                revm_primitives::U256
            )
        };
        (bool, $v:expr) => {
            $v.remove(0)
                .into_bool()
                .expect("impossible: return value is not bool")
        };
        (String, $v:expr) => {
            $v.remove(0)
                .into_string()
                .expect("impossible: return value is not string")
        };
        (Vec<Address>, $v:expr) => {
            (match $v.remove(0) {
                ethers::abi::Token::Array(arr)
                | ethers::abi::Token::FixedArray(arr)
                | ethers::abi::Token::Tuple(arr) => arr
                    .into_iter()
                    .map(|v| {
                        $crate::convert_to_primitive!(
                            v.into_address().expect(
                                "impossible: return value is not address"
                            ),
                            ethers::types::Address,
                            reth_primitives::Address
                        )
                    })
                    .collect::<Vec<revm_primitives::Address>>(),
                _ => panic!("impossible: return value is not array"),
            })
        };
        (Vec<Token>, $v:expr) => {
            (match $v.remove(0) {
                ethers::abi::Token::Array(v)
                | ethers::abi::Token::FixedArray(v)
                | ethers::abi::Token::Tuple(v) => Some(v),
                _ => panic!("impossible: return value is not array"),
            })
            .expect("impossible: return value is not array or tuple")
        };
    }

    #[macro_export]
    macro_rules! unwrap_token_values {
        ($v: expr, $($t:tt),*) => {{
            let mut ret = { $v };
            (
                $(
                    $crate::unwrap_first_token_value!($t, ret),
                )*
            )
        }};
    }

    #[cfg(test)]
    mod tests_nodep {
        use ethers::abi::Token;
        use reth_primitives::Address;
        use revm_primitives::U256;

        #[test]
        fn test_unwrap_single() {
            let ret = vec![Token::Address(ethers::types::H160::zero())];
            let (addr,) = unwrap_token_values!(ret, Address);
            assert_eq!(addr, Address::zero());
        }

        #[test]
        fn test_unwrap_multiple() {
            let ret = vec![
                Token::Address(ethers::types::H160::zero()),
                Token::Uint(ethers::types::U256::zero()),
            ];
            let (addr, value) = unwrap_token_values!(ret, Address, Uint);
            assert_eq!(addr, Address::zero());
            assert_eq!(value, U256::ZERO);
        }
    }
}
