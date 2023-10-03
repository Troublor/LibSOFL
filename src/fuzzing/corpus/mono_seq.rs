use std::fmt::Debug;

use revm_primitives::{BlockEnv, CfgEnv, TxEnv};
use serde::{Deserialize, Serialize};

use crate::fuzzing::interfaces::BcState;

/// An fuzzing input with a fixed execution environment (EVM cfg + block spec + pre-execution blockchain state) and a transaction sequence.
/// MonoEnvTxSeqInput implements the Input trait of libafl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonoEnvTxSeqInput<BS> {
    pub evm: CfgEnv,
    pub block: BlockEnv,
    pub state: BS,
    pub txs: Vec<TxEnv>,
}

impl<BS> MonoEnvTxSeqInput<BS> {
    pub fn new(evm: CfgEnv, block: BlockEnv, state: BS) -> Self {
        Self {
            evm,
            block,
            state,
            txs: vec![],
        }
    }
}

impl<BS> MonoEnvTxSeqInput<BS> {
    pub fn append_tx(&mut self, tx: TxEnv) -> &mut Self {
        self.txs.push(tx);
        self
    }
}

impl<BS: BcState> libafl::inputs::Input for MonoEnvTxSeqInput<BS> {
    fn generate_name(&self, _idx: usize) -> String {
        todo!()
    }
}
