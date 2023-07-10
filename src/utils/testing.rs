use std::path::PathBuf;

use tracing::info;

use crate::error::SoflError;

/// Compile a solidity snippet.
/// This method uses svm-rs internally to download solc binary.
/// The default solc version is 0.8.12.
pub fn compile_solidity_snippet(
    snippet: &str,
    solc_version: Option<&semver::Version>,
) -> Result<ethers_solc::ProjectCompileOutput, SoflError> {
    let default_version = semver::Version::new(0, 8, 12);
    let solc_version = solc_version.unwrap_or(&default_version);
    let solc = ethers_solc::Solc::find_or_install_svm_version(
        solc_version.to_string().as_str(),
    )
    .map_err(SoflError::Solc)?;
    let source = ethers_solc::artifacts::Source::new(snippet);
    let sources = ethers_solc::artifacts::Sources::from([(
        PathBuf::from("test.sol"),
        source,
    )]);
    let project = ethers_solc::Project::builder()
        .build()
        .map_err(SoflError::Solc)?;
    project
        .compile_with_version(&solc, sources)
        .map_err(SoflError::Solc)
}

#[cfg(not(feature = "test-use-jsonrpc"))]
pub fn get_testing_bc_provider(
) -> crate::engine::providers::reth::RethBcProvider {
    let db_provider = crate::engine::providers::BcProviderBuilder::default_db();
    if let Ok(provider) = db_provider {
        info!("Using reth database provider.");
        provider
    } else {
        panic!("No reth database provider is set in SoflConfig or the database does not exist.")
    }
}

#[cfg(feature = "test-use-jsonrpc")]
pub fn get_testing_bc_provider(
) -> crate::engine::providers::rpc::JsonRpcBcProvider<ethers_providers::Http> {
    let db_provider =
        crate::engine::providers::BcProviderBuilder::default_jsonrpc();
    if let Ok(provider) = db_provider {
        info!("Using jsonrpc provider.");
        provider
    } else {
        panic!("No jsonrpc endpoint is set in SoflConfig or the endpoint is not valid.")
    }
}
