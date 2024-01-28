use libsofl_core::engine::{
    inspector::EvmInspector,
    state::BcState,
    types::{opcode, Inspector},
};

use crate::taint::{
    call::TaintableCall, memory::TaintableMemory, stack::TaintableStack,
    TaintTracker,
};

use super::{policy::TaintPolicy, stack::OPCODE_STACK_DELTA, TaintAnalyzer};

impl<S: BcState, P: TaintPolicy<S>> Inspector<S> for TaintAnalyzer<S, P> {
    fn step(
        &mut self,
        interp: &mut libsofl_core::engine::types::Interpreter,
        context: &mut libsofl_core::engine::types::EvmContext<S>,
    ) {
        let state_addr = interp.contract().address;
        let taint_stack = self.stacks.last_mut().unwrap();
        let taint_memory = self.memories.last_mut().unwrap();
        let taint_storage = self.storages.entry(state_addr).or_default();
        let (ref mut taint_call, ref mut op) = self.calls.last_mut().unwrap();
        op.replace(interp.current_opcode());
        let taint_child_call = match interp.current_opcode() {
            opcode::CREATE
            | opcode::CALL
            | opcode::CALLCODE
            | opcode::DELEGATECALL
            | opcode::CREATE2
            | opcode::STATICCALL => {
                // construct child taintable call
                let child_call = TaintableCall::new(self.memory_word_size);
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

        self.stack_taint_effects = self.policy.before_step(
            &mut current_taint_tracker,
            interp,
            context,
        );
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
        interp: &mut libsofl_core::engine::types::Interpreter,
        context: &mut libsofl_core::engine::types::EvmContext<S>,
    ) {
        let state_addr = interp.contract().address;
        let taint_stack = self.stacks.last_mut().unwrap();
        let taint_memory = self.memories.last_mut().unwrap();
        let taint_storage = self.storages.entry(state_addr).or_default();
        let (ref mut taint_call, ref op) = self.calls.last_mut().unwrap();
        let taint_child_call = self.child_calls.last_mut().unwrap().as_mut();
        let mut current_taint_tracker = TaintTracker {
            stack: taint_stack,
            memory: taint_memory,
            storage: taint_storage,
            call: taint_call,
            child_call: taint_child_call,
        };
        let op = op.unwrap();

        // push values to taintable stack
        match op {
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
                let (_, n_push) = OPCODE_STACK_DELTA[op as usize];
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

        assert_eq!(
            interp.stack().len(),
            current_taint_tracker.stack.raw().len()
        );

        self.policy
            .after_step(&mut current_taint_tracker, op, interp, context);

        // sanity check
        assert_eq!(
            interp.stack().len(),
            current_taint_tracker.stack.raw().len()
        );
    }

    fn call(
        &mut self,
        context: &mut libsofl_core::engine::types::EvmContext<S>,
        _inputs: &mut libsofl_core::engine::types::CallInputs,
        _return_memory_offset: std::ops::Range<usize>,
    ) -> Option<libsofl_core::engine::types::CallOutcome> {
        let taint_stack = TaintableStack::default();
        self.stacks.push(taint_stack);
        let taint_memory = TaintableMemory::new(self.memory_word_size);
        self.memories.push(taint_memory);
        let child_call = if context.journaled_state.depth() != 0 {
            self.child_calls.last_mut().unwrap().take().unwrap()
        } else {
            TaintableCall::new(self.memory_word_size)
        };
        self.calls.push((child_call, None));
        self.child_calls.push(None);

        // sanity check
        assert_eq!(self.calls.len(), self.stacks.len());
        assert_eq!(self.calls.len(), self.memories.len());
        assert_eq!(self.calls.len(), self.child_calls.len());

        None
    }

    fn call_end(
        &mut self,
        context: &mut libsofl_core::engine::types::EvmContext<S>,
        _inputs: &libsofl_core::engine::types::CallInputs,
        result: libsofl_core::engine::types::InterpreterResult,
    ) -> libsofl_core::engine::types::InterpreterResult {
        self.stacks.pop();
        self.memories.pop();
        self.child_calls.pop();
        let (child_call, _) = self.calls.pop().unwrap();
        if context.journaled_state.depth() != 1 {
            // call depth is shifted in call_end hook: https://github.com/bluealloy/revm/issues/1018
            self.child_calls.last_mut().unwrap().replace(child_call);
        }

        // sanity check
        assert_eq!(self.calls.len(), self.stacks.len());
        assert_eq!(self.calls.len(), self.memories.len());
        assert_eq!(self.calls.len(), self.child_calls.len());

        result
    }

    fn create(
        &mut self,
        _context: &mut libsofl_core::engine::types::EvmContext<S>,
        _inputs: &mut libsofl_core::engine::types::CreateInputs,
    ) -> Option<libsofl_core::engine::types::CreateOutcome> {
        let taint_stack = TaintableStack::default();
        self.stacks.push(taint_stack);
        let taint_memory = TaintableMemory::new(self.memory_word_size);
        self.memories.push(taint_memory);
        let child_call = self.child_calls.last_mut().unwrap().take().unwrap();
        self.calls.push((child_call, None));

        // sanity check
        assert_eq!(self.calls.len(), self.stacks.len());
        assert_eq!(self.calls.len(), self.memories.len());
        assert_eq!(self.calls.len(), self.child_calls.len());

        None
    }

    fn create_end(
        &mut self,
        _context: &mut libsofl_core::engine::types::EvmContext<S>,
        _inputs: &libsofl_core::engine::types::CreateInputs,
        result: libsofl_core::engine::types::InterpreterResult,
        address: Option<libsofl_core::engine::types::Address>,
    ) -> libsofl_core::engine::types::CreateOutcome {
        self.stacks.pop();
        self.memories.pop();
        let (child_call, _) = self.calls.pop().unwrap();
        self.child_calls.last_mut().unwrap().replace(child_call);

        // sanity check
        assert_eq!(self.calls.len(), self.stacks.len());
        assert_eq!(self.calls.len(), self.memories.len());
        assert_eq!(self.calls.len(), self.child_calls.len());

        libsofl_core::engine::types::CreateOutcome::new(result, address)
    }
}

impl<S: BcState, P: TaintPolicy<S>> EvmInspector<S> for TaintAnalyzer<S, P> {}
