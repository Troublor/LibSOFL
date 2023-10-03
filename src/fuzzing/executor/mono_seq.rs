use libafl::prelude::HasObservers;
use libafl_bolts::ErrorBacktrace;
use revm::EVM;
use revm_primitives::{ExecutionResult, Halt};

use crate::{
    engine::inspectors::TxHook,
    fuzzing::{
        corpus::mono_seq::MonoEnvTxSeqInput, interfaces::BcState,
        observer::EvmObserversTuple, state::FuzzState,
    },
};

pub type MonoEnvTxSeqCorpus<BS> =
    libafl::corpus::InMemoryCorpus<MonoEnvTxSeqInput<BS>>;
pub type MonoEnvTxSeqFuzzState<BS> = FuzzState<
    MonoEnvTxSeqInput<BS>,
    MonoEnvTxSeqCorpus<BS>,
    libafl_bolts::rands::StdRand,
    MonoEnvTxSeqCorpus<BS>,
>;

/// The EVM-based fuzzing executor, which executes a transaction sequence in a fixed execution environment.
/// EVMExecutor implements the Executor trait of libafl.
#[derive(Default, Debug)]
pub struct MonoEnvTxSeqFuzzExecutor<BS, OT> {
    pub observers: OT,
    _phantom: std::marker::PhantomData<(BS, OT)>,
}

// Required by trait libafl::executors::Executor
impl<BS: BcState, OT> libafl::state::UsesState
    for MonoEnvTxSeqFuzzExecutor<BS, OT>
{
    type State = MonoEnvTxSeqFuzzState<BS>;
}

// Implement the Executor trait of libafl, executing a sequence of transactions in a fixed execution environment.
impl<
        BS: BcState,
        OT: EvmObserversTuple<Self::State, BS>,
        EM: libafl::state::UsesState<State = Self::State>,
        Z: libafl::state::UsesState<State = Self::State>,
    > libafl::executors::Executor<EM, Z> for MonoEnvTxSeqFuzzExecutor<BS, OT>
{
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        _state: &mut Self::State,
        _mgr: &mut EM,
        _input: &Self::Input,
    ) -> Result<libafl::prelude::ExitKind, libafl_bolts::Error> {
        let mut evm: EVM<BS> = EVM::new();
        evm.env.cfg = _input.evm.clone();
        evm.env.block = _input.block.clone();
        evm.database(_input.state.clone());

        // get inspector from observers
        let observers = self.observers_mut();
        let mut inspector = observers.get_inspector(&_input.state, _input)?;

        let mut results: Vec<ExecutionResult> = Vec::new();
        for tx in _input.txs.iter() {
            let insp = &mut inspector;
            evm.env.tx = tx.clone();

            // inspector pre-transaction hook
            if !insp.transaction(&evm.env.tx, evm.db.as_ref().expect("impossible: db does not exist while database has been set in evm")) {
                // return false to skip transaction
                results.push(ExecutionResult::Halt { reason: Halt::NotActivated, gas_used: 0 });
                continue;
            }

            // execute transaction with inspector
            let result = evm.inspect_commit(insp).map_err(|_e| {
                libafl::Error::Unknown(
                    String::from("transaction execution error"),
                    ErrorBacktrace::new(),
                )
            })?;

            // inspector post-transaction hook
            inspector.transaction_end(&evm.env.tx, evm.db.as_ref().expect("impossible: db does not exist while database has been set in evm"), &result);

            results.push(result);
        }

        let exit_kind = libafl::prelude::ExitKind::Ok;
        observers.on_executed(evm.db.as_ref().expect("impossible: db does not exist while database has been set in evm"), inspector, results, _input)?;
        observers.post_exec_all(_state, _input, &exit_kind)?;
        Ok(exit_kind)
    }
}

impl<BS: BcState, OT: EvmObserversTuple<Self::State, BS>>
    libafl::observers::UsesObservers for MonoEnvTxSeqFuzzExecutor<BS, OT>
{
    type Observers = OT;
}

impl<BS: BcState, OT: EvmObserversTuple<Self::State, BS>>
    libafl::prelude::HasObservers for MonoEnvTxSeqFuzzExecutor<BS, OT>
{
    fn observers(&self) -> &Self::Observers {
        &self.observers
    }

    fn observers_mut(&mut self) -> &mut Self::Observers {
        &mut self.observers
    }
}
