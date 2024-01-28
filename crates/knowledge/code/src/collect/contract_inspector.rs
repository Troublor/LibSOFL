use std::collections::HashSet;

use libsofl_core::engine::{
    inspector::EvmInspector,
    state::BcState,
    types::{Address, Inspector},
};

/// Contract inspector collects all contracts that are used in the txs
pub struct ContractInspector {
    pub contracts: HashSet<Address>,
}

impl<S: BcState> Inspector<S> for ContractInspector {
    fn call(
        &mut self,
        _context: &mut libsofl_core::engine::types::EvmContext<S>,
        inputs: &mut libsofl_core::engine::types::CallInputs,
        _return_memory_offset: std::ops::Range<usize>,
    ) -> Option<libsofl_core::engine::types::CallOutcome> {
        let contract = inputs.context.code_address;
        self.contracts.insert(contract);
        None
    }

    fn create_end(
        &mut self,
        _context: &mut libsofl_core::engine::types::EvmContext<S>,
        _inputs: &libsofl_core::engine::types::CreateInputs,
        result: libsofl_core::engine::types::InterpreterResult,
        address: Option<libsofl_core::engine::types::Address>,
    ) -> libsofl_core::engine::types::CreateOutcome {
        if let Some(address) = address {
            self.contracts.insert(address);
        }
        libsofl_core::engine::types::CreateOutcome::new(result, address)
    }
}

impl<S: BcState> EvmInspector<S> for ContractInspector {}
