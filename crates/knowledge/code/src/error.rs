use std::fmt::Display;

use foundry_block_explorers::errors::EtherscanError;
use foundry_compilers::error::SolcError;
use sea_orm::DbErr;

#[derive(Debug)]
pub enum Error {
    Etherscan(EtherscanError),
    Solc(SolcError),
    VyperNotSupported,
    SolidityVersionTooLow,
    CompilationFailed(Vec<foundry_compilers::artifacts::Error>),
    Database(DbErr),
    Sofl(libsofl_core::error::SoflError),
}

impl From<Error> for jsonrpsee::types::ErrorObject<'static> {
    fn from(value: Error) -> Self {
        jsonrpsee::types::ErrorObject::owned(
            jsonrpsee::types::error::INTERNAL_ERROR_CODE,
            jsonrpsee::types::error::INTERNAL_ERROR_MSG,
            Some(format!("{:?}", value)),
        )
    }
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Etherscan(err) => write!(f, "Etherscan error: {}", err),
            Error::Solc(err) => write!(f, "Solc error: {}", err),
            Error::VyperNotSupported => write!(f, "Vyper not supported"),
            Error::SolidityVersionTooLow => {
                write!(f, "Solidity version too low")
            }
            Error::CompilationFailed(errors) => {
                write!(f, "Compilation failed: {:?}", errors)
            }
            Error::Database(err) => write!(f, "Database error: {}", err),
            Error::Sofl(err) => write!(f, "Sofl error: {}", err),
        }
    }
}
