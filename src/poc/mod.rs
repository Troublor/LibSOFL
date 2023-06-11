use ::ethers::prelude::{Http, Provider, TxHash};
use std::str::FromStr;

mod ethers;
mod evm;

fn provider() -> Provider<Http> {
    Provider::<Http>::try_from("http://127.0.0.1:8545").unwrap()
}

fn hex2hash(hash: &str) -> TxHash {
    TxHash::from_str("0x146063226f2bc60ab02fff825393555672ff505afb352ff11b820355422ba31e").unwrap()
}
