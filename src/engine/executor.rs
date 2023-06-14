use reth_interfaces::Error;
use reth_primitives::{Receipt, TransactionSigned};
use revm_primitives::{Env, ExecutionResult};

pub enum ExecutorError {
    InvalidTransactionError,
    RethError(Error),
}

pub trait Executor {
    /// Simulate the execution of a transaction in the current evm environment.
    /// The blockchain state will not be changed.
    fn simulate(&self, tx: TransactionSigned) -> Result<(ExecutionResult, Receipt), ExecutorError>;

    /// Execute a transaction in the current evm environment.
    /// The blockchain state will be changed.
    fn execute(&self, tx: TransactionSigned) -> Result<(ExecutionResult, Receipt), ExecutorError>;

    /// Get the current evm environment, i.e., environment of next transaction.
    fn env(&self) -> Env;

    /// Commit block.
    /// This will update the evm environment.
    fn commit_block(&self);
}
