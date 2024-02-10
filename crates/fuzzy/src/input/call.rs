use libsofl_core::engine::types::{Address, TransactTo, TxEnv, U256};

use super::calldata::StructuredCalldata;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MsgCall {
    /// The direct message call to subject contracts.
    TopLevel(Call),

    /// The message call to subject contracts in the callback of outsider contracts.
    Nested { call: Call, depth: u32 },
}

impl MsgCall {
    pub fn into_call(self) -> Call {
        match self {
            MsgCall::TopLevel(call) => call,
            MsgCall::Nested { call, .. } => call,
        }
    }

    pub fn as_call(&self) -> &Call {
        match self {
            MsgCall::TopLevel(call) => call,
            MsgCall::Nested { call, .. } => call,
        }
    }

    pub fn depth(&self) -> u32 {
        match self {
            MsgCall::TopLevel(_) => 0,
            MsgCall::Nested { depth, .. } => *depth,
        }
    }
}

/// MsgCall depicts a invocation to a contract.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Call {
    /// Caller (i.e., msg.sender)
    pub caller: Address,

    /// Callee (i.e., contract addresss), None if it is a contract creation
    pub transact_to: TransactTo,

    /// Calldata
    pub calldata: StructuredCalldata,

    /// Call value
    pub value: U256,

    /// Gas limit
    pub gas_limit: u64,
}

impl Default for Call {
    fn default() -> Self {
        Self {
            caller: Address::ZERO,
            transact_to: TransactTo::Call(Address::ZERO),
            calldata: StructuredCalldata::default(),
            value: U256::ZERO,
            gas_limit: u64::MAX,
        }
    }
}

impl Call {
    pub fn to_tx_env(&self) -> TxEnv {
        self.clone().into()
    }
}

impl From<Call> for TxEnv {
    fn from(value: Call) -> Self {
        Self {
            caller: value.caller,
            gas_limit: value.gas_limit,
            gas_price: U256::ZERO,
            transact_to: value.transact_to,
            value: value.value,
            data: value.calldata.bytes(),
            nonce: None,
            chain_id: None,
            ..Default::default()
        }
    }
}
