use reth_primitives::Address;
use revm_primitives::{BlockEnv, Bytes, ExecutionResult, Output};

use crate::{
    engine::{inspectors::no_inspector, transactions::builder::TxBuilder},
    error::SoflError,
};

use super::{config::EngineConfig, state::BcState};

#[derive(Debug, Clone, Default)]
pub struct HighLevelCaller {
    pub address: Address,
    pub nonce: u64,
    pub cfg: EngineConfig,
    pub block: BlockEnv,
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

    pub fn set_cfg(mut self, cfg: EngineConfig) -> Self {
        self.cfg = cfg;
        self
    }

    pub fn set_block(mut self, block: BlockEnv) -> Self {
        self.block = block;
        self
    }

    pub fn bypass_check(mut self) -> Self {
        self.cfg = self
            .cfg
            .toggle_nonce_check(false)
            .toggle_balance_check(false)
            .toggle_base_fee(false)
            .toggle_block_gas_limit(false)
            .toggle_eip3607(false)
            .toggle_base_fee(false);
        self
    }
}

impl HighLevelCaller {
    pub fn static_call<BS: BcState>(
        &self,
        state: &mut BS,
        callee: Address,
        calldata: &[u8],
    ) -> Result<Bytes, SoflError<BS::DbErr>> {
        let tx = TxBuilder::new()
            .set_from(self.address)
            .set_to(callee)
            .set_input(calldata)
            .build();
        let out = state.transact(
            self.cfg.clone(),
            self.block.clone(),
            tx,
            no_inspector(),
        )?;
        match out.result {
            ExecutionResult::Success { output, .. } => {
                let Output::Call(ret) = output else {
                    panic!("should not happen since `tx.to` is set")
                };
                Ok(ret)
            }
            _ => Err(SoflError::Exec(out.result)),
        }
    }

    pub fn call<BS: BcState>(
        &self,
        state: &mut BS,
        callee: Address,
        calldata: &[u8],
    ) -> Result<Bytes, SoflError<BS::DbErr>> {
        let tx = TxBuilder::new()
            .set_from(self.address)
            .set_to(callee)
            .set_input(calldata)
            .build();
        let out = state.transact(
            self.cfg.clone(),
            self.block.clone(),
            tx,
            no_inspector(),
        )?;
        match out.result {
            ExecutionResult::Success { output, .. } => {
                let Output::Call(ret) = output else {
                    panic!("should not happen since `tx.to` is set")
                };
                state.commit(out.state);
                Ok(ret)
            }
            _ => Err(SoflError::Exec(out.result)),
        }
    }

    pub fn view<BS: BcState>(
        &self,
        state: &mut BS,
        callee: Address,
        func: &ethers::abi::Function,
        args: &[ethers::abi::Token],
    ) -> Result<Vec<ethers::abi::Token>, SoflError<BS::DbErr>> {
        let calldata = func.encode_input(args).map_err(SoflError::Abi)?;
        let ret = self.static_call(state, callee, &calldata)?;
        func.decode_output(ret.to_vec().as_slice())
            .map_err(SoflError::Abi)
    }

    pub fn invoke<BS: BcState>(
        &self,
        state: &mut BS,
        callee: Address,
        func: &ethers::abi::Function,
        args: &[ethers::abi::Token],
    ) -> Result<Vec<ethers::abi::Token>, SoflError<BS::DbErr>> {
        let calldata = func.encode_input(args).map_err(SoflError::Abi)?;
        let ret = self.call(state, callee, &calldata)?;
        func.decode_output(ret.to_vec().as_slice())
            .map_err(SoflError::Abi)
    }
}
