use derive_more::{AsMut, AsRef, Deref, DerefMut, From};
use revm_primitives::CfgEnv;

#[derive(
    Default,
    Debug,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    AsRef,
    Deref,
    DerefMut,
    AsMut,
)]
pub struct EngineConfig {
    #[deref]
    #[deref_mut]
    #[as_ref]
    #[as_mut]
    pub evm_cfg: CfgEnv,

    pub disable_nonce_check: bool,
}

impl From<CfgEnv> for EngineConfig {
    fn from(evm_cfg: CfgEnv) -> Self {
        Self {
            evm_cfg,
            ..Default::default()
        }
    }
}

impl EngineConfig {
    pub fn toggle_nonce_check(mut self, enable: bool) -> Self {
        self.disable_nonce_check = !enable;
        self
    }

    pub fn set_contract_code_size_limit(
        mut self,
        limit: Option<usize>,
    ) -> Self {
        self.evm_cfg.limit_contract_code_size = limit;
        self
    }

    pub fn set_memory_limit(mut self, limit: u64) -> Self {
        self.evm_cfg.memory_limit = limit;
        self
    }

    pub fn toggle_balance_check(mut self, enable: bool) -> Self {
        self.evm_cfg.disable_balance_check = !enable;
        self
    }

    pub fn toggle_block_gas_limit(mut self, enable: bool) -> Self {
        self.evm_cfg.disable_block_gas_limit = !enable;
        self
    }

    pub fn toggle_eip3607(mut self, enable: bool) -> Self {
        self.evm_cfg.disable_eip3607 = !enable;
        self
    }

    pub fn toggle_gas_refund(mut self, enable: bool) -> Self {
        self.evm_cfg.disable_gas_refund = !enable;
        self
    }

    pub fn toggle_base_fee(mut self, enable: bool) -> Self {
        self.evm_cfg.disable_base_fee = !enable;
        self
    }
}
