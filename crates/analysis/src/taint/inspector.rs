use libsofl_core::engine::{
    inspector::EvmInspector,
    state::BcState,
    types::{
        opcode, Address, Bytes, CallInputs, CreateInputs, EVMData, Gas,
        Inspector, InstructionResult, Interpreter,
    },
};

use crate::taint::{
    call::TaintableCall, memory::TaintableMemory, stack::TaintableStack,
    TaintTracker,
};

use super::{
    policy::PropagationPolicy, stack::OPCODE_STACK_DELTA, TaintAnalyzer,
};

impl<S: BcState, P: PropagationPolicy<S>> Inspector<S> for TaintAnalyzer<S, P> {
    fn step(
        &mut self,
        interp: &mut Interpreter<'_>,
        data: &mut EVMData<'_, S>,
    ) {
        let state_addr = interp.contract().address;
        let taint_stack = self.stacks.last_mut().unwrap();
        let taint_memory = self.memories.last_mut().unwrap();
        let taint_storage = self.storages.entry(state_addr).or_default();
        let taint_call = self.calls.last_mut().unwrap();
        let taint_child_call = match interp.current_opcode() {
            opcode::CREATE
            | opcode::CALL
            | opcode::CALLCODE
            | opcode::DELEGATECALL
            | opcode::CREATE2
            | opcode::STATICCALL => {
                // construct child taintable call
                let child_call = TaintableCall::default();
                self.child_calls.last_mut().unwrap().replace(child_call);
                self.child_calls.last_mut().unwrap().as_mut()
            }
            _ => None,
        };
        let mut current_taint_tracker = TaintTracker {
            stack: taint_stack,
            memory: taint_memory,
            storage: taint_storage,
            call: taint_call,
            child_call: taint_child_call,
        };

        self.stack_taint_effects =
            self.policy
                .before_step(&mut current_taint_tracker, interp, data);
        // pop values from taintable stack
        match interp.current_opcode() {
            op if opcode::PUSH0 <= op && op <= opcode::PUSH32 => {}
            op if opcode::DUP1 <= op && op <= opcode::DUP16 => {}
            op if opcode::SWAP1 <= op && op <= opcode::SWAP16 => {}
            _ => {
                let (n_pop, _) =
                    OPCODE_STACK_DELTA[interp.current_opcode() as usize];
                #[allow(deprecated)]
                current_taint_tracker.stack.pop(n_pop);
            }
        }
    }

    fn step_end(
        &mut self,
        interp: &mut Interpreter<'_>,
        data: &mut EVMData<'_, S>,
    ) {
        let state_addr = interp.contract().address;
        let taint_stack = self.stacks.last_mut().unwrap();
        let taint_memory = self.memories.last_mut().unwrap();
        let taint_storage = self.storages.entry(state_addr).or_default();
        let taint_call = self.calls.last_mut().unwrap();
        let taint_child_call = self.child_calls.last_mut().unwrap().as_mut();
        let mut current_taint_tracker = TaintTracker {
            stack: taint_stack,
            memory: taint_memory,
            storage: taint_storage,
            call: taint_call,
            child_call: taint_child_call,
        };

        // push values to taintable stack
        match interp.current_opcode() {
            op if opcode::PUSH0 <= op && op <= opcode::PUSH32 => {
                #[allow(deprecated)]
                current_taint_tracker.stack.push(1, false);
            }
            op if opcode::DUP1 <= op && op <= opcode::DUP16 => {
                let tainted = current_taint_tracker
                    .stack
                    .is_tainted((op - opcode::DUP1) as usize);
                #[allow(deprecated)]
                current_taint_tracker.stack.push(1, tainted);
            }
            op if opcode::SWAP1 <= op && op <= opcode::SWAP16 => {
                let n = (op - opcode::SWAP1 + 1) as usize;
                let tainted_top = current_taint_tracker.stack.is_tainted(0);
                let tainted_nth = current_taint_tracker.stack.is_tainted(n);
                if tainted_nth {
                    current_taint_tracker.stack.taint(0);
                } else {
                    current_taint_tracker.stack.clean(0);
                }
                if tainted_top {
                    current_taint_tracker.stack.taint(n);
                } else {
                    current_taint_tracker.stack.clean(n);
                }
            }
            _ => {
                let (_, n_push) =
                    OPCODE_STACK_DELTA[interp.current_opcode() as usize];
                #[allow(deprecated)]
                current_taint_tracker.stack.push(n_push, false);
            }
        }
        // apply taint stack effects
        for (i, tainted) in self.stack_taint_effects.iter().rev().enumerate() {
            if let Some(tainted) = tainted {
                if *tainted {
                    current_taint_tracker.stack.taint(i);
                } else {
                    current_taint_tracker.stack.clean(i);
                }
            }
        }

        self.policy
            .after_step(&mut current_taint_tracker, interp, data);
    }

    fn call(
        &mut self,
        _data: &mut EVMData<'_, S>,
        _inputs: &mut CallInputs,
    ) -> (InstructionResult, Gas, Bytes) {
        let taint_stack = TaintableStack::default();
        self.stacks.push(taint_stack);
        let taint_memory = TaintableMemory::default();
        self.memories.push(taint_memory);
        let child_call = self.child_calls.last_mut().unwrap().take().unwrap();
        self.calls.push(child_call);

        // sanity check
        assert_eq!(self.calls.len(), self.stacks.len());
        assert_eq!(self.calls.len(), self.memories.len());
        assert_eq!(self.calls.len(), self.child_calls.len());

        (InstructionResult::Continue, Gas::new(0), Bytes::new())
    }

    fn call_end(
        &mut self,
        _data: &mut EVMData<'_, S>,
        _inputs: &CallInputs,
        remaining_gas: Gas,
        ret: InstructionResult,
        out: Bytes,
    ) -> (InstructionResult, Gas, Bytes) {
        self.stacks.pop();
        self.memories.pop();
        let child_call = self.calls.pop().unwrap();
        self.child_calls.last_mut().unwrap().replace(child_call);

        // sanity check
        assert_eq!(self.calls.len(), self.stacks.len());
        assert_eq!(self.calls.len(), self.memories.len());
        assert_eq!(self.calls.len(), self.child_calls.len());

        (ret, remaining_gas, out)
    }

    fn create(
        &mut self,
        _data: &mut EVMData<'_, S>,
        _inputs: &mut CreateInputs,
    ) -> (InstructionResult, Option<Address>, Gas, Bytes) {
        let taint_stack = TaintableStack::default();
        self.stacks.push(taint_stack);
        let taint_memory = TaintableMemory::default();
        self.memories.push(taint_memory);
        let child_call = self.child_calls.last_mut().unwrap().take().unwrap();
        self.calls.push(child_call);

        // sanity check
        assert_eq!(self.calls.len(), self.stacks.len());
        assert_eq!(self.calls.len(), self.memories.len());
        assert_eq!(self.calls.len(), self.child_calls.len());

        (
            InstructionResult::Continue,
            None,
            Gas::new(0),
            Bytes::default(),
        )
    }

    fn create_end(
        &mut self,
        _data: &mut EVMData<'_, S>,
        _inputs: &CreateInputs,
        ret: InstructionResult,
        address: Option<Address>,
        remaining_gas: Gas,
        out: Bytes,
    ) -> (InstructionResult, Option<Address>, Gas, Bytes) {
        self.stacks.pop();
        self.memories.pop();
        let child_call = self.calls.pop().unwrap();
        self.child_calls.last_mut().unwrap().replace(child_call);

        // sanity check
        assert_eq!(self.calls.len(), self.stacks.len());
        assert_eq!(self.calls.len(), self.memories.len());
        assert_eq!(self.calls.len(), self.child_calls.len());

        (ret, address, remaining_gas, out)
    }
}

impl<S: BcState, P: PropagationPolicy<S>> EvmInspector<S>
    for TaintAnalyzer<S, P>
{
}
