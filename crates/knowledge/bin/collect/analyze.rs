use std::{collections::HashSet, sync::Arc};

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
use libsofl_utils::log::info;

pub struct Analyzer<
    T: Tx,
    S: DatabaseRef,
    P: BcProvider<T> + BcStateProvider<S>,
> where
    S::Error: std::fmt::Debug,
{
    provider: Arc<P>,

    _phantom: std::marker::PhantomData<(T, S)>,
}

impl<T: Tx, S: DatabaseRef, P: BcProvider<T> + BcStateProvider<S>> Clone
    for Analyzer<T, S, P>
where
    S::Error: std::fmt::Debug,
{
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: Tx, S: DatabaseRef, P: BcProvider<T> + BcStateProvider<S>>
    Analyzer<T, S, P>
where
    S::Error: std::fmt::Debug,
{
    pub fn new(provider: Arc<P>) -> Self {
        Self {
            provider,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: Tx, S: DatabaseRef, P: BcProvider<T> + BcStateProvider<S>>
    Analyzer<T, S, P>
where
    S::Error: std::fmt::Debug,
{
    pub async fn analyze_one_block(
        &mut self,
        block: u64,
    ) -> Result<(Vec<(String, String, bool)>, HashSet<String>), SoflError> {
        let txs = self.provider.txs_in_block(block.cvt())?;
        let mut cfg_env = CfgEnv::default();
        self.provider.fill_cfg_env(&mut cfg_env, block.cvt())?;
        let mut block_env = BlockEnv::default();
        self.provider.fill_block_env(&mut block_env, block.cvt())?;
        let mut state = self.provider.bc_state_at(block.cvt())?;

        let mut total_creations = Vec::new();
        let mut total_invocations = HashSet::new();

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
            total_creations.extend(creations);

            let invocations: Vec<String> = invocation_insp
                .invocations
                .iter()
                .map(|addr| ConvertTo::<String>::cvt(addr))
                .collect();
            total_invocations.extend(invocations);
        }
        info!(
            block = block,
            creations = total_creations.len(),
            invocations = total_invocations.len(),
            "block analyzed"
        );
        Ok((total_creations, total_invocations))
    }
}

#[cfg(test)]
mod tests_with_dep {
    use std::sync::Arc;

    use libsofl_knowledge::testing::get_bc_provider;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_analyze_block() {
        let bp = get_bc_provider();

        let mut analyzer = super::Analyzer::new(Arc::new(bp));
        let (creations, invocations) =
            analyzer.analyze_one_block(1000000).await.unwrap();

        assert_eq!(creations.len(), 0);
        assert_eq!(invocations.len(), 2);
    }
}
