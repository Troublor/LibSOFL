use libafl::prelude::{Input, UsesInput};
use libafl::{prelude::StdRand, state::State};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::engine::state::BcState;
use crate::fuzzing::OffersBcState;

/// FixedState holds an fixed blockchain state.
/// All inputs transactions are executed on this state.
#[derive(Debug, Clone)]
pub struct FixedState<I, S> {
    _phantom: std::marker::PhantomData<I>,

    pub rand: StdRand,

    pub bc_state: S,
}

impl<I, SS> Serialize for FixedState<I, SS> {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        unimplemented!()
    }
}

impl<'a, I, SS> Deserialize<'a> for FixedState<I, SS> {
    fn deserialize<D: Deserializer<'a>>(
        _deserializer: D,
    ) -> Result<Self, D::Error> {
        unimplemented!()
    }
}

impl<I: Input, S> State for FixedState<I, S> {}

impl<I: Input, S> UsesInput for FixedState<I, S> {
    type Input = I;
}

impl<I, S: BcState> OffersBcState<S> for FixedState<I, S> {
    fn offer_bc_state(&self) -> &S {
        &self.bc_state
    }
}
