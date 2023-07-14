use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(unused)]
pub struct JsonRpcConfig {
    pub endpoint: String,
    pub cloudflare_client_id: Option<String>,
    pub cloudflare_client_secret: Option<String>,
}

impl From<JsonRpcConfig> for Option<HeaderMap> {
    fn from(val: JsonRpcConfig) -> Self {
        let mut headers = HeaderMap::new();
        if let Some(id) = val.cloudflare_client_id {
            headers.insert("CF-Access-Client-Id", id.parse().unwrap());
        } else {
            return None;
        }
        if let Some(secret) = val.cloudflare_client_secret {
            headers.insert("CF-Access-Client-Secret", secret.parse().unwrap());
        } else {
            return None;
        }
        Some(headers)
    }
}

impl Default for JsonRpcConfig {
    fn default() -> Self {
        Self {
            endpoint: String::from("http://localhost:8545"),
            cloudflare_client_id: None,
            cloudflare_client_secret: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EtherscanConfig {
    pub url: String,
    pub api_url: String,
    pub api_key: Option<String>,
    /// Number of requests per second
    pub rate_limit: Option<u32>,
}

impl Default for EtherscanConfig {
    fn default() -> Self {
        Self {
            url: String::from("https://etherscan.io"),
            api_url: String::from("https://api.etherscan.io/api"),
            api_key: None,
            rate_limit: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[allow(unused)]
pub struct SoflConfig {
    pub reth: RethConfig,
    pub jsonrpc: JsonRpcConfig,
    pub etherscan: EtherscanConfig,
}
