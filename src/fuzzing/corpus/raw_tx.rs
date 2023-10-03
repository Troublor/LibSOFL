use revm_primitives::TxEnv;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTx(pub TxEnv);

impl libafl::inputs::Input for RawTx {
    fn generate_name(&self, idx: usize) -> String {
        format!("tx_{}", idx)
    }
}
