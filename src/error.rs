use std::convert::Infallible;

use ethers::etherscan;
use reth_primitives::TxHash;

use crate::engine::transactions::position::TxPosition;

#[derive(Debug, thiserror::Error)]
pub enum SoflError<DBERR = Infallible> {
    /// Custom error
    #[error("custom error: {0:?}")]
    Custom(String),

    /// Wrapper of abi encoding and decoding error
    #[error("abi error: {0}")]
    Abi(
        #[from]
        #[source]
        ethers::abi::Error,
    ),

    /// Wrapper of abi parsing error
    #[error("abi error: {0}")]
    AbiParsing(
        #[from]
        #[source]
        ethers::abi::ParseError,
    ),

    /// Wrapper of reth error
    #[error("reth error: {0}")]
    Reth(
        #[from]
        #[source]
        reth_interfaces::RethError,
    ),

    /// Wrapper of BcState::DbErr
    #[error("database error: {0:?}")]
    Db(DBERR),

    /// Wrapper of Execution Result
    #[error("execution result error: {0:?}")]
    Exec(revm_primitives::ExecutionResult),

    /// Fork position not found
    #[error("fork position ({0}) not found")]
    PosNotFound(TxPosition),

    #[error("fork before tx ({0}) not found")]
    TxNotFound(TxHash),

    /// Wrapper of EVM error
    #[error("EVM error: {0:?}")]
    Evm(
        #[from]
        #[source]
        revm_primitives::EVMError<DBERR>,
    ),

    /// Wrapper of SolcVM error
    #[error("SolcVM error: {0}")]
    SolcVM(
        #[from]
        #[source]
        svm_lib::SolcVmError,
    ),

    /// Wrapper of ethers Solc error
    #[error("ethers Solc error: {0}")]
    Solc(
        #[from]
        #[source]
        ethers_solc::error::SolcError,
    ),

    #[error("etherscan error: {0}")]
    Etherscan(
        #[from]
        #[source]
        etherscan::errors::EtherscanError,
    ),
}
