use revm::Database;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::engine::types::{Context, EvmContext};

use super::revm::EvmContextSerde;

#[derive(Serialize, Deserialize)]
pub struct ContextSerde<EXT>
{
    /// External contexts.
    #[serde(bound(deserialize = "EXT: Deserialize<'de>", serialize = "EXT: Serialize"))]
    pub external: EXT,
}