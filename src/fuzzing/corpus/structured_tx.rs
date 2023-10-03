use serde::{Deserialize, Serialize};

/// StructuredTxInput is a transaction with a satisfying an ABI specification.
/// StructuredTxInput is a transaction input that is structuredly stored to facilitate future mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredTxInput {
    args: Vec<InstantiatedArg>,
}

impl libafl::inputs::Input for StructuredTxInput {
    fn generate_name(&self, _idx: usize) -> String {
        format!("tx_{}", _idx)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstantiatedArg {
    param: ethers::abi::Param,
    value: ethers::abi::Token,
}
