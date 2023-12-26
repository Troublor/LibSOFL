#[cfg(not(feature = "test-using-jsonrpc"))]
use libsofl_reth::blockchain::provider::RethProvider;
#[cfg(not(feature = "test-using-jsonrpc"))]
pub fn get_test_bc_provider() -> RethProvider {
    use libsofl_reth::config::RethConfig;
    use libsofl_utils::config::Config;

    RethConfig::must_load().bc_provider().unwrap()
}

#[cfg(feature = "test-using-jsonrpc")]
use libsofl_jsonrpc::provider::JsonRpcProvider;
#[cfg(feature = "test-using-jsonrpc")]
pub fn get_test_bc_provider() -> JsonRpcProvider {
    use libsofl_jsonrpc::config::JsonRpcConfig;
    use libsofl_utils::config::Config;

    JsonRpcConfig::must_load().bc_provider().unwrap()
}
