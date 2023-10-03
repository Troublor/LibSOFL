use revm_primitives::{BlockEnv, CfgEnv, TxEnv};

pub mod mono_seq;
pub mod raw_tx;
pub mod seq;
pub mod state_tx;
pub mod structured_tx;

/// deprecated
pub mod tx;

/// Generic transaction trait declaration, used for fuzzing, generic for any EVM-compatible blockchains.
pub trait FuzzTx {
    /// Returns the revm env of the transaction.
    fn tx_spec(&self) -> TxEnv;
}

pub trait FuzzBlock {
    /// Returns the revm env of the block, in which the underlying transaction executes.
    fn block_spec(&self) -> BlockEnv;
}

pub trait FuzzEvm {
    /// Returns the revm env of the block, in which the underlying transaction executes.
    fn evm_spec(&self) -> CfgEnv;
}
