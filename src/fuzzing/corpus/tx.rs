use libafl::prelude::{InMemoryCorpus, Input};

use crate::engine::transaction::PortableTx;

pub type TxInput = PortableTx;

impl Input for TxInput {
    fn generate_name(&self, idx: usize) -> String {
        format!("tx_{}", idx)
    }
}

pub type TxCorpus = InMemoryCorpus<TxInput>;
