use crate::config::flags::SoflConfig;

#[derive(
    Clone,
    Debug,
    derive_more::AsRef,
    derive_more::AsMut,
    derive_more::Deref,
    derive_more::DerefMut,
)]
pub struct EtherscanClient {
    #[as_ref]
    #[as_mut]
    #[deref]
    #[deref_mut]
    client: ethers::etherscan::Client,
}

impl Default for EtherscanClient {
    fn default() -> Self {
        let cfg = SoflConfig::load().expect("Failed to load config");
        let builder = ethers::etherscan::ClientBuilder::default()
            .with_api_url(cfg.etherscan.api_url);
        let builder = match builder {
            Ok(builder) => builder,
            Err(err) => panic!("Failed to build etherscan client: {}", err),
        };
        let mut builder = match builder.with_url(cfg.etherscan.url) {
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
        Self { client }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitController {
    /// The number of requests that can be made every second
    pub limit: usize,

    /// The last time a request was made
    last_request_time: std::time::SystemTime,
}

impl Default for RateLimitController {
    fn default() -> Self {
        Self {
            limit: 5,
            last_request_time: std::time::UNIX_EPOCH,
        }
    }
}

impl RateLimitController {
    pub fn _call<T>(
        &mut self,
        f: impl Fn() -> Result<T, ethers::etherscan::errors::EtherscanError>,
        max_attemps: Option<usize>,
    ) -> Result<T, ethers::etherscan::errors::EtherscanError> {
        let mut i = 0;
        loop {
            let now = std::time::SystemTime::now();
            let elapsed = now
                .duration_since(self.last_request_time)
                .expect("Bug: time went backwards");
            let interval =
                std::time::Duration::from_secs(1) / self.limit as u32;
            if elapsed < interval {
                std::thread::sleep(interval - elapsed);
            }
            self.last_request_time = std::time::SystemTime::now();
            let r = f();
            i += 1;
            match max_attemps {
                Some(max_attemps) if i >= max_attemps => return r,
                _ => (),
            };
            match r {
                Err(ethers::etherscan::errors::EtherscanError::RateLimitExceeded) => {
                    self.last_request_time = std::time::SystemTime::now();
                }
                _ => return r
            };
        }
    }

    pub fn call<T>(
        &mut self,
        f: impl Fn() -> Result<T, ethers::etherscan::errors::EtherscanError>,
    ) -> Result<T, ethers::etherscan::errors::EtherscanError> {
        self._call(f, None)
    }

    pub fn call_with_max_attemps<T>(
        &mut self,
        f: impl Fn() -> Result<T, ethers::etherscan::errors::EtherscanError>,
        max_attemps: usize,
    ) -> Result<T, ethers::etherscan::errors::EtherscanError> {
        self._call(f, Some(max_attemps))
    }
}

#[cfg(test)]
mod tests_with_dep {
    use ethers::types::Chain;

    use crate::utils::addresses::ADDRESS_BOOK;

    #[test]
    fn test_control_request_rate() {
        let etherscan = super::EtherscanClient::default();
        let mut rate_limit_controller = super::RateLimitController {
            limit: 5,
            ..Default::default()
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let f = || {
            runtime.block_on(etherscan.contract_abi(
                ADDRESS_BOOK.weth.on_chain(Chain::Mainnet).unwrap().into(),
            ))
        };
        for _ in 0..5 {
            let r = rate_limit_controller.call(f);
            assert!(!matches!(
                r,
                Err(ethers::etherscan::errors::EtherscanError::RateLimitExceeded)
            ));
        }
    }
}
