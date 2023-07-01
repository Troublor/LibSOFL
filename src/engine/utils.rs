use reth_primitives::Address;
use revm::{Database, DatabaseCommit};
use revm_primitives::{BlockEnv, Bytes, CfgEnv, ExecutionResult, Output, U256};

use crate::{engine::transactions::builder::TxBuilder, error::SoflError};

use super::{
    inspectors::{static_call::StaticCallEnforceInspector, MultiTxInspector},
    state::{env::TransitionSpecBuilder, state::BcState},
};

#[derive(Debug, Clone, Default)]
pub struct HighLevelCaller {
    pub address: Address,
    pub nonce: u64,
    pub gas_limit: u64,
    pub spec_builder: TransitionSpecBuilder,
}

impl From<Address> for HighLevelCaller {
    fn from(address: Address) -> Self {
        Self::new(address)
    }
}

impl HighLevelCaller {
    pub fn new(address: Address) -> Self {
        Self {
            address,
            ..Default::default()
        }
    }

    pub fn set_nonce(mut self, nonce: u64) -> Self {
        self.nonce = nonce;
        self
    }

    pub fn set_cfg(mut self, cfg: CfgEnv) -> Self {
        self.spec_builder = self.spec_builder.set_cfg(cfg);
        self
    }

    pub fn set_block(mut self, block: BlockEnv) -> Self {
        self.spec_builder = self.spec_builder.set_block(block);
        self
    }

    pub fn set_gas_limit(mut self, gas_limit: u64) -> Self {
        self.gas_limit = gas_limit;
        self
    }

    pub fn bypass_check(mut self) -> Self {
        self.spec_builder = self.spec_builder.bypass_check();
        self.set_gas_limit(u64::MAX)
    }
}

impl HighLevelCaller {
    pub fn static_call<
        'a,
        BS: Database + DatabaseCommit,
        I: MultiTxInspector<&'a mut BS>,
    >(
        &self,
        state: &'a mut BS,
        callee: Address,
        calldata: &[u8],
        inspector: &mut I,
    ) -> Result<Bytes, SoflError<BS::Error>> {
        let tx = TxBuilder::new()
            .set_from(self.address)
            .set_to(callee)
            .set_input(calldata)
            .build();
        let spec = self.spec_builder.clone().append_tx(tx.from(), tx).build();
        let (_, mut result) = BcState::transit(
            state,
            spec,
            &mut (&mut StaticCallEnforceInspector::default(), inspector),
        )?;
        let result = result.pop().unwrap();
        match result {
            ExecutionResult::Success { output, .. } => {
                let Output::Call(ret) = output else {
                    panic!("should not happen since `tx.to` is set")
                };
                Ok(ret)
            }
            _ => Err(SoflError::Exec(result)),
        }
    }

    pub fn call<
        'a,
        BS: Database + DatabaseCommit,
        I: MultiTxInspector<&'a mut BS>,
    >(
        &self,
        state: &'a mut BS,
        callee: Address,
        calldata: &[u8],
        value: Option<U256>,
        inspector: &mut I,
    ) -> Result<Bytes, SoflError<BS::Error>> {
        let tx = TxBuilder::new()
            .set_from(self.address)
            .set_to(callee)
            .set_input(calldata)
            .set_gas_limit(self.gas_limit)
            .set_value(value.unwrap_or(U256::default()))
            .build();
        let spec = self.spec_builder.clone().append_tx(tx.from(), tx).build();
        let (_, mut result) = BcState::transit(state, spec, inspector)?;
        let result = result.pop().unwrap();
        match result {
            ExecutionResult::Success { output, .. } => {
                let Output::Call(ret) = output else {
                    panic!("should not happen since `tx.to` is set")
                };
                Ok(ret)
            }
            _ => Err(SoflError::Exec(result)),
        }
    }

    pub fn view<
        'a,
        BS: Database + DatabaseCommit,
        I: MultiTxInspector<&'a mut BS>,
    >(
        &self,
        state: &'a mut BS,
        callee: Address,
        func: &ethers::abi::Function,
        args: &[ethers::abi::Token],
        inspector: &mut I,
    ) -> Result<Vec<ethers::abi::Token>, SoflError<BS::Error>> {
        let calldata = func.encode_input(args).map_err(SoflError::Abi)?;
        let ret = self.static_call(state, callee, &calldata, inspector)?;
        func.decode_output(ret.to_vec().as_slice())
            .map_err(SoflError::Abi)
    }

    pub fn invoke<
        'a,
        BS: Database + DatabaseCommit,
        I: MultiTxInspector<&'a mut BS>,
    >(
        &self,
        state: &'a mut BS,
        callee: Address,
        func: &ethers::abi::Function,
        args: &[ethers::abi::Token],
        value: Option<U256>,
        inspector: &mut I,
    ) -> Result<Vec<ethers::abi::Token>, SoflError<BS::Error>> {
        let calldata = func.encode_input(args).map_err(SoflError::Abi)?;
        let ret = self.call(state, callee, &calldata, value, inspector)?;
        func.decode_output(ret.to_vec().as_slice())
            .map_err(SoflError::Abi)
    }
}
