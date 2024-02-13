pub mod interpreter;
pub mod revm;

use serde::{Deserialize, Serialize};

use crate::engine::types::Frame;
use revm::FrameSerde;

#[derive(
    derive_more::AsRef,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::From,
    derive_more::Into,
    Serialize,
    Deserialize,
)]
pub struct WrappedFrame(
    #[serde(with = "FrameSerde")]
    #[as_ref]
    #[deref]
    #[deref_mut]
    pub Frame,
);
