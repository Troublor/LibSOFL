use ethers::abi::Abi;
use reth_primitives::Address;

use crate::{
    config::flags::SoflConfig,
    error::SoflError,
    utils::conversion::{Convert, ToEthers},
};

pub trait AbiProvider {
    /// Get the ABI for a contract.
    /// If the address is not a contract or does not have ABI available, return None.
    fn get_abi(&self, contract: Address) -> Result<Option<Abi>, SoflError>;
}

pub struct EtherscanAbiProvider {
    runtime: tokio::runtime::Runtime,
    client: ethers::etherscan::Client,
}

impl Default for EtherscanAbiProvider {
    fn default() -> Self {
        let cfg = SoflConfig::load().expect("Failed to load config");
        let builder = ethers::etherscan::ClientBuilder::default()
            .with_api_url(cfg.etherscan.api_url);
        let mut builder = match builder {
            Ok(builder) => builder,
            Err(err) => panic!("Failed to build etherscan client: {}", err),
        };
        if let Some(key) = cfg.etherscan.api_key {
            builder = builder.with_api_key(key)
        };
        let client = match builder.build() {
            Ok(client) => client,
            Err(err) => panic!("Failed to build etherscan client: {}", err),
        };
        Self {
            runtime: tokio::runtime::Runtime::new()
                .expect("Failed to create tokio runtime"),
            client,
        }
    }
}

impl AbiProvider for EtherscanAbiProvider {
    fn get_abi(&self, contract: Address) -> Result<Option<Abi>, SoflError> {
        let maybe_abi = self
            .runtime
            .block_on(self.client.contract_abi(ToEthers::cvt(contract)));
        let abi = match maybe_abi {
            Ok(abi) => Some(abi),
            Err(ethers::etherscan::errors::EtherscanError::ContractCodeNotVerified(_)) => None,
            Err(ethers::etherscan::errors::EtherscanError::RateLimitExceeded) => {
                todo!()
            }
            Err(e) => return Err(SoflError::Etherscan(e)),
        };

        todo!()
    }
}
