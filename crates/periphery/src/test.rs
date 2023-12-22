use libsofl_reth::blockchain::provider::RethProvider;

#[cfg(not(feature = "test-using-jsonrpc"))]
pub fn get_test_bc_provider() -> RethProvider {
    use libsofl_reth::config::RethConfig;

    RethConfig::must_load().bc_provider().unwrap()
}
