use std::{ops::RangeBounds, sync::Arc};

use reth_interfaces::Result as rethResult;
use reth_primitives::{Address, Block, BlockNumber, TransactionSigned};

use crate::{
    engine::{
        executor::{Executor, NoInspector},
        providers::BcProvider,
        transaction::{Tx, TxPosition},
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
        let latest = self.provider.last_block_number().unwrap();
        let txs = self
            .get_historical_txs(contract, latest - 100..latest)
            .unwrap();
        for tx in txs.iter() {
            let mut exe1 =
                Executor::fork_at(&self.provider, TxPosition::new(latest, 0))
                    .unwrap();
            let mut exe2 = exe1.clone();

            // no price manipulation
            let _ = exe1.transact::<NoInspector>(tx.into(), None).unwrap();

            // with price manipulation
            self.manipulate(&exe2, contract).unwrap();
            let _ = exe2.transact::<NoInspector>(tx.into(), None).unwrap();
        }
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
