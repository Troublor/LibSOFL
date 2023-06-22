use libafl::{prelude::Executor, state::UsesState};

#[derive(Debug)]
pub struct EvmExecutor {}

impl EvmExecutor {
    pub fn new() -> Self {
        EvmExecutor {}
    }
}

// impl UsesState for EvmExecutor {
//     type State = PomFuzzState;
// }

// impl Executor<EM, Z> for EvmExecutor
// where
//     EM: UsesState<State = Self::State>,
//     Z: UsesState<State = Self::State>,
// {
//     fn run_target(
//         &mut self,
//         fuzzer: &mut Z,
//         state: &mut Self::State,
//         mgr: &mut EM,
//         input: &Self::Input,
//     ) -> Result<libafl::prelude::ExitKind, libafl::Error> {
//         Ok(libafl::prelude::ExitKind::Ok)
//     }
// }
