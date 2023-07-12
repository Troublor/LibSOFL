use reth_primitives::Address;
use revm::{
    interpreter::{instruction_result::SuccessOrHalt, CallContext},
    Database, Inspector,
};
use revm_primitives::{Bytes, U256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsgCall {
    pub context: CallContext,
    pub gas_limit: u64,
    pub value: U256,
    pub input: Bytes,
    pub output: Bytes,
    pub result: SuccessOrHalt,
}

impl Default for MsgCall {
    fn default() -> Self {
        Self {
            context: Default::default(),
            gas_limit: 0,
            value: Default::default(),
            input: Default::default(),
            output: Default::default(),
            result: SuccessOrHalt::InternalContinue,
        }
    }
}
