use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[allow(unused)]
pub struct RethConfig {
    pub datadir: String,
}

impl Default for RethConfig {
    fn default() -> Self {
        Self {
            datadir: String::from("/ssddata/wzhangcb/blockchain/mainnet-reth"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[allow(unused)]
pub struct SeeFuzzConfig {
    pub reth: RethConfig,
}
