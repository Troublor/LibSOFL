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

#[derive(Debug, Deserialize, Serialize)]
#[allow(unused)]
pub struct JsonRpcConfig {
    pub endpoint: String,
}

impl Default for JsonRpcConfig {
    fn default() -> Self {
        Self {
            endpoint: String::from("http://localhost:8545"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[allow(unused)]
pub struct SoflConfig {
    pub reth: RethConfig,
    pub jsonrpc: JsonRpcConfig,
}
