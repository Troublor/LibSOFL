use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// The state of the fuzzer.
/// FuzzState implements the State trait of libafl.
/// Type parameters:
/// - I: the type of the input
/// - C: the type of the corpus
/// - R: the type of the RNG
/// - SC: the type of the solutions corpus
#[derive(Debug, Serialize, Deserialize)]
pub struct FuzzState<I, C, R, SC> {
    // TODO: add the other fields
    _phantom: PhantomData<(I, C, R, SC)>,
}

// Required by trait libafl::state::State
impl<I: libafl::inputs::Input, C, R, SC> libafl::inputs::UsesInput
    for FuzzState<I, C, R, SC>
{
    type Input = I;
}

impl<I: libafl::inputs::Input, C, R, SC> libafl::state::State
    for FuzzState<I, C, R, SC>
{
}
