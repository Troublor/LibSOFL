// This module perform differential testing between the InterruptableEvm and the original Evm.

use std::sync::Arc;

use crate::{
    blockchain::{
        provider::{BcProvider, BcStateProvider},
        transaction::Tx,
    },
    conversion::ConvertTo,
    engine::{
        inspector::no_inspector,
        interruptable::breakpoint::RunResult,
        state::BcState,
        transition::{TransitionSpec, TransitionSpecBuilder},
        types::{BcStateRef, ExecutionResult, StateChange, TxHash},
    },
    error::SoflError,
};

use super::{
    breakpoint::{break_everywhere, break_nowhere, Breakpoint},
    evm::InterruptableEvm,
};

#[derive(Debug)]
pub struct BehaviorDeivation {
    pub tx: TxHash,
    pub oracle: (StateChange, ExecutionResult),
    pub output: (StateChange, ExecutionResult),
}

pub type DifferentialTestingReport = Vec<BehaviorDeivation>;

pub fn differential_testing<
    T: Tx,
    D: BcStateRef,
    P: BcProvider<T> + BcStateProvider<D>,
>(
    provider: Arc<P>,
    bn: u64,
) -> Result<Option<BehaviorDeivation>, SoflError>
where
    D::Error: std::fmt::Debug,
{
    let mut state = provider.bc_state_at(bn.cvt())?;
    let txs = provider.txs_in_block(bn.cvt())?;

    for tx in txs.into_iter() {
        let tx_hash = tx.hash();
        let spec = TransitionSpecBuilder::default()
            .at_block(provider.clone(), bn.cvt())
            .append_tx(tx)
            .build();
        let report =
            differential_testing_one_tx::<T, _, P>(&mut state, tx_hash, spec)?;
        if let Some(report) = report {
            return Ok(Some(report));
        }
    }

    Ok(None)
}

pub fn differential_testing_one_tx<T: Tx, S: BcState, P: BcProvider<T>>(
    mut state: S,
    tx_hash: TxHash,
    spec: TransitionSpec,
) -> Result<Option<BehaviorDeivation>, SoflError> {
    let (mut state_change, mut execution_result) =
        state.simulate(spec.clone(), no_inspector())?;
    let oracle_state_change = state_change.remove(0);
    let oracle_execution_result = execution_result.remove(0);

    let output_no_breakpoints =
        run_interruptable_evm_no_breakpoints(&mut state, spec.clone())?;
    let RunResult::Done((state_change, execution_result)) =
        output_no_breakpoints
    else {
        unreachable!("run should be done")
    };

    if state_change != oracle_state_change
        || execution_result != oracle_execution_result
    {
        println!("{:?}", oracle_state_change);
        println!("{:?}", state_change);
        return Ok(Some(BehaviorDeivation {
            tx: tx_hash,
            oracle: (oracle_state_change, oracle_execution_result),
            output: (state_change, execution_result),
        }));
    }

    let output_with_breakpoints = run_interrutable_evm_with_breakpoints(
        &mut state,
        spec.clone(),
        break_everywhere(),
    )?;
    let RunResult::Done((state_change, execution_result)) =
        output_with_breakpoints
    else {
        unreachable!("run should be done")
    };

    if state_change != oracle_state_change
        || execution_result != oracle_execution_result
    {
        return Ok(Some(BehaviorDeivation {
            tx: tx_hash,
            oracle: (oracle_state_change.clone(), oracle_execution_result),
            output: (state_change, execution_result),
        }));
    }

    state.commit(oracle_state_change);
    Ok(None)
}

fn run_interruptable_evm_no_breakpoints<'a, S: BcState>(
    state: S,
    spec: TransitionSpec,
) -> Result<RunResult<()>, SoflError> {
    let mut evm = InterruptableEvm::new(
        spec.get_evm_version(),
        state,
        spec,
        *no_inspector(),
    );
    let output = evm.run(break_nowhere())?;
    Ok(output)
}

fn run_interrutable_evm_with_breakpoints<
    'a,
    S: BcState + 'a,
    M,
    B: Breakpoint<M>,
>(
    state: S,
    spec: TransitionSpec,
    breakpoints: Arc<B>,
) -> Result<RunResult<M>, SoflError> {
    let mut evm = InterruptableEvm::new(
        spec.get_evm_version(),
        state,
        spec,
        *no_inspector(),
    );
    let mut output = evm.run(breakpoints.clone())?;
    while matches!(output, RunResult::Breakpoint(_)) {
        output = evm.run(breakpoints.clone())?;
    }
    Ok(output)
}
