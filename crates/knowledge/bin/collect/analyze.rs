use std::ops::Range;

use libsofl_core::{
    blockchain::{
        provider::{BcProvider, BcStateProvider},
        transaction::Tx,
    },
    conversion::ConvertTo,
    engine::{
        inspector::CombinedInspector,
        state::BcState,
        transition::TransitionSpec,
        types::{BlockEnv, CfgEnv, DatabaseRef, TxEnv},
    },
    error::SoflError,
};
use libsofl_knowledge::inspectors::{
    extract_creation::ExtractCreationInspector,
    extract_invocation::ExtractInvocationInspector,
};
use libsofl_utils::log::{error, info};

pub(crate) struct Analyzer<
    T: Tx,
    S: DatabaseRef,
    P: BcProvider<T> + BcStateProvider<S>,
> {
    provider: P,
    creation_result_feed: tokio::sync::mpsc::Sender<(String, String, bool)>, // (tx, contract, destruct)
    invocation_result_feed: tokio::sync::mpsc::Sender<(u64, String)>, // (block number, contract)
    failed_block_feed: tokio::sync::mpsc::Sender<u64>,
    _phantom: std::marker::PhantomData<(T, S)>,
}

impl<T: Tx, S: DatabaseRef, P: BcProvider<T> + BcStateProvider<S>>
    Analyzer<T, S, P>
where
    S::Error: std::fmt::Debug,
{
    pub async fn analyze_blocks(
        &mut self,
        blocks: Range<u64>,
    ) -> Result<(), SoflError> {
        for block in blocks {
            let r = self.analyze_one_block(block).await;
            match r {
                Ok(_) => {}
                Err(e) => {
                    error!(
                        block = block,
                        error = format!("{:?}", e),
                        "failed to analyze block"
                    );
                    self.failed_block_feed
                        .send(block)
                        .await
                        .map_err(|e| SoflError::Custom(format!("{:?}", e)))?;
                }
            }
        }
        Ok(())
    }

    async fn analyze_one_block(&mut self, block: u64) -> Result<(), SoflError> {
        let txs = self.provider.txs_in_block(block.cvt())?;
        let mut cfg_env = CfgEnv::default();
        self.provider.fill_cfg_env(&mut cfg_env, block.cvt())?;
        let mut block_env = BlockEnv::default();
        self.provider.fill_block_env(&mut block_env, block.cvt())?;
        let mut state = self.provider.bc_state_at(block.cvt())?;

        let mut n_creations = 0;
        let mut n_invocations = 0;

        for tx in txs {
            let mut tx_env = TxEnv::default();
            tx.fill_tx_env(&mut tx_env)?;
            let spec = TransitionSpec {
                cfg: cfg_env.clone(),
                block: block_env.clone(),
                txs: vec![tx_env],
            };

            let mut creation_insp = ExtractCreationInspector::default();
            let mut invocation_insp = ExtractInvocationInspector::default();
            let mut insp = CombinedInspector {
                inspectors: Vec::new(),
            };
            insp.add(&mut creation_insp);
            insp.add(&mut invocation_insp);

            state.transit(spec, &mut insp)?;

            drop(insp);

            let tx_hash: String = tx.hash().cvt();
            let creations: Vec<(String, String, bool)> = creation_insp
                .created
                .iter()
                .map(|(addr, destruct)| {
                    (tx_hash.clone(), ConvertTo::<String>::cvt(addr), *destruct)
                })
                .collect();
            for c in creations {
                self.creation_result_feed
                    .send(c)
                    .await
                    .map_err(|e| SoflError::Custom(format!("{:?}", e)))?;
                n_creations += 1;
            }

            let invocations: Vec<(u64, String)> = invocation_insp
                .invocations
                .iter()
                .map(|addr| (block, ConvertTo::<String>::cvt(addr)))
                .collect();
            for i in invocations {
                self.invocation_result_feed
                    .send(i)
                    .await
                    .map_err(|e| SoflError::Custom(format!("{:?}", e)))?;
                n_invocations += 1;
            }
        }
        info!(
            block = block,
            creations = n_creations,
            invocations = n_invocations,
            "block analyzed"
        );
        Ok(())
    }
}
