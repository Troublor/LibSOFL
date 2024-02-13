use revm::{inspector_handle_register, interpreter::CallInputs, GetInspector};
use serde::{Deserialize, Serialize};

use crate::{
    engine::{
        state::BcState,
        transition::TransitionSpec,
        types::{
            Evm, EvmBuilder, Handler, HandlerCfg, Inspector, SharedMemory,
            SpecId,
        },
    },
    error::SoflError,
};

use super::{
    breakpoint::{Breakpoint, BreakpointResult, RunResult},
    execution::Action,
    execution::Executor,
    serde_helpers::{revm::ContextSerde, WrappedFrame},
};

#[derive(Serialize, Deserialize)]
#[serde(bound(
    serialize = "S: Serialize + BcState, I: Serialize",
    deserialize = "S: Deserialize<'de> + BcState, I: Deserialize<'de>"
))]
pub struct InterruptableEvm<S: BcState, I> {
    pub spec_id: SpecId,
    pub context: Option<ContextSerde<I, S>>,
    pub call_stack_stages: Vec<Vec<WrappedFrame>>, // stages of call stacks
    pub shared_memory: SharedMemory,
    pub next_action_stages: Vec<Action>, // stages of next actions
}

impl<S: BcState, I: Inspector<S>> InterruptableEvm<S, I> {
    pub fn new(
        spec_id: SpecId,
        state: S,
        mut spec: TransitionSpec,
        inspector: I,
    ) -> Self {
        let mut evm = EvmBuilder::default()
            .with_db(state)
            .with_external_context(inspector)
            .spec_id(spec_id)
            .append_handler_register(inspector_handle_register)
            .build();
        evm.context.evm.env.cfg = spec.cfg;
        evm.context.evm.env.block = spec.block;
        evm.context.evm.env.tx = spec.txs.remove(0);
        let shared_memory = SharedMemory::new_with_memory_limit(
            evm.context.evm.env.cfg.memory_limit,
        );
        Self {
            spec_id,
            context: Some(evm.context.into()),
            call_stack_stages: vec![],
            shared_memory,
            next_action_stages: vec![],
        }
    }
}

impl<S: BcState, I> InterruptableEvm<S, I> {
    pub fn take_state_and_inspector(self) -> (S, I) {
        let context = self.context.expect("Context not found");
        (context.evm.db, context.external)
    }
}

impl<S: BcState, I> InterruptableEvm<S, I> {
    fn distill_executor<'a>(&mut self) -> Executor<'a, S, I> {
        let context = self.context.take().expect("Context not found");
        let evm = Evm {
            context: context.into(),
            handler: Handler::new(HandlerCfg::new(self.spec_id)),
        };
        let call_stack_stages = std::mem::take(&mut self.call_stack_stages);
        let call_stack_stages = call_stack_stages
            .into_iter()
            .map(|frames| {
                frames.into_iter().map(|frame| frame.into()).collect()
            })
            .collect();
        let shared_memory = std::mem::take(&mut self.shared_memory);
        let next_action_stages = std::mem::take(&mut self.next_action_stages);
        Executor {
            evm,
            call_stack_stages,
            shared_memory,
            next_action_stages,
        }
    }

    fn dissolve_executor<'a>(&mut self, executor: Executor<'a, S, I>) {
        let evm = executor.evm;
        let context = evm.context.into();
        self.context = Some(context);
        self.call_stack_stages = executor
            .call_stack_stages
            .into_iter()
            .map(|frames| {
                frames.into_iter().map(|frame| frame.into()).collect()
            })
            .collect();
        self.shared_memory = executor.shared_memory;
        self.next_action_stages = executor.next_action_stages;
    }
}

impl<S: BcState, I: GetInspector<S>> InterruptableEvm<S, I> {
    /// Continue to run the EVM until it reaches a breakpoint or transaction completes.
    pub fn run<M, B: Breakpoint<M>>(
        &mut self,
        breakpoint: B,
    ) -> Result<RunResult<M>, SoflError> {
        let mut executor = self.distill_executor();
        let r = executor.run(breakpoint)?;
        self.dissolve_executor(executor);
        Ok(r)
    }

    /// Perform a message call at the current state of the EVM.
    pub fn msg_call<M, B: Breakpoint<M>>(
        &mut self,
        inputs: CallInputs,
        breakpoint: B,
    ) -> Result<BreakpointResult<M>, SoflError> {
        let mut executor = self.distill_executor();
        let r = executor.msg_call(inputs, breakpoint)?;
        self.dissolve_executor(executor);
        Ok(r)
    }
}
