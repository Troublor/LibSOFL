pub use alloy_dyn_abi::{DynSolEvent, DynSolType};
use alloy_dyn_abi::{DynSolValue, FunctionExt, JsonAbiExt};
use alloy_json_abi::Function;
use alloy_sol_types::{Revert, SolError};
use libsofl_core::{
    blockchain::{provider::BcProvider, transaction::Tx},
    conversion::ConvertTo,
    engine::{
        inspector::EvmInspector,
        state::BcState,
        transition::TransitionSpecBuilder,
        types::{
            Address, BlockEnv, BlockHashOrNumber, Bytes, CfgEnv, CreateScheme,
            ExecutionResult, Output, StateChange, TransactTo, TxEnv, U256,
        },
    },
    error::SoflError,
};

/// HighLevelCaller provider a high level interface for calling contract.
/// HighLevelCaller is readonly caller, which means it can not change the state.
/// All calls are simulations.
#[derive(Debug, Clone)]
pub struct HighLevelCaller {
    pub address: Address,
    pub nonce: u64,
    pub gas_limit: u64,
    pub spec_builder: TransitionSpecBuilder,
}

impl Default for HighLevelCaller {
    fn default() -> Self {
        Self {
            address: "0x4354bB7C9dad5b0299199c0084E6ae386afD636C".cvt(),
            nonce: 0,
            gas_limit: 0,
            spec_builder: TransitionSpecBuilder::default(),
        }
    }
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

    pub fn set_address(mut self, address: Address) -> Self {
        self.address = address;
        self
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

    pub fn at_block<T: Tx, P: BcProvider<T>, B: Into<BlockHashOrNumber>>(
        mut self,
        p: P,
        block: B,
    ) -> Self {
        self.spec_builder = self.spec_builder.at_block(p, block.into());
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
    /// Call a contract with static call with low-level calldata.
    /// State will not be changed.
    pub fn static_call<'a, BS: BcState, I: EvmInspector<&'a mut BS>>(
        &self,
        state: &'a mut BS,
        callee: Address,
        calldata: Bytes,
        inspector: &mut I,
    ) -> Result<Bytes, SoflError>
    where
        BS::Error: std::fmt::Debug,
    {
        let mut tx = TxEnv::default();
        tx.caller = self.address;
        tx.transact_to = TransactTo::Call(callee);
        tx.gas_limit = self.gas_limit;
        tx.data = calldata;
        let spec = self.spec_builder.clone().append_tx_env(tx).build();

        let (_, mut result) = state.simulate(spec, inspector)?;
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

    /// Create a contract with low-level calldata.
    /// State will not be changed, but StateChange will be returned,
    pub fn create<'a, BS: BcState, I: EvmInspector<&'a mut BS>>(
        &self,
        state: &'a mut BS,
        salt: Option<U256>,
        calldata: &[u8],
        value: Option<U256>,
        inspector: &mut I,
    ) -> Result<(Bytes, Option<Address>), SoflError>
    where
        BS::Error: std::fmt::Debug,
    {
        let mut tx = TxEnv::default();
        tx.caller = self.address;
        tx.transact_to = TransactTo::Create(
            salt.map(|s| CreateScheme::Create2 { salt: s })
                .unwrap_or(CreateScheme::Create),
        );
        tx.data = calldata.cvt();
        tx.gas_limit = self.gas_limit;
        tx.value = value.unwrap_or(U256::default());

        let spec = self.spec_builder.clone().append_tx_env(tx).build();
        let mut result = state.transit(spec, inspector)?;

        let result = result.pop().unwrap();
        match result {
            ExecutionResult::Success { output, .. } => {
                let Output::Create(bytes, addr) = output else {
                    panic!("should not happen since `tx.to` is set")
                };
                Ok((bytes, addr))
            }
            _ => Err(SoflError::Exec(result)),
        }
    }

    /// Create a contract with low-level calldata.
    /// State will not be changed, but StateChange will be returned,
    /// which can be committed to the State using `BcState::commit` method.
    pub fn simulate_create<'a, BS: BcState, I: EvmInspector<&'a mut BS>>(
        &self,
        state: &'a mut BS,
        salt: Option<U256>,
        calldata: &[u8],
        value: Option<U256>,
        inspector: &mut I,
    ) -> Result<(Bytes, Option<Address>, StateChange), SoflError>
    where
        BS::Error: std::fmt::Debug,
    {
        let mut tx = TxEnv::default();
        tx.caller = self.address;
        tx.transact_to = TransactTo::Create(
            salt.map(|s| CreateScheme::Create2 { salt: s })
                .unwrap_or(CreateScheme::Create),
        );
        tx.data = calldata.cvt();
        tx.gas_limit = self.gas_limit;
        tx.value = value.unwrap_or(U256::default());

        let spec = self.spec_builder.clone().append_tx_env(tx).build();
        let (mut changes, mut result) = state.simulate(spec, inspector)?;
        let change = changes.pop().unwrap();

        let result = result.pop().unwrap();
        match result {
            ExecutionResult::Success { output, .. } => {
                let Output::Create(bytes, addr) = output else {
                    panic!("should not happen since `tx.to` is set")
                };
                Ok((bytes, addr, change))
            }
            _ => Err(SoflError::Exec(result)),
        }
    }

    pub fn call<'a, BS: BcState, I: EvmInspector<&'a mut BS>>(
        &self,
        state: &'a mut BS,
        callee: Address,
        calldata: Bytes,
        value: Option<U256>,
        inspector: &mut I,
    ) -> Result<Bytes, SoflError>
    where
        BS::Error: std::fmt::Debug,
    {
        let mut tx = TxEnv::default();
        tx.caller = self.address;
        tx.transact_to = TransactTo::Call(callee);
        tx.data = calldata;
        tx.gas_limit = self.gas_limit;
        tx.value = value.unwrap_or(U256::default());

        let spec = self.spec_builder.clone().append_tx_env(tx).build();
        let mut result = state.transit(spec, inspector)?;
        let result = result.pop().unwrap();
        match result.clone() {
            ExecutionResult::Success { output, .. } => {
                let Output::Call(ret) = output else {
                    panic!("should not happen since `tx.to` is set")
                };
                Ok(ret)
            }
            ExecutionResult::Revert {
                gas_used: _,
                output,
            } => {
                let x = Revert::abi_decode(&output, false).unwrap();
                println!("revert: {:?}", x.reason);
                Err(SoflError::Exec(result))
            }
            _ => Err(SoflError::Exec(result)),
        }
    }

    /// Call a contract with low-level calldata.
    /// State will not be changed, but StateChange will be returned,
    /// which can be committed to the State using `BcState::commit` method.
    pub fn simulate_call<'a, BS: BcState, I: EvmInspector<&'a mut BS>>(
        &self,
        state: &'a mut BS,
        callee: Address,
        calldata: Bytes,
        value: Option<U256>,
        inspector: &mut I,
    ) -> Result<(Bytes, StateChange), SoflError>
    where
        BS::Error: std::fmt::Debug,
    {
        let mut tx = TxEnv::default();
        tx.caller = self.address;
        tx.transact_to = TransactTo::Call(callee);
        tx.data = calldata;
        tx.gas_limit = self.gas_limit;
        tx.value = value.unwrap_or(U256::default());

        let spec = self.spec_builder.clone().append_tx_env(tx).build();
        let (mut changes, mut result) = state.simulate(spec, inspector)?;
        let result = result.pop().unwrap();
        let change = changes.pop().unwrap();
        match result.clone() {
            ExecutionResult::Success { output, .. } => {
                let Output::Call(ret) = output else {
                    panic!("should not happen since `tx.to` is set")
                };
                Ok((ret, change))
            }
            ExecutionResult::Revert {
                gas_used: _,
                output,
            } => {
                let x = Revert::abi_decode(&output, false).unwrap();
                println!("revert: {:?}", x.reason);
                Err(SoflError::Exec(result))
            }
            _ => Err(SoflError::Exec(result)),
        }
    }

    pub fn view<'a, BS: BcState, I: EvmInspector<&'a mut BS>>(
        &self,
        state: &'a mut BS,
        callee: Address,
        func: &str,
        args: &[DynSolValue],
        inspector: &mut I,
    ) -> Result<Vec<DynSolValue>, SoflError> {
        let f = Function::parse(func)
            .map_err(|e| SoflError::Abi(format!("{:?}", e)))?;
        let calldata = f
            .abi_encode_input(args)
            .map_err(|e| SoflError::Abi(format!("{:?}", e)))?;
        let ret = self.static_call(state, callee, calldata.cvt(), inspector)?;
        f.abi_decode_output(&ret, true)
            .map_err(|e| SoflError::Abi(format!("{:?}", e)))
    }

    pub fn invoke<'a, BS: BcState, I: EvmInspector<&'a mut BS>>(
        &self,
        state: &'a mut BS,
        callee: Address,
        func: &str,
        args: &[DynSolValue],
        value: Option<U256>,
        inspector: &mut I,
    ) -> Result<Vec<DynSolValue>, SoflError> {
        let f = Function::parse(func)
            .map_err(|e| SoflError::Abi(format!("{:?}", e)))?;
        let calldata = f
            .abi_encode_input(args)
            .map_err(|e| SoflError::Abi(format!("{:?}", e)))?;
        let ret = self.call(state, callee, calldata.cvt(), value, inspector)?;
        f.abi_decode_output(&ret, true)
            .map_err(|e| SoflError::Abi(format!("{:?}", e)))
    }
}
