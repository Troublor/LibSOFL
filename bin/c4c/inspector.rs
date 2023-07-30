use libsofl::engine::inspectors::MultiTxInspector;
use revm::{Database, Inspector};
use revm_primitives::{Address, Bytes};

#[derive(Debug, Default)]
pub struct InternalTransactionInspector {
    pub target_contract: Address,
    pub included: bool,
    pub index: usize,
    pub txs: Vec<usize>,
}

impl InternalTransactionInspector {
    pub fn new(target_contract: Address) -> Self {
        Self {
            target_contract,
            included: false,
            index: 0,
            txs: Vec::new(),
        }
    }
}

impl<BS: Database> Inspector<BS> for InternalTransactionInspector {
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
        if address == Some(self.target_contract) {
            self.included = true;
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
        if inputs.contract == self.target_contract {
            self.included = true;
        }
        (ret, remaining_gas, out)
    }
}

impl<BS: Database> MultiTxInspector<BS> for InternalTransactionInspector {
    fn transaction(
        &mut self,
        _tx: &revm_primitives::TxEnv,
        _state: &BS,
    ) -> bool {
        self.included = false;
        true
    }

    fn transaction_end(
        &mut self,
        _tx: &revm_primitives::TxEnv,
        _state: &BS,
        _result: &revm_primitives::ExecutionResult,
    ) {
        if self.included {
            self.txs.push(self.index);
        }
        self.index += 1;
    }
}
