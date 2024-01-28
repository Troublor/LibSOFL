use revm_primitives::{BlockEnv, CfgEnv, SpecId, TxEnv};

use crate::{
    blockchain::{
        provider::BcProvider, transaction::Tx, tx_position::TxPosition,
    },
    conversion::ConvertTo,
    error::SoflError,
};

use super::types::{BlockHashOrNumber, Env, TxHash};

#[derive(Default, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct TransitionSpec {
    /// set to None to infer the evm version with chain_id and block number
    pub evm_version: Option<SpecId>,
    pub cfg: CfgEnv,
    pub block: BlockEnv,
    pub txs: Vec<TxEnv>,
}

impl TransitionSpec {
    pub fn from_tx_position<T: Tx, P: BcProvider<T>>(
        p: &P,
        pos: TxPosition,
    ) -> Result<Self, SoflError> {
        let mut this = TransitionSpec::default();

        p.fill_cfg_env(&mut this.cfg, pos.block)?;
        p.fill_block_env(&mut this.block, pos.block)?;
        let mut tx_env = TxEnv::default();
        p.fill_tx_env(&mut tx_env, pos.into())?;
        this.txs.push(tx_env);
        Ok(this)
    }

    pub fn from_tx_hash<T: Tx, P: BcProvider<T>>(
        p: &P,
        hash: TxHash,
    ) -> Result<Self, SoflError> {
        let mut this = TransitionSpec::default();

        let tx = p.tx(hash.into())?;
        let pos = tx.position().ok_or(SoflError::NotFound(format!(
            "transaction with hash {}",
            hash
        )))?;
        p.fill_cfg_env(&mut this.cfg, pos.block)?;
        p.fill_block_env(&mut this.block, pos.block)?;
        let mut tx_env = TxEnv::default();
        tx.fill_tx_env(&mut tx_env)?;
        this.txs.push(tx_env);
        Ok(this)
    }
}

pub fn get_evm_version(chain_id: u64, block_number: u64) -> SpecId {
    assert_eq!(chain_id, 1, "only mainnet is supported");
    let spec_id = match block_number {
        0..=199999 => SpecId::FRONTIER,
        200000..=1149999 => SpecId::FRONTIER_THAWING,
        1150000..=1919999 => SpecId::HOMESTEAD,
        1920000..=2462999 => SpecId::DAO_FORK,
        2463000..=2674999 => SpecId::TANGERINE,
        2675000..=4369999 => SpecId::SPURIOUS_DRAGON,
        4370000..=7279999 => SpecId::BYZANTIUM,
        // 7280000..9069000 => SpecId::CONSTANTINOPLE,
        7280000..=9068999 => SpecId::PETERSBURG,
        9069000..=9199999 => SpecId::ISTANBUL,
        9200000..=12243999 => SpecId::MUIR_GLACIER,
        12244000..=12964999 => SpecId::BERLIN,
        12965000..=13772999 => SpecId::LONDON,
        13773000..=15049999 => SpecId::ARROW_GLACIER,
        15050000..=15537393 => SpecId::GRAY_GLACIER,
        15537394..=17034869 => SpecId::MERGE,
        17034870.. => SpecId::SHANGHAI,
    };
    spec_id
}

impl TransitionSpec {
    pub fn get_evm_version(&self) -> SpecId {
        if let Some(evm_version) = self.evm_version {
            evm_version
        } else {
            let bn = ConvertTo::<u64>::cvt(&self.block.number);
            get_evm_version(self.cfg.chain_id, bn)
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct TransitionSpecBuilder {
    evm_version: Option<SpecId>,
    cfg: CfgEnv,
    block: BlockEnv,
    txs: Vec<TxEnv>,
    bypass_check: bool,
}

impl TransitionSpecBuilder {
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<TransitionSpec> for Vec<Env> {
    fn from(spec: TransitionSpec) -> Self {
        let mut envs = Vec::new();
        for tx in spec.txs.into_iter() {
            let mut env = Env::default();
            env.cfg = spec.cfg.clone();
            env.block = spec.block.clone();
            env.tx = tx;
            envs.push(env);
        }
        envs
    }
}

impl TransitionSpecBuilder {
    pub fn build(mut self) -> TransitionSpec {
        if self.bypass_check {
            self.cfg.disable_balance_check = true;
            self.cfg.disable_base_fee = true;
            self.cfg.disable_block_gas_limit = true;
            self.cfg.disable_eip3607 = true;
            self.txs.iter_mut().for_each(|tx| {
                tx.nonce = None;
            });
        }
        TransitionSpec {
            evm_version: self.evm_version,
            cfg: self.cfg,
            block: self.block,
            txs: self.txs,
        }
    }

    pub fn set_evm_version(mut self, evm_version: SpecId) -> Self {
        self.evm_version.replace(evm_version);
        self
    }

    pub fn append_tx_env(mut self, tx_env: TxEnv) -> Self {
        // if self.disable_nonce_check {
        //     tx_env.nonce = None;
        // }
        self.txs.push(tx_env);
        self
    }

    pub fn bypass_check(mut self) -> Self {
        self.bypass_check = true;
        self
    }

    pub fn set_cfg(mut self, cfg: CfgEnv) -> Self {
        self.cfg = cfg;
        self
    }

    pub fn set_block(mut self, block: BlockEnv) -> Self {
        self.block = block;
        self
    }

    pub fn append_tx<T: Tx>(self, tx: T) -> Self {
        let mut tx_env = TxEnv::default();
        tx.fill_tx_env(&mut tx_env)
            .expect("assumption: tx must be valid");
        self.append_tx_env(tx_env)
    }

    pub fn at_block<T: Tx, P: BcProvider<T>>(
        mut self,
        p: P,
        block: BlockHashOrNumber,
    ) -> Self {
        p.fill_cfg_env(&mut self.cfg, block)
            .expect("assumption: chain cfg must exist");
        p.fill_block_env(&mut self.block, block)
            .expect("assumption: block must exist");
        self
    }
}
