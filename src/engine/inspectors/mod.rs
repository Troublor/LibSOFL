pub mod asset_flow;
pub mod call_extract;
pub mod combined;
pub mod static_call;

use auto_impl::auto_impl;
use reth::primitives::bytes::Bytes;
use reth::primitives::U256;
use reth::revm::interpreter::{
    CallInputs, CreateInputs, Gas, InstructionResult, Interpreter,
};
use reth_revm_inspectors::tracing::TracingInspector;
use revm::{inspectors::NoOpInspector, Database, EVMData, Inspector};
use revm_primitives::{ExecutionResult, TxEnv, B160, B256};

/// NoInspector is used as a placeholder for type parameters when no inspector is needed.
pub type NoInspector = NoOpInspector;

pub static mut NO_INSPECTOR: NoInspector = NoOpInspector {};

pub fn no_inspector() -> &'static mut NoInspector {
    // unsafe is ok here since NoInspector is essential a no-op inspector
    unsafe { &mut NO_INSPECTOR }
}

pub fn gen_no_inspector() -> NoInspector {
    NoOpInspector {}
}

pub trait InspectorWithTxHook<BS: Database>:
    Inspector<BS> + TxHook<BS>
{
}

impl<BS: Database, T: Inspector<BS> + TxHook<BS>> InspectorWithTxHook<BS>
    for T
{
}

pub struct InspectorTuple<BS, L, R>
where
    BS: Database,
    L: InspectorWithTxHook<BS>,
    R: InspectorWithTxHook<BS>,
{
    pub left: L,
    pub right: R,
    _phantom: std::marker::PhantomData<BS>,
}

impl<BS, I1, I2> From<(I1, I2)> for InspectorTuple<BS, I1, I2>
where
    BS: Database,
    I1: InspectorWithTxHook<BS>,
    I2: InspectorWithTxHook<BS>,
{
    fn from(value: (I1, I2)) -> Self {
        Self {
            left: value.0,
            right: value.1,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<BS: Database, I1: InspectorWithTxHook<BS>> From<I1>
    for InspectorTuple<BS, I1, NoInspector>
{
    fn from(value: I1) -> Self {
        Self {
            left: value,
            right: NoInspector {},
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<
        BS: Database,
        I1: InspectorWithTxHook<BS>,
        I2: InspectorWithTxHook<BS>,
    > From<InspectorTuple<BS, I1, I2>> for (I1, I2)
{
    fn from(value: InspectorTuple<BS, I1, I2>) -> Self {
        (value.left, value.right)
    }
}
impl<BS, L> InspectorTuple<BS, L, NoInspector>
where
    BS: Database,
    L: InspectorWithTxHook<BS>,
{
    pub fn singleton(left: L) -> Self {
        Self {
            left,
            right: NoInspector {},
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<BS, L, R> InspectorTuple<BS, L, R>
where
    BS: Database,
    L: InspectorWithTxHook<BS>,
    R: InspectorWithTxHook<BS>,
{
    pub fn new(left: L, right: R) -> Self {
        Self {
            left,
            right,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn append(
        self,
        inspector: impl InspectorWithTxHook<BS>,
    ) -> InspectorTuple<BS, Self, impl InspectorWithTxHook<BS>> {
        InspectorTuple::new(self, inspector)
    }
}

impl<BS, L, R> Inspector<BS> for InspectorTuple<BS, L, R>
where
    BS: Database,
    L: InspectorWithTxHook<BS>,
    R: InspectorWithTxHook<BS>,
{
    fn initialize_interp(
        &mut self,
        _interp: &mut Interpreter,
        _data: &mut EVMData<'_, BS>,
    ) -> InstructionResult {
        let r = self.left.initialize_interp(_interp, _data);
        if r != InstructionResult::Continue {
            return r;
        }
        self.right.initialize_interp(_interp, _data)
    }

    fn step(
        &mut self,
        _interp: &mut Interpreter,
        _data: &mut EVMData<'_, BS>,
    ) -> InstructionResult {
        self.left.step(_interp, _data);
        self.right.step(_interp, _data)
    }

    fn log(
        &mut self,
        _evm_data: &mut EVMData<'_, BS>,
        _address: &B160,
        _topics: &[B256],
        _data: &Bytes,
    ) {
        self.left.log(_evm_data, _address, _topics, _data);
        self.right.log(_evm_data, _address, _topics, _data);
    }

    fn step_end(
        &mut self,
        _interp: &mut Interpreter,
        _data: &mut EVMData<'_, BS>,
        _eval: InstructionResult,
    ) -> InstructionResult {
        let r = self.left.step_end(_interp, _data, _eval);
        if r != InstructionResult::Continue {
            return r;
        }
        self.right.step_end(_interp, _data, _eval)
    }

    fn call(
        &mut self,
        _data: &mut EVMData<'_, BS>,
        _inputs: &mut CallInputs,
    ) -> (InstructionResult, Gas, Bytes) {
        let (left_result, left_gas, left_out) = self.left.call(_data, _inputs);
        if left_result != InstructionResult::Continue {
            return (left_result, left_gas, left_out);
        }
        self.right.call(_data, _inputs)
    }

    fn call_end(
        &mut self,
        _data: &mut EVMData<'_, BS>,
        _inputs: &CallInputs,
        remaining_gas: Gas,
        ret: InstructionResult,
        out: Bytes,
    ) -> (InstructionResult, Gas, Bytes) {
        self.left
            .call_end(_data, _inputs, remaining_gas, ret, out.clone());
        self.right.call_end(_data, _inputs, remaining_gas, ret, out)
    }

    fn create(
        &mut self,
        _data: &mut EVMData<'_, BS>,
        _inputs: &mut CreateInputs,
    ) -> (InstructionResult, Option<B160>, Gas, Bytes) {
        let (left_result, left_address, left_gas, left_out) =
            self.left.create(_data, _inputs);
        if left_result != InstructionResult::Continue {
            return (left_result, left_address, left_gas, left_out);
        }
        self.right.create(_data, _inputs)
    }

    fn create_end(
        &mut self,
        _data: &mut EVMData<'_, BS>,
        _inputs: &CreateInputs,
        ret: InstructionResult,
        address: Option<B160>,
        remaining_gas: Gas,
        out: Bytes,
    ) -> (InstructionResult, Option<B160>, Gas, Bytes) {
        self.left.create_end(
            _data,
            _inputs,
            ret,
            address,
            remaining_gas,
            out.clone(),
        );
        self.right
            .create_end(_data, _inputs, ret, address, remaining_gas, out)
    }

    fn selfdestruct(&mut self, _contract: B160, _target: B160, _value: U256) {
        self.left.selfdestruct(_contract, _target, _value);
        self.right.selfdestruct(_contract, _target, _value);
    }
}

impl<BS, L, R> TxHook<BS> for InspectorTuple<BS, L, R>
where
    BS: Database,
    L: InspectorWithTxHook<BS>,
    R: InspectorWithTxHook<BS>,
{
    #[doc = " Called before the transaction is executed."]
    #[doc = " Return false to skip the transaction."]
    fn transaction(&mut self, _tx: &TxEnv, _state: &BS) -> bool {
        let r = self.left.transaction(_tx, _state);
        if !r {
            return r;
        }
        self.right.transaction(_tx, _state)
    }

    #[doc = " Called after the transaction is executed."]
    fn transaction_end(
        &mut self,
        _tx: &TxEnv,
        _state: &BS,
        _result: &ExecutionResult,
    ) {
        self.left.transaction_end(_tx, _state, _result);
        self.right.transaction_end(_tx, _state, _result);
    }
}

/// Inspector that can be used to inspect the execution of a sequence of transactions.
#[auto_impl(& mut, Box)]
pub trait TxHook<BS: Database> {
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
impl<BS: Database> TxHook<BS> for () {}

impl<BS: Database, Head: TxHook<BS>, Tail: TxHook<BS>> TxHook<BS>
    for (Head, Tail)
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

impl<BS: Database> TxHook<BS> for NoOpInspector {}

impl<BS: Database> TxHook<BS> for TracingInspector {}
