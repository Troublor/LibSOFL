use crate::engine::types::ExecutionResult;

pub type Result<T, E = SoflError> = std::result::Result<T, E>;

#[derive(Debug, derive_more::Display, thiserror::Error)]
pub enum SoflError {
    #[display(fmt = "Err not found: {}", _0)]
    NotFound(String),

    #[display(fmt = "Err unsupported: {}", _0)]
    Unsupported(String),

    #[display(fmt = "Err blockchain provider failure: {}", _0)]
    Provider(String),

    #[display(fmt = "Err invalid blockchain state: {}", _0)]
    BcState(String),

    #[display(fmt = "Err invalid transaction: {:?}", _0)]
    InvalidTransaction(revm::primitives::InvalidTransaction),

    #[display(fmt = "Err invalid header: {:?}", _0)]
    InvalidHeader(revm::primitives::InvalidHeader),

    #[display(fmt = "Err invalid config: {}", _0)]
    Config(String),

    #[display(fmt = "Err execution failed: {:?}", _0)]
    Exec(ExecutionResult),

    #[display(fmt = "Err invalid abi encoding/decoding: {}", _0)]
    Abi(String),

    #[display(fmt = "Execution interrupted")]
    Interrupted,

    #[display(fmt = "Err: {}", _0)]
    Custom(String),
}
