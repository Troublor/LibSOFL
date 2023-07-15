use reth_primitives::Address;
use revm::interpreter::{instruction_result::SuccessOrHalt, CallContext, Gas};
use revm_primitives::{Bytes, CreateScheme, U256};

#[derive(Clone, Debug)]
pub enum MsgCall {
    Invocation(Invocation),
    Creation(Creation),
}

#[derive(Clone, Debug)]
pub struct Invocation {
    pub context: CallContext,
    pub value: U256,
    pub input: Bytes,
    pub gas: Gas,
    pub output: Bytes,
    pub result: SuccessOrHalt,
}

impl Default for Invocation {
    fn default() -> Self {
        Self {
            context: Default::default(),
            gas: Gas::new(0),
            value: Default::default(),
            input: Default::default(),
            output: Default::default(),
            result: SuccessOrHalt::InternalContinue,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Creation {
    pub creator: Address,
    pub scheme: CreateScheme,
    pub value: U256,
    pub gas: Gas,
    pub init_code: Bytes,
    pub contract: Option<Address>,
    pub code: Bytes,
    pub result: SuccessOrHalt,
}

impl Default for Creation {
    fn default() -> Self {
        Self {
            creator: Default::default(),
            scheme: CreateScheme::Create,
            value: Default::default(),
            gas: Gas::new(0),
            init_code: Default::default(),
            contract: None,
            code: Default::default(),
            result: SuccessOrHalt::InternalContinue,
        }
    }
}
