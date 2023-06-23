use std::fmt::Debug;

use libafl::{prelude::Executor, state::UsesState};

use crate::fuzzing::state::fixed::FixedState;
use crate::{engine::state::BcState, fuzzing::corpus::tx::TxInput};

#[derive(Debug)]
pub struct TxExecutor<S: BcState> {
    _phantom: std::marker::PhantomData<S>,
}

/// TxExecutor execute a single transaction.
/// That is to say, the input of TxExecutor during fuzzing is a single transaction.
impl<S: BcState> TxExecutor<S> {
    pub fn new() -> Self {
        TxExecutor {
            _phantom: std::marker::PhantomData,
        }
    }
}

// impl Debug for TxExecutor<()> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("TxExecutor").finish()
//     }
// }

impl<S: BcState> UsesState for TxExecutor<S> {
    type State = FixedState<TxInput, S>;
}

impl<EM, Z, S: BcState> Executor<EM, Z> for TxExecutor<S>
where
    EM: UsesState<State = Self::State>,
    Z: UsesState<State = Self::State>,
{
    fn run_target(
        &mut self,
        fuzzer: &mut Z,
        state: &mut Self::State,
        mgr: &mut EM,
        input: &Self::Input,
    ) -> Result<libafl::prelude::ExitKind, libafl::Error> {
        Ok(libafl::prelude::ExitKind::Ok)
    }
}
