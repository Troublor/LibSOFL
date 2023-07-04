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
