use libsofl_core::engine::{
    inspector::EvmInspector,
    state::{self, BcState},
    types::{
        opcode, Bytes, CallInputs, EVMData, Gas, Inspector, InstructionResult,
        Interpreter,
    },
};

use crate::taint::{
    call::TaintableCall, memory::TaintableMemory, stack::TaintableStack,
    storage::TaintableStorage, TaintMarker, TaintTracker,
};

use super::{
    policy::PropagationPolicy, stack::OPCODE_STACK_DELTA, TaintAnalyzer,
};

impl<'a, S: BcState, P: PropagationPolicy<S>> Inspector<S>
    for TaintAnalyzer<'a, S, P>
{
    fn step(
        &mut self,
        interp: &mut Interpreter<'_>,
        data: &mut EVMData<'_, S>,
    ) {
        let mut trackers = self.trackers.borrow_mut();
        let current_taint_tracker =
            trackers.last_mut().expect("bug: call stack underflow");
        self.stack_taint_effects =
            self.policy.before_step(current_taint_tracker, interp, data);
        // pop values from taintable stack
        match interp.current_opcode() {
            op if opcode::PUSH0 <= op && op <= opcode::PUSH32 => {}
            op if opcode::DUP1 <= op && op <= opcode::DUP16 => {}
            op if opcode::SWAP1 <= op && op <= opcode::SWAP16 => {}
            opcode::CREATE
            | opcode::CALL
            | opcode::CALLCODE
            | opcode::DELEGATECALL
            | opcode::CREATE2
            | opcode::STATICCALL => {
                // construct child taintable call
                let child_call = TaintableCall::default();
                current_taint_tracker.child_call = Some(child_call);
            }
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
        let mut trackers = self.trackers.borrow_mut();
        let current_taint_tracker =
            trackers.last_mut().expect("bug: call stack underflow");
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

        self.policy.after_step(current_taint_tracker, interp, data);
    }

    fn call(
        &mut self,
        data: &mut EVMData<'_, S>,
        inputs: &mut CallInputs,
    ) -> (InstructionResult, Gas, Bytes) {
        let mut trackers = self.trackers.borrow_mut();
        let current_taint_tracker =
            trackers.last_mut().expect("bug: call stack underflow");

        let state_addr = inputs.context.address;

        let child_call = current_taint_tracker
            .child_call
            .as_mut()
            .expect("bug: child call not constructed");
        let mut storages = self.storages.borrow_mut();
        let child_storage = storages.entry(state_addr).or_default();

        let child_taint = TaintTracker {
            stack: TaintableStack::default(),
            memory: TaintableMemory::default(),
            storage: child_storage,
            call: child_call,
            child_call: None,
        };

        // trackers.push(child_taint);

        (InstructionResult::Continue, Gas::new(0), Bytes::new())
    }

    fn call_end(
        &mut self,
        data: &mut EVMData<'_, S>,
        inputs: &CallInputs,
        remaining_gas: Gas,
        ret: InstructionResult,
        out: Bytes,
    ) -> (InstructionResult, Gas, Bytes) {
        (ret, remaining_gas, out)
    }
}

impl<'a, S: BcState, P: PropagationPolicy<S>> EvmInspector<S>
    for TaintAnalyzer<'a, S, P>
{
}
