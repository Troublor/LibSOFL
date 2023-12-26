use libsofl_core::engine::{
    inspector::EvmInspector,
    state::BcState,
    types::{
        Address, Bytes, CallInputs, EVMData, Gas, Inspector, InstructionResult,
    },
};

#[derive(Default)]
pub struct ExtractInvocationInspector {
    pub invocations: Vec<Address>, // invoked code addresses ordered
}

impl<BS: BcState> Inspector<BS> for ExtractInvocationInspector {
    fn call(
        &mut self,
        _data: &mut EVMData<'_, BS>,
        inputs: &mut CallInputs,
    ) -> (InstructionResult, Gas, Bytes) {
        let addr = inputs.context.code_address;
        self.invocations.push(addr);
        (InstructionResult::Continue, Gas::new(0), Bytes::new())
    }
}

impl<BS: BcState> EvmInspector<BS> for ExtractInvocationInspector {}
