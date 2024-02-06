use std::sync::Arc;

use libafl::inputs::Input;
use libsofl_core::engine::memory::MemoryBcState;

use self::call::MsgCall;

pub mod call;
pub mod calldata;

pub type FuzzInput<SR> = MsgCallSeq<MemoryBcState<SR>>;

/// The input of fuzzing is a sequence of calls.
/// Type parameters:
/// - `S`: BcState
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MsgCallSeq<S> {
    /// Unique identification of this input
    pub id: u32,

    /// Sequence of message calls
    pub calls: Vec<MsgCall>,

    /// The pre-/post-execution state of each call.
    /// The length of `states` is `calls.len() + 1`.
    /// Each state is a `Arc` smart pointer, in that
    /// one state may be shared by multiple inputs.
    pub states: Vec<Arc<S>>,
}

impl<S> Input for MsgCallSeq<S>
where
    S: std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned,
{
    fn generate_name(&self, idx: usize) -> String {
        format!("call_seq_{}", idx)
    }
}
