use std::{ops::RangeBounds, sync::Arc};

use libafl::{
    prelude::{
        self, current_nanos, InMemoryCorpus, RomuDuoJrRand, SimpleEventManager,
        SimpleMonitor, StdRand,
    },
    schedulers::QueueScheduler,
    state::StdState,
    StdFuzzer,
};
use reth_interfaces::Result as rethResult;
use reth_primitives::{Address, Block, BlockNumber, TransactionSigned};

use crate::{
    engine::{
        self,
        executor::{Executor, NoInspector},
        providers::BcProvider,
        transaction::{self, PortableTx, Tx, TxPosition},
    },
    utils::conversion::{Convert, ToIterator},
};

/// Price oracle manipulation vulnerability fuzzer

pub struct POMFuzzer<P> {
    provider: P,
}

impl<P> POMFuzzer<P> {
    pub fn new(provider: P) -> Self {
        POMFuzzer { provider }
    }
}

impl<P: BcProvider> POMFuzzer<P> {
    /// Entry point of fuzzing one subject contract.
    pub fn fuzz(&self, contract: Address) -> rethResult<()> {
        todo!()
    }

    pub fn manipulate<S>(
        &self,
        executor: &Executor<S>,
        oracle: Address,
    ) -> rethResult<()> {
        todo!()
    }
}

impl<P: BcProvider> POMFuzzer<P> {
    pub fn get_historical_txs(
        &self,
        contract: Address,
        period: impl RangeBounds<BlockNumber>,
    ) -> rethResult<Vec<TransactionSigned>> {
        let mut blocks: Vec<Block> = Vec::new();
        for bn in ToIterator::cvt(period) {
            let block = self.provider.block_by_number(bn)?;
            if let Some(b) = block {
                blocks.push(b);
            }
        }
        Ok(blocks
            .iter()
            .flat_map(|b| b.body.clone())
            .filter(|tx| tx.to() == Some(contract))
            .collect())
    }
}
