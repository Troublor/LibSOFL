use revm::{interpreter::opcode, Database, Inspector};
use revm_primitives::U256;

use super::MultiTxInspector;

/// An inspector that disallow any state change.
#[derive(Default)]
pub struct StaticCallEnforceInspector {}

impl<BS: Database> Inspector<BS> for StaticCallEnforceInspector {
    fn create(
        &mut self,
        _data: &mut revm::EVMData<'_, BS>,
        _inputs: &mut revm::interpreter::CreateInputs,
    ) -> (
        revm::interpreter::InstructionResult,
        Option<revm_primitives::B160>,
        revm::interpreter::Gas,
        revm_primitives::Bytes,
    ) {
        (
            revm::interpreter::InstructionResult::StateChangeDuringStaticCall,
            None,
            revm::interpreter::Gas::new(0),
            revm_primitives::Bytes::new(),
        )
    }

    fn call(
        &mut self,
        _data: &mut revm::EVMData<'_, BS>,
        _inputs: &mut revm::interpreter::CallInputs,
        _is_static: bool,
    ) -> (
        revm::interpreter::InstructionResult,
        revm::interpreter::Gas,
        revm_primitives::Bytes,
    ) {
        if _inputs.transfer.value != U256::ZERO {
            (
                revm::interpreter::InstructionResult::StateChangeDuringStaticCall,
                revm::interpreter::Gas::new(0),
                revm_primitives::Bytes::new(),
            )
        } else {
            (
                revm::interpreter::InstructionResult::Continue,
                revm::interpreter::Gas::new(0),
                revm_primitives::Bytes::new(),
            )
        }
    }

    fn step(
        &mut self,
        _interp: &mut revm::interpreter::Interpreter,
        _data: &mut revm::EVMData<'_, BS>,
        _is_static: bool,
    ) -> revm::interpreter::InstructionResult {
        if _interp.current_opcode() == opcode::SSTORE {
            revm::interpreter::InstructionResult::StateChangeDuringStaticCall
        } else {
            revm::interpreter::InstructionResult::Continue
        }
    }
}

impl<BS: Database> MultiTxInspector<BS> for StaticCallEnforceInspector {
    fn transaction(
        &mut self,
        _tx: &revm_primitives::TxEnv,
        _state: &BS,
    ) -> bool {
        _tx.value == U256::ZERO
    }
}
