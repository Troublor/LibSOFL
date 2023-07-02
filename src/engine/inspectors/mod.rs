pub mod asset_flow;
pub mod combined;
pub mod static_call;

use auto_impl::auto_impl;
use reth_revm_inspectors::tracing::TracingInspector;
use revm::{inspectors::NoOpInspector, Database, Inspector};
use revm_primitives::{ExecutionResult, TxEnv};

/// NoInspector is used as a placeholder for type parameters when no inspector is needed.
pub type NoInspector = ();

pub static mut NO_INSPECTOR: NoInspector = ();

pub fn no_inspector() -> &'static mut NoInspector {
    // unsafe is ok here since NoInspector is essential a no-op inspector
    unsafe { &mut NO_INSPECTOR }
}
/// Inspector that can be used to inspect the execution of a sequence of transactions.
#[auto_impl(&mut, Box)]
pub trait MultiTxInspector<BS: Database>: Inspector<BS> {
    /// Called before the transaction is executed.
    /// Return false to skip the transaction.
    fn transaction(&mut self, _tx: &TxEnv, _state: &BS) -> bool {
        true
    }

    /// Called after the transaction is executed.
    fn transaction_end(
        &mut self,
        _tx: &TxEnv,
        _state: &BS,
        _result: &ExecutionResult,
    ) {
    }
}

// /// Automatically implement `MultiTxInspector` for any `Inspector`.
// impl<BS: Database, I: Inspector<BS>> MultiTxInspector<BS> for I {}
impl<BS: Database> MultiTxInspector<BS> for () {}
impl<BS: Database, Head: MultiTxInspector<BS>, Tail: MultiTxInspector<BS>>
    MultiTxInspector<BS> for (Head, Tail)
{
    fn transaction(&mut self, _tx: &TxEnv, _state: &BS) -> bool {
        let r = self.0.transaction(_tx, _state);
        if !r {
            return r;
        }
        self.1.transaction(_tx, _state)
    }

    fn transaction_end(
        &mut self,
        _tx: &TxEnv,
        _state: &BS,
        _result: &ExecutionResult,
    ) {
        self.0.transaction_end(_tx, _state, _result);
        self.1.transaction_end(_tx, _state, _result);
    }
}
impl<BS: Database> MultiTxInspector<BS> for NoOpInspector {}
impl<BS: Database> MultiTxInspector<BS> for TracingInspector {}
