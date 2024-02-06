use libafl::{executors::Executor, state::UsesState};

use crate::{blockchain::state_ref::FuzzBcStateRef, state::FuzzState};

pub mod call_executor;
pub mod evm;

pub type FuzzExecutor<SR> = MSgCallExecutor<SR>;

/// The executor for message call sequences.
/// Type parameters:
/// - `SR`: FuzzBcStateRef
pub struct MSgCallExecutor<SR> {
    _phantom: std::marker::PhantomData<SR>,
}

impl<SR: FuzzBcStateRef> UsesState for MSgCallExecutor<SR> {
    type State = FuzzState<SR>;
}

impl<SR, EM, Z> Executor<EM, Z> for MSgCallExecutor<SR>
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
        input: &Self::Input,
    ) -> Result<libafl::prelude::ExitKind, libafl::prelude::Error> {
        // We run the input, which is `MsgCallSeq`.
        // The MsgCallSeq may or may not have already been executed,
        // which is indicated by the lenght of the states vector in the `MsgCallSeq`.
        let (_state, calls) = input.get_execution_data();
        if calls.is_empty() {
            // The input has already been executed.
            return Ok(libafl::prelude::ExitKind::Ok);
        }
        // execute the call sequence on the pre-execution state.
        // note that the call sequence may contain nested calls.
        todo!()
    }
}
