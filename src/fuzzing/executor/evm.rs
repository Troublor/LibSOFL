use libafl::{
    prelude::{Executor, StdRand, UsesInput},
    state::{StdState, UsesState},
};

use crate::fuzzing::corpus::tx::{TxCorpus, TxInput};

#[derive(Debug)]
pub struct TxExecutor {}

/// TxExecutor execute a single transaction.
/// That is to say, the input of TxExecutor during fuzzing is a single transaction.
impl TxExecutor {
    pub fn new() -> Self {
        TxExecutor {}
    }
}

impl UsesState for TxExecutor {
    type State = StdState<TxInput, TxCorpus, StdRand, TxCorpus>;
}

impl<EM, Z> Executor<EM, Z> for TxExecutor
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
