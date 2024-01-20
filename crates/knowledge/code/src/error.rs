use foundry_block_explorers::errors::EtherscanError;
use foundry_compilers::error::SolcError;

#[derive(Debug)]
pub enum Error {
    Etherscan(EtherscanError),
    Solc(SolcError),
    VyperNotSupported,
    CompilationFailed(Vec<foundry_compilers::artifacts::Error>),
}
