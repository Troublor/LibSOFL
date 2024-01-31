// This module perform differential testing between the InterruptableEvm and the original Evm.

use std::{collections::HashSet, sync::Arc};

use revm::Inspector;

use crate::{
    blockchain::{
        provider::{BcProvider, BcStateProvider},
        transaction::Tx,
    },
    conversion::ConvertTo,
    engine::{
        inspector::EvmInspector,
        interruptable::{breakpoint::RunResult, InterruptableEvm},
        state::BcState,
        transition::{TransitionSpec, TransitionSpecBuilder},
        types::{Address, BcStateRef, ExecutionResult, StateChange, TxHash},
    },
    error::SoflError,
};

use super::breakpoint::Breakpoint;

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
    let mut breakpoint_collector = AllBreakpointCollector::default();
    let (mut state_change, mut execution_result) =
        state.simulate(spec.clone(), &mut breakpoint_collector)?;
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
        return Ok(Some(BehaviorDeivation {
            tx: tx_hash,
            oracle: (oracle_state_change, oracle_execution_result),
            output: (state_change, execution_result),
        }));
    }

    let output_with_breakpoints = run_interrutable_evm_with_breakpoints(
        &mut state,
        spec.clone(),
        breakpoint_collector.breakpoints(),
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

fn run_interruptable_evm_no_breakpoints<S: BcState>(
    state: S,
    spec: TransitionSpec,
) -> Result<RunResult, SoflError> {
    let evm = InterruptableEvm::new(spec.get_evm_version());
    let mut run_ctx = evm.build_resumable_run_context(state, spec);
    let output = evm.run(&mut run_ctx, vec![])?;
    Ok(output)
}

fn run_interrutable_evm_with_breakpoints<S: BcState>(
    state: S,
    spec: TransitionSpec,
    breakpoints: Vec<Breakpoint>,
) -> Result<RunResult, SoflError> {
    let evm = InterruptableEvm::new(spec.get_evm_version());
    let mut run_ctx = evm.build_resumable_run_context(state, spec);
    let mut output = evm.run(&mut run_ctx, breakpoints.clone())?;
    while matches!(output, RunResult::Breakpoint(_)) {
        output = evm.run(&mut run_ctx, breakpoints.clone())?;
    }
    Ok(output)
}

#[derive(Default)]
struct AllBreakpointCollector {
    contracts: HashSet<Address>,
}

impl AllBreakpointCollector {
    pub fn breakpoints(&self) -> Vec<Breakpoint> {
        self.contracts
            .iter()
            .flat_map(|addr| {
                vec![
                    Breakpoint::MsgCallBefore(*addr),
                    Breakpoint::MsgCallBegin(*addr),
                    Breakpoint::MsgCallEnd(*addr),
                    Breakpoint::MsgCallAfter(*addr),
                ]
            })
            .collect()
    }
}

impl<S: BcState> Inspector<S> for AllBreakpointCollector {
    fn call(
        &mut self,
        _context: &mut revm::EvmContext<S>,
        inputs: &mut revm::interpreter::CallInputs,
        _return_memory_offset: std::ops::Range<usize>,
    ) -> Option<revm::interpreter::CallOutcome> {
        self.contracts.insert(inputs.contract);
        None
    }
}

impl<S: BcState> EvmInspector<S> for AllBreakpointCollector {}
