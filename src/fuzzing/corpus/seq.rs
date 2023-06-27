use libafl::prelude::{InMemoryCorpus, Input};
use serde::{Deserialize, Serialize};

use crate::engine::transactions::Tx;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TxSequenceInput(Vec<Tx>);

impl TxSequenceInput {
    pub fn to_txs(&self) -> Vec<Tx> {
        self.0.clone()
    }
}

impl Input for TxSequenceInput {
    fn generate_name(&self, idx: usize) -> String {
        format!("tx_seq_{}", idx)
    }
}

pub type TxSequenceCorpus = InMemoryCorpus<TxSequenceInput>;
