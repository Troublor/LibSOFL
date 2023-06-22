use libafl::prelude::Generator;
use reth_primitives::Address;

use crate::{
    engine::{providers::BcProvider, transaction::PortableTx},
    fuzzing::corpus::tx::TxInput,
};

/// Generate tx inputs from historical txs
/// Given a contract address, the generator will search back from the latest
/// block and find the transactions that call the contract as inputs.
pub struct HistoricalTxGenerator<P> {
    /// Blockchain provider
    provider: P,
    contract: Address,

    // cache
    bn: u64,
    txs: Vec<PortableTx>,
}

impl<S, P: BcProvider> Generator<TxInput, S> for HistoricalTxGenerator<P> {
    fn generate(&mut self, state: &mut S) -> Result<TxInput, libafl::Error> {
        todo!()
    }
}
