use std::collections::HashSet;

use libsofl_core::engine::{
    inspector::EvmInspector,
    state::BcState,
    types::{
        Address, Bytes, CallInputs, CreateInputs, EVMData, Gas, Inspector,
        InstructionResult,
    },
};

/// Contract inspector collects all contracts that are used in the txs
pub struct ContractInspector {
    pub contracts: HashSet<Address>,
}

impl<S: BcState> Inspector<S> for ContractInspector {
    fn call(
        &mut self,
        _data: &mut EVMData<'_, S>,
        inputs: &mut CallInputs,
    ) -> (InstructionResult, Gas, Bytes) {
        let contract = inputs.context.code_address;
        self.contracts.insert(contract);
        (InstructionResult::Continue, Gas::new(0), Bytes::new())
    }

    fn create_end(
        &mut self,
        _data: &mut EVMData<'_, S>,
        _inputs: &CreateInputs,
        ret: InstructionResult,
        address: Option<Address>,
        remaining_gas: Gas,
        out: Bytes,
    ) -> (InstructionResult, Option<Address>, Gas, Bytes) {
        if let Some(address) = address {
            self.contracts.insert(address);
        }
        (ret, address, remaining_gas, out)
    }
}

impl<S: BcState> EvmInspector<S> for ContractInspector {}
