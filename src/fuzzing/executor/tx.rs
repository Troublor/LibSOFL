use std::fmt::Debug;

use libafl::{
    prelude::{Executor, HasObservers, UsesInput, UsesObservers},
    state::UsesState,
};
use revm::{Database, DatabaseCommit};
use revm_primitives::{BlockEnv, CfgEnv};

use crate::engine::state::{env::TransitionSpecBuilder, BcState};
use crate::fuzzing::{corpus::tx::TxInput, observer::EvmObserversTuple};

/// TxExecutor execute a single transaction.
/// That is to say, the input of TxExecutor during fuzzing is a single transaction.
#[derive(Default)]
pub struct TxExecutor<S, BS, OT>
where
    BS: Database,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<S, BS>,
{
    evm_cfg: CfgEnv,
    block_env: BlockEnv,
    bc_state: BS,

    observers: OT,

    _phantom: std::marker::PhantomData<S>,
}
impl<S, BS, OT> TxExecutor<S, BS, OT>
where
    BS: Database,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<S, BS>,
{
    pub fn new(
        evm_cfg: CfgEnv,
        block_env: BlockEnv,
        bc_state: BS,
        observers: OT,
        _state: &S,
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

impl<S, BS, OT> UsesState for TxExecutor<S, BS, OT>
where
    BS: Database,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<S, BS>,
{
    type State = S;
}

impl<S, BS, OT> Debug for TxExecutor<S, BS, OT>
where
    BS: Database,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<S, BS>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxExecutor")
            .field("observers", &self.observers)
            .field("_phantom", &self._phantom)
            .finish()
    }
}

impl<EM, Z, S, BS, OT> Executor<EM, Z> for TxExecutor<S, BS, OT>
where
    EM: UsesState<State = Self::State>,
    Z: UsesState<State = Self::State>,
    BS: Database + Clone + DatabaseCommit,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<S, BS>,
{
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        state: &mut Self::State,
        _mgr: &mut EM,
        input: &Self::Input,
    ) -> Result<libafl::prelude::ExitKind, libafl::Error> {
        let spec = TransitionSpecBuilder::new()
            .set_cfg(self.evm_cfg.clone())
            .set_block(self.block_env.clone())
            .bypass_check()
            .append_tx(input.from(), input)
            .build();
        let bc_state = self.bc_state.clone();
        let mut inspector: <OT as EvmObserversTuple<S, BS>>::Inspector =
            self.observers.get_inspector(&bc_state, input)?;
        let (post_state, result) =
            BcState::transit(bc_state, spec, &mut inspector).map_err(|_| {
                libafl::Error::IllegalArgument(
                    "failed to execute transaction".to_string(),
                    libafl::ErrorBacktrace::new(),
                )
            })?;
        let exit_kind = libafl::prelude::ExitKind::Ok;
        self.observers
            .on_executed(&post_state, inspector, result, input)?;
        self.observers.post_exec_all(state, input, &exit_kind)?;
        Ok(exit_kind)
    }
}

impl<S, BS, OT> UsesObservers for TxExecutor<S, BS, OT>
where
    BS: Database,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<S, BS>,
{
    type Observers = OT;
}

impl<S, BS, OT> HasObservers for TxExecutor<S, BS, OT>
where
    BS: Database,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<S, BS>,
{
    fn observers_mut(&mut self) -> &mut Self::Observers {
        &mut self.observers
    }

    fn observers(&self) -> &Self::Observers {
        &self.observers
    }
}
