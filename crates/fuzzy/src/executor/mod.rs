use libafl::{executors::Executor, state::UsesState};

use crate::{blockchain::state_ref::FuzzBcStateRef, state::FuzzState};

pub mod call_executor;
pub mod evm;

/// The executor for message call sequences.
/// Type parameters:
/// - `SR`: FuzzBcStateRef
pub struct MsgCallSeqExecutor<SR> {
    _phantom: std::marker::PhantomData<SR>,
}

impl<SR: FuzzBcStateRef> UsesState for MsgCallSeqExecutor<SR> {
    type State = FuzzState<SR>;
}

impl<SR, EM, Z> Executor<EM, Z> for MsgCallSeqExecutor<SR>
where
    SR: FuzzBcStateRef,
    EM: UsesState<State = Self::State>,
    Z: UsesState<State = Self::State>,
{
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        _state: &mut Self::State,
        _mgr: &mut EM,
        _input: &Self::Input,
    ) -> Result<libafl::prelude::ExitKind, libafl::prelude::Error> {
        todo!()
    }
}
