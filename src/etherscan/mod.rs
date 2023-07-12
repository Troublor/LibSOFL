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

    last_request_time: std::time::SystemTime,
}

impl Default for EtherscanClient {
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
            client,
            last_request_time: std::time::UNIX_EPOCH,
        }
    }
}
