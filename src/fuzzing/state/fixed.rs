use libafl::{prelude::StdRand, state::State};
use serde::{Deserialize, Serialize};

use crate::engine::state::BcState;

/// FixedState holds an fixed blockchain state.
/// All inputs transactions are executed on this state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedState<I, S: BcState<DBERR>, DBERR> {
    _phantom: std::marker::PhantomData<I>,

    pub rand: StdRand,

    pub bc_state: S,
}

impl<I> State for FixedState<I> {}

impl<I> UsesInput for FixedState<I> {
    type Input = I;
}

impl<I, S: BcState<DBERR>, DBERR> OffersBcState for FixedState<I> {
    fn offer_bc_state(&self) -> &S {
        &self.bc_state
    }
}
