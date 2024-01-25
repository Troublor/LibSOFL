use std::collections::HashMap;

use libsofl_core::engine::types::{
    Address, Bytes, CallContext, CallScheme, CreateScheme, TransactTo, TxEnv,
    U256,
};

use super::calldata::StructuredCalldata;

#[derive(Debug, Clone, AsRef)]
pub struct MsgCallInput {
    #[as_ref]
    pub direct_call: MsgCall,

    /// The msg calls to perform in the callback initiated by the current call, if the callback contract address and call scheme match.
    /// The hijacked calls may also contain nested hijacked calls.
    pub hijacked_calls: HashMap<HijackTarget, Vec<HijackedMsgCallSpec>>,
}

impl From<MsgCall> for MsgCallInput {
    fn from(value: MsgCall) -> Self {
        Self {
            direct_call: value,
            hijacked_calls: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct HijackTarget {
    pub code_address: Address,
    pub call_scheme: CallScheme,
}

#[derive(Debug, Clone)]
pub struct HijackedMsgCallSpec {
    pub calls: Vec<MsgCallInput>,
    pub success: bool,
    pub gas_used: u64,
    pub return_data: Bytes,
}

/// MsgCall depicts a invocation to a contract.
#[derive(Debug, Clone)]
pub struct MsgCall {
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

impl Default for MsgCall {
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

impl MsgCall {
    pub fn to_tx_env(&self) -> TxEnv {
        self.clone().into()
    }
}

impl From<MsgCall> for TxEnv {
    fn from(value: MsgCall) -> Self {
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
