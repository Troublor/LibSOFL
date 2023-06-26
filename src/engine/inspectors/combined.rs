use revm::{
    interpreter::{
        CallInputs, CreateInputs, Gas, InstructionResult, Interpreter,
    },
    EVMData, Inspector,
};
use revm_primitives::{Bytes, B160, B256};

use crate::engine::state::BcState;

pub struct CombinedInspector<BS: BcState> {
    inspectors: Vec<Box<dyn Inspector<BS>>>,
}

impl<BS: BcState> Inspector<BS> for CombinedInspector<BS> {
    #[doc = " Called Before the interpreter is initialized."]
    #[doc = ""]
    #[doc = " If anything other than [InstructionResult::Continue] is returned then execution of the interpreter is"]
    #[doc = " skipped."]
    fn initialize_interp(
        &mut self,
        _interp: &mut Interpreter,
        _data: &mut EVMData<'_, BS>,
        _is_static: bool,
    ) -> InstructionResult {
        self.inspectors
            .iter_mut()
            .map(|insp| insp.initialize_interp(_interp, _data, _is_static))
            .filter(|res| *res != InstructionResult::Continue)
            .collect::<Vec<InstructionResult>>() // collect is necessary here since we need to ensure all inspectors are called.
            .pop()
            .unwrap_or(InstructionResult::Continue)
    }

    #[doc = " Called on each step of the interpreter."]
    #[doc = ""]
    #[doc = " Information about the current execution, including the memory, stack and more is available"]
    #[doc = " on `interp` (see [Interpreter])."]
    #[doc = ""]
    #[doc = " # Example"]
    #[doc = ""]
    #[doc = " To get the current opcode, use `interp.current_opcode()`."]
    fn step(
        &mut self,
        _interp: &mut Interpreter,
        _data: &mut EVMData<'_, BS>,
        _is_static: bool,
    ) -> InstructionResult {
        self.inspectors
            .iter_mut()
            .map(|insp| insp.step(_interp, _data, _is_static))
            .filter(|res| *res != InstructionResult::Continue)
            .collect::<Vec<InstructionResult>>() // collect is necessary here since we need to ensure all inspectors are called.
            .pop()
            .unwrap_or(InstructionResult::Continue)
    }

    #[doc = " Called when a log is emitted."]
    fn log(
        &mut self,
        _evm_data: &mut EVMData<'_, BS>,
        _address: &B160,
        _topics: &[B256],
        _data: &Bytes,
    ) {
        self.inspectors
            .iter_mut()
            .for_each(|insp| insp.log(_evm_data, _address, _topics, _data))
    }

    #[doc = " Called after `step` when the instruction has been executed."]
    #[doc = ""]
    #[doc = " InstructionResulting anything other than [InstructionResult::Continue] alters the execution of the interpreter."]
    fn step_end(
        &mut self,
        _interp: &mut Interpreter,
        _data: &mut EVMData<'_, BS>,
        _is_static: bool,
        _eval: InstructionResult,
    ) -> InstructionResult {
        self.inspectors
            .iter_mut()
            .map(|insp| insp.step_end(_interp, _data, _is_static, _eval))
            .filter(|res| *res != InstructionResult::Continue)
            .collect::<Vec<InstructionResult>>() // collect is necessary here since we need to ensure all inspectors are called.
            .pop()
            .unwrap_or(InstructionResult::Continue)
    }

    #[doc = " Called whenever a call to a contract is about to start."]
    #[doc = ""]
    #[doc = " InstructionResulting anything other than [InstructionResult::Continue] overrides the result of the call."]
    fn call(
        &mut self,
        _data: &mut EVMData<'_, BS>,
        _inputs: &mut CallInputs,
        _is_static: bool,
    ) -> (InstructionResult, Gas, Bytes) {
        self.inspectors
            .iter_mut()
            .map(|insp| insp.call(_data, _inputs, _is_static))
            .filter(|(res, _, _)| *res != InstructionResult::Continue)
            .collect::<Vec<(InstructionResult, Gas, Bytes)>>() // collect is necessary here since we need to ensure all inspectors are called.
            .pop()
            .unwrap_or((InstructionResult::Continue, Gas::new(0), Bytes::new()))
    }

    #[doc = " Called when a call to a contract has concluded."]
    #[doc = ""]
    #[doc = " InstructionResulting anything other than the values passed to this function (`(ret, remaining_gas,"]
    #[doc = " out)`) will alter the result of the call."]
    fn call_end(
        &mut self,
        _data: &mut EVMData<'_, BS>,
        _inputs: &CallInputs,
        remaining_gas: Gas,
        ret: InstructionResult,
        out: Bytes,
        _is_static: bool,
    ) -> (InstructionResult, Gas, Bytes) {
        self.inspectors
            .iter_mut()
            .map(|insp| {
                insp.call_end(
                    _data,
                    _inputs,
                    remaining_gas,
                    ret,
                    out.clone(),
                    _is_static,
                )
            })
            .filter(|(res, g, o)| {
                *res != ret || !is_gas_equal(*g, remaining_gas) || *o != out
            })
            .collect::<Vec<(InstructionResult, Gas, Bytes)>>() // collect is necessary here since we need to ensure all inspectors are called.
            .pop()
            .unwrap_or((ret, remaining_gas, out))
    }

    #[doc = " Called when a contract is about to be created."]
    #[doc = ""]
    #[doc = " InstructionResulting anything other than [InstructionResult::Continue] overrides the result of the creation."]
    fn create(
        &mut self,
        _data: &mut EVMData<'_, BS>,
        _inputs: &mut CreateInputs,
    ) -> (InstructionResult, Option<B160>, Gas, Bytes) {
        self.inspectors
            .iter_mut()
            .map(|insp| insp.create(_data, _inputs))
            .filter(|(res, _, _, _)| *res != InstructionResult::Continue)
            .collect::<Vec<(InstructionResult, Option<B160>, Gas, Bytes)>>() // collect is necessary here since we need to ensure all inspectors are called.
            .pop()
            .unwrap_or((
                InstructionResult::Continue,
                None,
                Gas::new(0),
                Bytes::default(),
            ))
    }

    #[doc = " Called when a contract has been created."]
    #[doc = ""]
    #[doc = " InstructionResulting anything other than the values passed to this function (`(ret, remaining_gas,"]
    #[doc = " address, out)`) will alter the result of the create."]
    fn create_end(
        &mut self,
        _data: &mut EVMData<'_, BS>,
        _inputs: &CreateInputs,
        ret: InstructionResult,
        address: Option<B160>,
        remaining_gas: Gas,
        out: Bytes,
    ) -> (InstructionResult, Option<B160>, Gas, Bytes) {
        self.inspectors
            .iter_mut()
            .map(|insp| {
                insp.create_end(
                    _data,
                    _inputs,
                    ret,
                    address,
                    remaining_gas,
                    out.clone(),
                )
            })
            .filter(|(res, addr, g, o)| {
                *res != ret
                    || *addr != address
                    || !is_gas_equal(*g, remaining_gas)
                    || *o != out
            })
            .collect::<Vec<(InstructionResult, Option<B160>, Gas, Bytes)>>() // collect is necessary here since we need to ensure all inspectors are called.
            .pop()
            .unwrap_or((ret, address, remaining_gas, out))
    }

    #[doc = " Called when a contract has been self-destructed with funds transferred to target."]
    fn selfdestruct(&mut self, _contract: B160, _target: B160) {
        self.inspectors.iter_mut().for_each(|insp| {
            insp.selfdestruct(_contract, _target);
        })
    }
}

fn is_gas_equal(a: Gas, b: Gas) -> bool {
    a.limit() == b.limit()
        && a.spend() == b.spend()
        && a.memory() == b.memory()
        && a.refunded() == b.refunded()
        && a.remaining() == b.remaining()
}
