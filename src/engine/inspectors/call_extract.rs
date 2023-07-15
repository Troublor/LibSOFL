use reth_primitives::Address;
use revm::{Database, Inspector};
use revm_primitives::Bytes;

use crate::knowledge::contract::msg_call::{Creation, Invocation};

use super::MultiTxInspector;

#[derive(Debug, Default)]
pub struct CallExtractInspector {
    pub target_contract: Option<Address>,
    pub invocations: Vec<Invocation>,
    pub creations: Vec<Creation>,
}

impl CallExtractInspector {
    pub fn new(target_contract: Address) -> Self {
        Self {
            target_contract: Some(target_contract),
            invocations: Vec::new(),
            creations: Vec::new(),
        }
    }
}

impl<BS: Database> Inspector<BS> for CallExtractInspector {
    fn create_end(
        &mut self,
        _data: &mut revm::EVMData<'_, BS>,
        _inputs: &revm::interpreter::CreateInputs,
        ret: revm::interpreter::InstructionResult,
        address: Option<revm_primitives::B160>,
        remaining_gas: revm::interpreter::Gas,
        out: Bytes,
    ) -> (
        revm::interpreter::InstructionResult,
        Option<revm_primitives::B160>,
        revm::interpreter::Gas,
        Bytes,
    ) {
        if self.target_contract.is_none() || address == self.target_contract {
            let creation = Creation {
                creator: _inputs.caller,
                scheme: _inputs.scheme,
                value: _inputs.value,
                gas: remaining_gas,
                init_code: _inputs.init_code.clone(),
                contract: address,
                code: out.clone(),
                result: ret.into(),
            };
            self.creations.push(creation);
        }
        (ret, address, remaining_gas, out)
    }

    fn call_end(
        &mut self,
        _data: &mut revm::EVMData<'_, BS>,
        inputs: &revm::interpreter::CallInputs,
        remaining_gas: revm::interpreter::Gas,
        ret: revm::interpreter::InstructionResult,
        out: Bytes,
        _is_static: bool,
    ) -> (
        revm::interpreter::InstructionResult,
        revm::interpreter::Gas,
        Bytes,
    ) {
        if self.target_contract.is_none()
            || Some(inputs.contract) == self.target_contract
        {
            let invocation = Invocation {
                context: inputs.context.clone(),
                value: inputs.transfer.value,
                input: inputs.input.clone(),
                gas: remaining_gas,
                output: out.clone(),
                result: ret.into(),
            };
            self.invocations.push(invocation);
        }
        (ret, remaining_gas, out)
    }
}

impl<BS: Database> MultiTxInspector<BS> for CallExtractInspector {}

#[cfg(test)]
mod tests_with_dep {
    use reth_primitives::{Address, TxHash};

    use crate::{
        engine::state::{env::TransitionSpec, BcState, BcStateBuilder},
        utils::{
            conversion::{Convert, ToPrimitive},
            testing::get_testing_bc_provider,
        },
    };

    #[test]
    fn test_inverse_finance_attack() {
        // attack tx: 0x958236266991bc3fe3b77feaacea120f172c0708ad01c7a715b255f218f9313c
        let provider = get_testing_bc_provider();
        let state = BcStateBuilder::fork_at(&provider, 14972419).unwrap();
        let tx: TxHash = ToPrimitive::cvt("0x958236266991bc3fe3b77feaacea120f172c0708ad01c7a715b255f218f9313c");
        let spec = TransitionSpec::from_tx_hash(&provider, tx).unwrap();
        let lending_pool: Address =
            ToPrimitive::cvt("0x7Fcb7DAC61eE35b3D4a51117A7c58D53f0a8a670");
        let mut inspector = super::CallExtractInspector::new(lending_pool);
        BcState::transit(state, spec, &mut inspector).unwrap();
        let calls = inspector.invocations;
        assert_eq!(calls.len(), 3);
    }
}
