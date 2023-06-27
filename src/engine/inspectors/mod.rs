pub mod combined;

use revm::{interpreter::InstructionResult, Inspector};
use revm_primitives::ExecutionResult;

use crate::engine::state::BcState;

use super::transactions::TxOrPseudo;

/// NoInspector is used as a placeholder for type parameters when no inspector is needed.
pub type NoInspector = ();

pub static mut NO_INSPECTOR: NoInspector = ();

pub fn no_inspector() -> &'static mut NoInspector {
    // unsafe is ok here since NoInspector is essential a no-op inspector
    unsafe { &mut NO_INSPECTOR }
}
/// Inspector that can be used to inspect the execution of a sequence of transactions.
pub trait MultiTxInspector<BS: BcState>: Inspector<BS> {
    /// Called before the transaction is executed.
    /// Return anything other than `InstructionResult::Continue` to skip the transaction.
    fn transaction(
        &mut self,
        _tx: &TxOrPseudo<'_, BS>,
        _state: &mut BS,
    ) -> InstructionResult {
        InstructionResult::Continue
    }

    /// Called after the transaction is executed.
    fn transaction_end(
        &mut self,
        _tx: &TxOrPseudo<'_, BS>,
        _state: &mut BS,
        _result: &ExecutionResult,
    ) {
    }
}

/// Automatically implement `MultiTxInspector` for any `Inspector`.
impl<BS: BcState, I: Inspector<BS>> MultiTxInspector<BS> for I {}
