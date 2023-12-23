use super::state::BcState;

/// EvmInspector is an extended revm::Inspector with additional methods called at each transaction start and end.
pub trait EvmInspector<BS: BcState>: revm::Inspector<BS> {
    /// Called before the transaction is executed.
    /// Return false to skip the transaction.
    fn transaction(
        &mut self,
        _tx: &revm::primitives::TxEnv,
        _state: &BS,
    ) -> bool {
        true
    }

    /// Called after the transaction is executed.
    fn transaction_end(
        &mut self,
        _tx: &revm::primitives::TxEnv,
        _state: &BS,
        _result: &revm::primitives::ExecutionResult,
    ) {
    }
}

// /// Any inspector that implements `revm::Inspector` can be used as `EvmInspector`.
// impl<I: revm::Inspector<BS>, BS: BcState> EvmInspector<BS> for I {}

pub type NoInspector = revm::inspectors::NoOpInspector;

impl<BS: BcState> EvmInspector<BS> for NoInspector {}

pub static mut NO_INSPECTOR: NoInspector = NoInspector {};

pub fn no_inspector() -> &'static mut NoInspector {
    unsafe { &mut NO_INSPECTOR }
}
