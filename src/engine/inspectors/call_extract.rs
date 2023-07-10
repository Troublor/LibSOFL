use reth_primitives::Address;
use revm::{
    interpreter::{instruction_result::SuccessOrHalt, CallContext},
    Database, Inspector,
};
use revm_primitives::{Bytes, U256};

use super::MultiTxInspector;

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

#[derive(Debug)]
pub struct CallExtractInspector {
    pub target_contract: Address,
    pub calls: Vec<MsgCall>,

    call_stack: Vec<Option<MsgCall>>,
}

impl CallExtractInspector {
    pub fn new(target_contract: Address) -> Self {
        Self {
            target_contract,
            calls: Vec::new(),
            call_stack: Vec::new(),
        }
    }
}

impl<BS: Database> Inspector<BS> for CallExtractInspector {
    fn call(
        &mut self,
        _data: &mut revm::EVMData<'_, BS>,
        inputs: &mut revm::interpreter::CallInputs,
        _is_static: bool,
    ) -> (
        revm::interpreter::InstructionResult,
        revm::interpreter::Gas,
        revm_primitives::Bytes,
    ) {
        if inputs.contract == self.target_contract {
            self.call_stack.push(Some(MsgCall {
                context: inputs.context.clone(),
                value: inputs.transfer.value,
                gas_limit: inputs.gas_limit,
                input: inputs.input.clone(),
                ..Default::default()
            }));
        } else {
            self.call_stack.push(None);
        }
        (
            revm::interpreter::InstructionResult::Continue,
            revm::interpreter::Gas::new(0),
            revm_primitives::Bytes::new(),
        )
    }

    fn call_end(
        &mut self,
        _data: &mut revm::EVMData<'_, BS>,
        _inputs: &revm::interpreter::CallInputs,
        remaining_gas: revm::interpreter::Gas,
        ret: revm::interpreter::InstructionResult,
        out: Bytes,
        _is_static: bool,
    ) -> (
        revm::interpreter::InstructionResult,
        revm::interpreter::Gas,
        Bytes,
    ) {
        let maybe_call = self
            .call_stack
            .pop()
            .expect("bug: call stack is unexpected empty");
        if let Some(mut call) = maybe_call {
            call.output = out.clone();
            call.result = ret.into();
            self.calls.push(call);
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
        let tx : TxHash= ToPrimitive::cvt("0x958236266991bc3fe3b77feaacea120f172c0708ad01c7a715b255f218f9313c");
        let spec = TransitionSpec::from_tx_hash(&provider, tx).unwrap();
        let lending_pool: Address =
            ToPrimitive::cvt("0x7Fcb7DAC61eE35b3D4a51117A7c58D53f0a8a670");
        let mut inspector = super::CallExtractInspector::new(lending_pool);
        BcState::transit(state, spec, &mut inspector).unwrap();
        let calls = inspector.calls;
        assert_eq!(calls.len(), 3);
    }
}
