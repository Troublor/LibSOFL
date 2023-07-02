use reth_primitives::{
    Address, BlockHashOrNumber, Transaction, TransactionSigned, TxHash,
};
use reth_provider::{EvmEnvProvider, TransactionsProvider};
use reth_revm::env::fill_tx_env;
use revm_primitives::{BlockEnv, CfgEnv, TxEnv};

use crate::{engine::transactions::position::TxPosition, error::SoflError};

#[derive(Default, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct TransitionSpec {
    pub cfg: CfgEnv,
    pub block: BlockEnv,
    pub txs: Vec<TxEnv>,
}

impl TransitionSpec {
    pub fn from_tx_position<P: EvmEnvProvider + TransactionsProvider>(
        p: &P,
        pos: impl Into<TxPosition>,
    ) -> Result<Self, SoflError> {
        let mut this = TransitionSpec::default();
        let pos = pos.into();
        p.fill_env_at(&mut this.cfg, &mut this.block, pos.block)
            .map_err(SoflError::Reth)?;
        let txs = p
            .transactions_by_block(pos.block)
            .map_err(SoflError::Reth)?
            .ok_or(SoflError::Fork(pos))?;
        let tx = txs.get(pos.index as usize).ok_or(SoflError::Fork(pos))?;
        let sender = tx.recover_signer().expect(
            "impossible: cannot recover signer from a signed transaction",
        );
        let mut tx_env = TxEnv::default();
        fill_tx_env(&mut tx_env, tx, sender);
        this.txs.push(tx_env);
        Ok(this)
    }

    pub fn from_tx_hash<P: EvmEnvProvider + TransactionsProvider>(
        p: &P,
        hash: TxHash,
    ) -> Result<Self, SoflError> {
        let mut this = TransitionSpec::default();
        let (tx, meta) = p
            .transaction_by_hash_with_meta(hash)
            .map_err(SoflError::Reth)?
            .ok_or(SoflError::Custom(format!(
                "transaction with hash {} not found",
                hash
            )))?;
        p.fill_env_at(&mut this.cfg, &mut this.block, meta.block_number.into())
            .map_err(SoflError::Reth)?;
        let sender = tx.recover_signer().expect(
            "impossible: cannot recover signer from a signed transaction",
        );
        let mut tx_env = TxEnv::default();
        fill_tx_env(&mut tx_env, tx, sender);
        this.txs.push(tx_env);
        Ok(this)
    }
}

#[derive(Default, Clone, Debug)]
pub struct TransitionSpecBuilder {
    cfg: CfgEnv,
    block: BlockEnv,
    txs: Vec<TxEnv>,
    disable_nonce_check: bool,
}

impl TransitionSpecBuilder {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TransitionSpecBuilder {
    pub fn build(self) -> TransitionSpec {
        TransitionSpec {
            cfg: self.cfg,
            block: self.block,
            txs: self.txs,
        }
    }

    pub fn append_tx_env(mut self, mut tx_env: TxEnv) -> Self {
        if self.disable_nonce_check {
            tx_env.nonce = None;
        }
        self.txs.push(tx_env);
        self
    }

    pub fn bypass_check(mut self) -> Self {
        self.cfg.disable_balance_check = true;
        self.cfg.disable_base_fee = true;
        self.cfg.disable_block_gas_limit = true;
        self.cfg.disable_eip3607 = true;
        self.disable_nonce_check = true;
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

    pub fn append_tx<T: AsRef<Transaction>>(
        self,
        from: Address,
        tx: T,
    ) -> Self {
        let mut tx_env = TxEnv::default();
        fill_tx_env(&mut tx_env, tx, from);
        self.append_tx_env(tx_env)
    }

    pub fn append_signed_tx<T: AsRef<TransactionSigned>>(self, tx: T) -> Self {
        let mut tx_env = TxEnv::default();
        fill_tx_env(
            &mut tx_env,
            tx.as_ref(),
            tx.as_ref().recover_signer().unwrap(),
        );
        self.append_tx_env(tx_env)
    }

    pub fn append_signed_txs(self, txs: Vec<TransactionSigned>) -> Self {
        let mut this = self;
        for tx in txs.into_iter() {
            this = this.append_signed_tx(tx);
        }
        this
    }

    pub fn at_block<P: EvmEnvProvider, B: Into<BlockHashOrNumber>>(
        mut self,
        p: P,
        block: B,
    ) -> Self {
        p.fill_env_at(&mut self.cfg, &mut self.block, block.into())
            .expect("assumption: block must exist");
        self
    }
}
