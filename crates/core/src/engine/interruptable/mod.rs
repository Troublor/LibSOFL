// This module reinvents the EVM and support interrupting the execution of the EVM, and resume the execution of the EVM later.
// This implementation is based on the fragile branch `reth_freeze`, subject to substantial change.
pub mod breakpoint;
pub mod differential_testing;
pub mod evm;
pub mod execution;
pub mod serde_helpers;
mod tests;

/// EVM call stack limit.
pub const CALL_STACK_LIMIT: u64 = 1024;
