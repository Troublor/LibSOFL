use libafl::prelude::Generator;
use reth_primitives::{Address, BlockNumber};
use reth_provider::TransactionsProvider;

use crate::{
    engine::{
        providers::BcProvider,
        transaction::{PortableTx, Tx},
    },
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
    bn: BlockNumber,
    txs: Vec<PortableTx>,
}

impl<P> HistoricalTxGenerator<P> {
    pub fn new(provider: P, contract: Address, from: BlockNumber) -> Self {
        Self {
            provider,
            contract,
            bn: from + 1,
            txs: Vec::new(),
        }
    }
}

impl<S, P: TransactionsProvider> Generator<TxInput, S>
    for HistoricalTxGenerator<P>
{
    fn generate(&mut self, _state: &mut S) -> Result<TxInput, libafl::Error> {
        loop {
            if let Some(tx) = self.txs.pop() {
                if tx.to() == Some(self.contract) {
                    return Ok(TxInput::from(Tx::from(tx)));
                }
            } else {
                // fetch previous block
                self.bn = self.bn - 1;
                self.txs = self
                    .provider
                    .transactions_by_block(self.bn.into())
                    .map_err(|e| {
                        libafl::Error::Unknown(
                            format!("failed to fetch txs: {:?}", e),
                            libafl::ErrorBacktrace::default(),
                        )
                    })?
                    .ok_or(libafl::Error::Unknown(
                        format!("failed to fetch txs: {:?}", self.bn),
                        libafl::ErrorBacktrace::default(),
                    ))?
                    .into_iter()
                    .map(|tx| tx.into())
                    .collect();
            }
        }
    }
}

#[cfg(test)]
mod tests_with_jsonrpc {
    use libafl::prelude::Generator;

    use crate::{
        engine::providers::rpc::JsonRpcBcProvider,
        utils::conversion::{Convert, ToPrimitive},
    };

    #[test]
    fn test_generate_tx_across_blocks() {
        let provider = JsonRpcBcProvider::default();
        let wtf_token =
            ToPrimitive::cvt("0xA68Dd8cB83097765263AdAD881Af6eeD479c4a33");
        let mut generator =
            super::HistoricalTxGenerator::new(provider, wtf_token, 14000001);
        let tx = generator.generate(&mut ()).unwrap();
        assert_eq!(tx.hash(), ToPrimitive::cvt("0x4ff24890c147efeb3cd57db2e93eff7e7ed6c7466fbf39aac595d17f1045bca2"));
        let tx = generator.generate(&mut ()).unwrap();
        assert_eq!(tx.hash(), ToPrimitive::cvt("0x28e7d8a981c7d62c267068366bfeff7f3e5cd6cb5b5da3d5bd873f22d04aedf3"));
        let tx = generator.generate(&mut ()).unwrap();
        assert_eq!(tx.hash(), ToPrimitive::cvt("0x7c6b5ed5afbc91853c3b012ab243793b47b653137bbe465d5e6daef49ef6b702"));
        // block break
        let tx = generator.generate(&mut ()).unwrap();
        assert_eq!(tx.hash(), ToPrimitive::cvt("0x77ae0fe18ac9b3147e2e7542f4d352039c37c7ff3751a31e2bc69e27f5d72f55"));
    }
}
