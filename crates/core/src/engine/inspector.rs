use auto_impl::auto_impl;

use super::{
    state::BcState,
    types::{
        Address, Bytes, CallInputs, CreateInputs, Database, EVMData,
        ExecutionResult, Gas, Inspector, InstructionResult, Interpreter, TxEnv,
        B256, U256,
    },
};

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

pub type NoInspector = revm::inspectors::NoOpInspector;

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
        interp: &mut Interpreter<'_>,
        data: &mut EVMData<'_, DB>,
    ) {
        self.inspectors.iter_mut().for_each(|i| {
            i.initialize_interp(interp, data);
        });
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
    fn step(
        &mut self,
        interp: &mut Interpreter<'_>,
        data: &mut EVMData<'_, DB>,
    ) {
        self.inspectors.iter_mut().for_each(|i| {
            i.step(interp, data);
        });
    }

    #[doc = r" Called when a log is emitted."]
    #[inline]
    fn log(
        &mut self,
        evm_data: &mut EVMData<'_, DB>,
        address: &Address,
        topics: &[B256],
        data: &Bytes,
    ) {
        self.inspectors.iter_mut().for_each(|i| {
            i.log(evm_data, address, topics, data);
        });
    }

    #[doc = r" Called after `step` when the instruction has been executed."]
    #[doc = r""]
    #[doc = r" Setting `interp.instruction_result` to anything other than [InstructionResult::Continue] alters the execution"]
    #[doc = r" of the interpreter."]
    #[inline]
    fn step_end(
        &mut self,
        interp: &mut Interpreter<'_>,
        data: &mut EVMData<'_, DB>,
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
        data: &mut EVMData<'_, DB>,
        inputs: &mut CallInputs,
    ) -> (InstructionResult, Gas, Bytes) {
        for i in self.inspectors.iter_mut() {
            let (ret, gas, out) = i.call(data, inputs);
            if ret != InstructionResult::Continue {
                return (ret, gas, out);
            }
        }
        (InstructionResult::Continue, Gas::new(0), Bytes::new())
    }

    #[doc = r" Called when a call to a contract has concluded."]
    #[doc = r""]
    #[doc = r" InstructionResulting anything other than the values passed to this function (`(ret, remaining_gas,"]
    #[doc = r" out)`) will alter the result of the call."]
    #[inline]
    /// Inspectors are called in the order they are added.
    /// If any inspector returns a different result other than the value passed to this function, the other inspectors are skipped.
    fn call_end(
        &mut self,
        data: &mut EVMData<'_, DB>,
        inputs: &CallInputs,
        remaining_gas: Gas,
        ret: InstructionResult,
        out: Bytes,
    ) -> (InstructionResult, Gas, Bytes) {
        for i in self.inspectors.iter_mut() {
            let (r, g, o) =
                i.call_end(data, inputs, remaining_gas, ret, out.clone());
            if r != ret || g != remaining_gas || o != out {
                return (r, g, out);
            }
        }
        (ret, remaining_gas, out)
    }

    #[doc = r" Called when a contract is about to be created."]
    #[doc = r""]
    #[doc = r" InstructionResulting anything other than [InstructionResult::Continue] overrides the result of the creation."]
    #[inline]
    /// Inspectors are called in the order they are added.
    /// If any inspector returns a non-Continue result, the other inspectors are skipped.
    fn create(
        &mut self,
        data: &mut EVMData<'_, DB>,
        inputs: &mut CreateInputs,
    ) -> (InstructionResult, Option<Address>, Gas, Bytes) {
        for i in self.inspectors.iter_mut() {
            let (ret, address, gas, out) = i.create(data, inputs);
            if ret != InstructionResult::Continue {
                return (ret, address, gas, out);
            }
        }
        (
            InstructionResult::Continue,
            None,
            Gas::new(0),
            Bytes::default(),
        )
    }

    #[doc = r" Called when a contract has been created."]
    #[doc = r""]
    #[doc = r" InstructionResulting anything other than the values passed to this function (`(ret, remaining_gas,"]
    #[doc = r" address, out)`) will alter the result of the create."]
    #[inline]
    fn create_end(
        &mut self,
        data: &mut EVMData<'_, DB>,
        inputs: &CreateInputs,
        ret: InstructionResult,
        address: Option<Address>,
        remaining_gas: Gas,
        out: Bytes,
    ) -> (InstructionResult, Option<Address>, Gas, Bytes) {
        for i in self.inspectors.iter_mut() {
            let (r, addr, g, o) = i.create_end(
                data,
                inputs,
                ret,
                address,
                remaining_gas,
                out.clone(),
            );
            if r != ret || addr != address || g != remaining_gas || o != out {
                return (r, addr, g, o);
            }
        }
        (ret, address, remaining_gas, out)
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
