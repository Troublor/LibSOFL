use std::fmt::Debug;

use libafl::prelude::ObserversTuple;
use libafl::{
    prelude::{Executor, HasObservers, UsesInput, UsesObservers},
    state::UsesState,
};
use revm_primitives::{BlockEnv, CfgEnv};

use crate::{
    engine::{config::EngineConfig, state::BcState},
    fuzzing::{corpus::tx::TxInput, observer::EvmObserversTuple},
};

/// TxExecutor execute a single transaction.
/// That is to say, the input of TxExecutor during fuzzing is a single transaction.
#[derive(Default)]
pub struct TxExecutor<S, BS, OT>
where
    BS: BcState,
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
    BS: BcState,
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
    BS: BcState,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<S, BS>,
{
    type State = S;
}

impl<S, BS, OT> Debug for TxExecutor<S, BS, OT>
where
    BS: BcState,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<S, BS>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxExecutor")
            .field("observers", &self.observers)
            .field("bc_state", &self.bc_state)
            .field("_phantom", &self._phantom)
            .finish()
    }
}

impl<EM, Z, S, BS, OT> Executor<EM, Z> for TxExecutor<S, BS, OT>
where
    EM: UsesState<State = Self::State>,
    Z: UsesState<State = Self::State>,
    BS: BcState + Clone,
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
        let cfg = self.evm_cfg.clone();
        let cfg = EngineConfig::from(cfg)
            .toggle_nonce_check(false)
            .toggle_balance_check(false);
        let block = self.block_env.clone();
        let tx = input.clone();
        let bc_state = self.bc_state.clone();
        let mut inspector: <OT as EvmObserversTuple<S, BS>>::Inspector =
            self.observers.get_inspector(&bc_state, input)?;
        let (post_state, result) = bc_state
            .transit_one(cfg, block, tx, &mut inspector)
            .map_err(|_| {
                libafl::Error::IllegalArgument(
                    "failed to execute transaction".to_string(),
                    libafl::ErrorBacktrace::new(),
                )
            })?;
        let exit_kind = libafl::prelude::ExitKind::Ok;
        self.observers.on_executed(
            &post_state,
            inspector,
            vec![result],
            input,
        )?;
        self.observers.post_exec_all(state, input, &exit_kind)?;
        Ok(exit_kind)
    }
}

impl<S, BS, OT> UsesObservers for TxExecutor<S, BS, OT>
where
    BS: BcState,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<S, BS>,
{
    type Observers = OT;
}

impl<S, BS, OT> HasObservers for TxExecutor<S, BS, OT>
where
    BS: BcState,
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
