use std::ops::Range;

use super::{
    state::BcState,
    types::{
        Address, CallInputs, CreateInputs, Database, EvmContext,
        ExecutionResult, Inspector, Interpreter, TxEnv, U256,
    },
};
use alloy_primitives::Log;
use auto_impl::auto_impl;
use revm::interpreter::{CallOutcome, CreateOutcome};

/// EvmInspector is an extended revm::Inspector with additional methods called at each transaction start and end.
#[auto_impl(&mut, Box)]
pub trait EvmInspector<BS: BcState>: revm::Inspector<BS> {
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

// /// Any inspector that implements `revm::Inspector` can be used as `EvmInspector`.
// impl<I: revm::Inspector<BS>, BS: BcState> EvmInspector<BS> for I {}

#[derive(
    Default,
    Clone,
    PartialEq,
    Eq,
    Copy,
    Debug,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct NoInspector;

impl<BS: BcState> Inspector<BS> for NoInspector {}

impl<BS: BcState> EvmInspector<BS> for NoInspector {}

pub static mut NO_INSPECTOR: NoInspector = NoInspector {};

pub fn no_inspector() -> &'static mut NoInspector {
    unsafe { &mut NO_INSPECTOR }
}
#[derive(
    derive_more::AsRef,
    derive_more::AsMut,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::From,
)]
pub struct CombinedInspector<'a, BS> {
    #[as_ref]
    #[as_mut]
    #[deref]
    #[deref_mut]
    #[from(forward)]
    pub inspectors: Vec<Box<dyn EvmInspector<BS> + 'a>>,
}

impl<'a, BS> Default for CombinedInspector<'a, BS> {
    fn default() -> Self {
        Self { inspectors: vec![] }
    }
}

impl<'a, BS: BcState> CombinedInspector<'a, BS> {
    pub fn add(&mut self, inspector: impl EvmInspector<BS> + 'a) {
        let boxed: Box<dyn EvmInspector<BS> + 'a> = Box::new(inspector);
        self.inspectors.push(boxed);
    }
}

impl<'a, DB: Database> Inspector<DB> for CombinedInspector<'a, DB> {
    #[doc = r" Called before the interpreter is initialized."]
    #[doc = r""]
    #[doc = r" If `interp.instruction_result` is set to anything other than [InstructionResult::Continue] then the execution of the interpreter"]
    #[doc = r" is skipped."]
    #[inline]
    fn initialize_interp(
        &mut self,
        interp: &mut Interpreter,
        data: &mut EvmContext<DB>,
    ) {
        for i in self.inspectors.iter_mut() {
            i.initialize_interp(interp, data);
        }
    }

    #[doc = r" Called on each step of the interpreter."]
    #[doc = r""]
    #[doc = r" Information about the current execution, including the memory, stack and more is available"]
    #[doc = r" on `interp` (see [Interpreter])."]
    #[doc = r""]
    #[doc = r" # Example"]
    #[doc = r""]
    #[doc = r" To get the current opcode, use `interp.current_opcode()`."]
    #[inline]
    fn step(&mut self, interp: &mut Interpreter, data: &mut EvmContext<DB>) {
        for i in self.inspectors.iter_mut() {
            i.step(interp, data);
        }
    }

    #[doc = r" Called when a log is emitted."]
    #[inline]
    fn log(&mut self, context: &mut EvmContext<DB>, log: &Log) {
        self.inspectors.iter_mut().for_each(|i| {
            i.log(context, log);
        });
    }

    #[doc = r" Called after `step` when the instruction has been executed."]
    #[doc = r""]
    #[doc = r" Setting `interp.instruction_result` to anything other than [InstructionResult::Continue] alters the execution"]
    #[doc = r" of the interpreter."]
    #[inline]
    fn step_end(
        &mut self,
        interp: &mut Interpreter,
        data: &mut EvmContext<DB>,
    ) {
        self.inspectors.iter_mut().for_each(|i| {
            i.step_end(interp, data);
        });
    }

    #[doc = r" Called whenever a call to a contract is about to start."]
    #[doc = r""]
    #[doc = r" InstructionResulting anything other than [InstructionResult::Continue] overrides the result of the call."]
    #[inline]
    /// Inspectors are called in the order they are added.
    /// If any inspector returns a non-Continue result, the other inspectors are skipped.
    fn call(
        &mut self,
        data: &mut EvmContext<DB>,
        inputs: &mut CallInputs,
        return_memory_offset: Range<usize>,
    ) -> Option<CallOutcome> {
        for i in self.inspectors.iter_mut() {
            let outcome = i.call(data, inputs, return_memory_offset.clone());
            if let Some(outcome) = outcome {
                return Some(outcome);
            }
        }
        None
    }

    fn call_end(
        &mut self,
        data: &mut EvmContext<DB>,
        inputs: &CallInputs,
        outcome: CallOutcome,
    ) -> CallOutcome {
        let mut r = outcome;
        for i in self.inspectors.iter_mut() {
            r = i.call_end(data, inputs, r);
        }
        r
    }

    fn create(
        &mut self,
        data: &mut EvmContext<DB>,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        for i in self.inspectors.iter_mut() {
            let outcome = i.create(data, inputs);
            if outcome.is_some() {
                return outcome;
            }
        }
        None
    }

    fn create_end(
        &mut self,
        data: &mut EvmContext<DB>,
        inputs: &CreateInputs,
        outcome: CreateOutcome,
    ) -> CreateOutcome {
        let mut r = outcome;
        for i in self.inspectors.iter_mut() {
            let outcome = i.create_end(data, inputs, r);
            r = outcome;
        }
        r
    }

    #[doc = r" Called when a contract has been self-destructed with funds transferred to target."]
    #[inline]
    fn selfdestruct(
        &mut self,
        contract: Address,
        target: Address,
        value: U256,
    ) {
        self.inspectors.iter_mut().for_each(|i| {
            i.selfdestruct(contract, target, value);
        });
    }
}

impl<'a, BS: BcState> EvmInspector<BS> for CombinedInspector<'a, BS> {
    /// Inspectors are called in the order they are added.
    /// If any inspector returns false, the other inspectors are skipped.
    fn transaction(
        &mut self,
        _tx: &revm::primitives::TxEnv,
        _state: &BS,
    ) -> bool {
        for i in self.inspectors.iter_mut() {
            if !i.transaction(_tx, _state) {
                return false;
            }
        }
        true
    }

    fn transaction_end(
        &mut self,
        _tx: &revm::primitives::TxEnv,
        _state: &BS,
        _result: &revm::primitives::ExecutionResult,
    ) {
        self.inspectors.iter_mut().for_each(|i| {
            i.transaction_end(_tx, _state, _result);
        });
    }
}
