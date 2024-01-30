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
