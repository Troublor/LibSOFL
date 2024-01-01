use libsofl_core::engine::{
    inspector::EvmInspector,
    state::BcState,
    types::{
        Address, Bytes, CreateInputs, EVMData, Gas, Inspector,
        InstructionResult, U256,
    },
};

#[derive(Default)]
pub struct ExtractCreationInspector {
    pub created: Vec<(Address, bool)>, // (created address, whether destruct) ordered
}

impl<BS: BcState> Inspector<BS> for ExtractCreationInspector {
    fn create_end(
        &mut self,
        _data: &mut EVMData<'_, BS>,
        _inputs: &CreateInputs,
        ret: InstructionResult,
        address: Option<Address>,
        remaining_gas: Gas,
        out: Bytes,
    ) -> (InstructionResult, Option<Address>, Gas, Bytes) {
        let addr = match address {
            Some(addr) => addr,
            None => return (ret, address, remaining_gas, out),
        };
        match ret {
            InstructionResult::Continue
            | InstructionResult::Stop
            | InstructionResult::Return => {
                self.created.push((addr, false));
            }
            _ => {}
        }
        (ret, address, remaining_gas, out)
    }

    fn selfdestruct(
        &mut self,
        contract: Address,
        _target: Address,
        _value: U256,
    ) {
        self.created.push((contract, true));
    }
}

impl<BS: BcState> EvmInspector<BS> for ExtractCreationInspector {}

#[cfg(test)]
mod tests {
    use libsofl_core::engine::memory::MemoryBcState;
    use libsofl_utils::solidity::{
        caller::HighLevelCaller, scripting::compile_solidity,
    };

    #[test]
    fn test_extract_creation() {
        let mut state = MemoryBcState::fresh();
        let mut inspector = super::ExtractCreationInspector::default();

        let code = format!(
            r#"
            contract A {{
                constructor() {{}}
            }}
            contract B {{
                A a;
                constructor() {{
                    a = new A{{salt: bytes32(uint(0))}}();
                }}
            }}
            "#,
        );
        let (_, bytecode) = compile_solidity("0.8.12", code)
            .unwrap()
            .into_iter()
            .filter(|(n, _)| n == "B")
            .next()
            .unwrap();
        HighLevelCaller::default()
            .bypass_check()
            .create(&mut state, None, &bytecode, None, &mut inspector)
            .unwrap();

        let creations = inspector.created;
        assert_eq!(creations.len(), 2);
    }
}
