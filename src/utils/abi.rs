lazy_static! {
    pub static ref UNISWAP_V2_ROUTER02_ABI: ethers::abi::Contract = {
        ethers::abi::Abi::load(std::io::Cursor::new(include_str!(
            "../../assets/uniswap_v2_router02.abi.json"
        )))
        .expect("failed to parse UniswapV2Router02 ABI")
    };
    pub static ref ERC20_ABI: ethers::abi::Contract = {
        ethers::abi::Abi::load(std::io::Cursor::new(include_str!(
            "../../assets/erc20.abi.json"
        )))
        .expect("failed to parse ERC20 ABI")
    };
    pub static ref ERC721_ABI: ethers::abi::Contract = {
        ethers::abi::Abi::load(std::io::Cursor::new(include_str!(
            "../../assets/erc721.abi.json"
        )))
        .expect("failed to parse ERC721 ABI")
    };
    pub static ref ERC777_ABI: ethers::abi::Contract = {
        ethers::abi::Abi::load(std::io::Cursor::new(include_str!(
            "../../assets/erc777.abi.json"
        )))
        .expect("failed to parse ERC777 ABI")
    };
    pub static ref ERC1155_ABI: ethers::abi::Contract = {
        ethers::abi::Abi::load(std::io::Cursor::new(include_str!(
            "../../assets/erc1155.abi.json"
        )))
        .expect("failed to parse ERC1155 ABI")
    };
    pub static ref WETH_ABI: ethers::abi::Contract = {
        ethers::abi::Abi::load(std::io::Cursor::new(include_str!(
            "../../assets/weth.abi.json"
        )))
        .expect("failed to parse WETH ABI")
    };
}
