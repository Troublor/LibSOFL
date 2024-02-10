// This module reinvents the EVM and support interrupting the execution of the EVM, and resume the execution of the EVM later.
// This implementation is based on the fragile branch `reth_freeze`, subject to substantial change.
pub mod breakpoint;
pub mod differential_testing;
pub mod evm;
mod tests;

use std::fmt;

use revm::{inspector_handle_register, Evm, EvmBuilder, Frame, GetInspector};

use crate::engine::revm::revm::interpreter::SharedMemory;

use self::evm::Action;

use super::{
    inspector::{no_inspector, EvmInspector, NoInspector},
    state::BcState,
    transition::TransitionSpec,
    types::SpecId,
};

/// EVM call stack limit.
pub const CALL_STACK_LIMIT: u64 = 1024;

pub struct ResumableContext<'a, DB: BcState, I> {
    pub revm_ctx: Evm<'a, I, DB>,
    pub call_stack_stages: Vec<Vec<Frame>>, // stages of call stacks
    pub shared_memory: SharedMemory,
    pub next_action_stages: Vec<Action>, // stages of next actions
}

impl<'a, DB: BcState> ResumableContext<'a, DB, &mut NoInspector> {
    pub fn new(state: DB, spec_id: SpecId) -> Self {
        let revm_ctx = EvmBuilder::default()
            .with_db(state)
            .with_external_context(no_inspector())
            .spec_id(spec_id)
            .append_handler_register(inspector_handle_register)
            .build();
        let shared_memory = SharedMemory::new_with_memory_limit(
            revm_ctx.context.evm.env.cfg.memory_limit,
        );
        Self {
            revm_ctx,
            call_stack_stages: vec![],
            shared_memory,
            next_action_stages: vec![],
        }
    }
}

impl<'a, DB: BcState, I: GetInspector<DB>> ResumableContext<'a, DB, I> {
    pub fn begin_stage(&mut self) {
        self.call_stack_stages
            .push(Vec::with_capacity(CALL_STACK_LIMIT as usize + 1));
        self.next_action_stages.push(Action::Continue);
    }

    pub fn end_stage(&mut self) {
        self.call_stack_stages.pop().expect("call stack is empty");
        self.next_action_stages.pop().expect("next action is empty");
    }

    pub fn take_call_stack(&mut self) -> Vec<Frame> {
        let len = self.call_stack_stages.len();
        std::mem::take(&mut self.call_stack_stages[len - 1])
    }

    pub fn replace_call_stack(&mut self, call_stack: Vec<Frame>) {
        let len = self.call_stack_stages.len();
        let _ =
            std::mem::replace(&mut self.call_stack_stages[len - 1], call_stack);
    }

    pub fn take_shared_memory(&mut self) -> SharedMemory {
        std::mem::take(&mut self.shared_memory)
    }

    pub fn take_next_action(&mut self) -> Action {
        let len = self.next_action_stages.len();
        std::mem::take(&mut self.next_action_stages[len - 1])
    }

    pub fn replace_next_action(&mut self, next_action: Action) {
        let len = self.next_action_stages.len();
        let _ = std::mem::replace(
            &mut self.next_action_stages[len - 1],
            next_action,
        );
    }

    pub fn is_new_transaction(&self) -> bool {
        // !self.in_progress
        self.call_stack_stages.is_empty()
    }
}

/// EVM instance containing both internal EVM context and external context
/// and the handler that dictates the logic of EVM (or hardfork specification).
pub struct InterruptableEvm<S: BcState, I: EvmInspector<S>> {
    pub spec_id: SpecId,

    pub inspector: Option<I>,

    _phantom: std::marker::PhantomData<S>,
}

impl<DB, I> fmt::Debug for InterruptableEvm<DB, I>
where
    DB: BcState + fmt::Debug,
    DB::Error: fmt::Debug,
    I: EvmInspector<DB>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Evm")
            // .field("evm context", &self.context.ctx.evm)
            .finish_non_exhaustive()
    }
}

impl<S: BcState> InterruptableEvm<S, NoInspector> {
    pub fn new(
        spec_id: SpecId,
    ) -> InterruptableEvm<S, &'static mut NoInspector> {
        InterruptableEvm {
            spec_id,
            inspector: Some(no_inspector()),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<S: BcState, I: EvmInspector<S>> InterruptableEvm<S, I> {
    pub fn new_with_inspector(
        spec_id: SpecId,
        inspector: I,
    ) -> InterruptableEvm<S, I> {
        InterruptableEvm {
            spec_id,
            inspector: Some(inspector),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn take_inspector(&mut self) -> Option<I> {
        self.inspector.take()
    }

    pub fn build_resumable_run_context(
        &self,
        state: S,
        mut spec: TransitionSpec,
    ) -> ResumableContext<S, &mut NoInspector> {
        let mut ctx = ResumableContext::new(state, self.spec_id);
        assert_eq!(spec.txs.len(), 1, "only one tx is supported");
        ctx.revm_ctx.context.evm.env().cfg = spec.cfg;
        ctx.revm_ctx.context.evm.env().block = spec.block;
        ctx.revm_ctx.context.evm.env().tx = spec.txs.remove(0);
        ctx
    }
}
