use std::fmt::Debug;

use revm::EVM;
use revm_primitives::ExecutionResult;
use revm_primitives::Halt;
use revm_primitives::TxEnv;
use revm_primitives::{BlockEnv, CfgEnv};

use crate::engine::inspectors::TxHook;
use crate::fuzzing::interfaces::BcState;
use crate::fuzzing::observer::EvmObserversTuple;
use crate::fuzzing::state::FuzzState;

/// Corpus of transaction inputs
/// Type parameter T should satisfy AsRef<TxEnv>
pub type TxCorpus<T> = libafl::corpus::InMemoryCorpus<T>;
pub type TxFuzzState<T> =
    FuzzState<T, TxCorpus<T>, libafl_bolts::rands::StdRand, TxCorpus<T>>;

/// TxExecutor execute a single transaction.
/// That is to say, the input of TxExecutor during fuzzing is a single transaction.
#[derive(Default, Debug)]
pub struct TxExecutor<BS, OT, T> {
    evm_cfg: CfgEnv,
    block_env: BlockEnv,
    bc_state: BS,

    observers: OT,

    _phantom: std::marker::PhantomData<T>,
}

impl<BS, OT, T: AsRef<TxEnv> + libafl::inputs::Input> libafl::state::UsesState
    for TxExecutor<BS, OT, T>
{
    type State = TxFuzzState<T>;
}

impl<
        BS: BcState,
        OT: EvmObserversTuple<<Self as libafl::state::UsesState>::State, BS>,
        T: AsRef<TxEnv> + libafl::inputs::Input,
    > TxExecutor<BS, OT, T>
{
    pub fn new(
        evm_cfg: CfgEnv,
        block_env: BlockEnv,
        bc_state: BS,
        observers: OT,
        _state: &<Self as libafl::state::UsesState>::State,
    ) -> Self {
        Self {
            evm_cfg,
            block_env,
            bc_state,
            observers,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<EM, Z, BS, OT, T> libafl::executors::Executor<EM, Z>
    for TxExecutor<BS, OT, T>
where
    EM: libafl::state::UsesState<State = Self::State>,
    Z: libafl::state::UsesState<State = Self::State>,
    BS: BcState,
    OT: EvmObserversTuple<Self::State, BS>,
    T: AsRef<TxEnv> + libafl::inputs::Input,
{
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        state: &mut Self::State,
        _mgr: &mut EM,
        input: &Self::Input,
    ) -> Result<libafl::prelude::ExitKind, libafl::Error> {
        let mut evm: EVM<BS> = EVM::new();
        evm.env.cfg = self.evm_cfg.clone();
        evm.env.block = self.block_env.clone();
        evm.database(self.bc_state.clone());

        // get inspector from observers
        let mut insp = self.observers.get_inspector(&self.bc_state, input)?;

        let tx: TxEnv = input.as_ref().clone();
        evm.env.tx = tx;

        // inspector pre-transaction hook
        let result: ExecutionResult = if !insp.transaction(&evm.env.tx, evm.db.as_ref().expect("impossible: db does not exist while database has been set in evm")) {
            // return false to skip transaction
            ExecutionResult::Halt { reason: Halt::NotActivated, gas_used: 0 }
        } else {
            // execute transaction with inspector
            evm.inspect_commit(&mut insp).map_err(|_e| {
                libafl::Error::Unknown(
                    String::from("transaction execution error"),
                    libafl_bolts::ErrorBacktrace::new(),
                )
            })?
        };

        let exit_kind = libafl::prelude::ExitKind::Ok;
        self.observers
            .on_executed(evm.db.as_ref().expect("impossible: db does not exist while database has been set in evm"), insp, vec![result], input)?;
        self.observers.post_exec_all(state, input, &exit_kind)?;
        Ok(exit_kind)
    }
}

impl<
        BS: BcState,
        OT: EvmObserversTuple<Self::State, BS>,
        T: AsRef<TxEnv> + libafl::inputs::Input,
    > libafl::observers::UsesObservers for TxExecutor<BS, OT, T>
{
    type Observers = OT;
}

impl<
        BS: BcState,
        OT: EvmObserversTuple<Self::State, BS>,
        T: AsRef<TxEnv> + libafl::inputs::Input,
    > libafl::prelude::HasObservers for TxExecutor<BS, OT, T>
{
    fn observers_mut(&mut self) -> &mut Self::Observers {
        &mut self.observers
    }

    fn observers(&self) -> &Self::Observers {
        &self.observers
    }
}
