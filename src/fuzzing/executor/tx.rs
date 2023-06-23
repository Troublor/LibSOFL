use std::fmt::Debug;

use libafl::{
    prelude::{
        Executor, HasObservers, ObserversTuple, UsesInput, UsesObserver,
        UsesObservers,
    },
    state::UsesState,
};
use revm_primitives::{BlockEnv, CfgEnv, ExecutionResult};

use crate::{
    engine::state::{no_inspector, BcState},
    fuzzing::corpus::tx::TxInput,
};

/// TxExecutor execute a single transaction.
/// That is to say, the input of TxExecutor during fuzzing is a single transaction.
#[derive(Default)]
pub struct TxExecutor<E, BS, OT, S>
where
    BS: BcState<E>,
    OT: ObserversTuple<S>,
    S: UsesInput<Input = TxInput>,
{
    evm_cfg: CfgEnv,
    block_env: BlockEnv,
    bc_state: BS,

    observers: OT,

    pub out: Option<ExecutionResult>,

    _phantom: std::marker::PhantomData<(E, S)>,
}
impl<E, BS, OT, S> TxExecutor<E, BS, OT, S>
where
    BS: BcState<E>,
    OT: ObserversTuple<S>,
    S: UsesInput<Input = TxInput>,
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
            out: None,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<E, BS, OT, S> UsesState for TxExecutor<E, BS, OT, S>
where
    BS: BcState<E>,
    OT: ObserversTuple<S>,
    S: UsesInput<Input = TxInput>,
{
    type State = S;
}

impl<E, BS, OT, S> Debug for TxExecutor<E, BS, OT, S>
where
    BS: BcState<E>,
    OT: ObserversTuple<S>,
    S: UsesInput<Input = TxInput>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxExecutor")
            .field("observers", &self.observers)
            .field("bc_state", &self.bc_state)
            .field("out", &self.out)
            .field("_phantom", &self._phantom)
            .finish()
    }
}

impl<EM, Z, E, BS, OT, S> Executor<EM, Z> for TxExecutor<E, BS, OT, S>
where
    EM: UsesState<State = Self::State>,
    Z: UsesState<State = Self::State>,
    E: Debug,
    BS: BcState<E>,
    OT: ObserversTuple<S>,
    S: UsesInput<Input = TxInput>,
{
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        state: &mut Self::State,
        _mgr: &mut EM,
        input: &Self::Input,
    ) -> Result<libafl::prelude::ExitKind, libafl::Error> {
        let cfg = self.evm_cfg.clone();
        let block = self.block_env.clone();
        let tx = input.clone();
        let out = self
            .bc_state
            .transact(cfg, block, tx, no_inspector())
            .map_err(|e| {
                libafl::Error::IllegalArgument(
                    format!("{}", e),
                    libafl::ErrorBacktrace::new(),
                )
            })?;
        self.out = Some(out.result);
        let exit_kind = libafl::prelude::ExitKind::Ok;
        self.observers.post_exec_all(state, input, &exit_kind)?;
        Ok(exit_kind)
    }
}

impl<E, BS, OT, S> UsesObservers for TxExecutor<E, BS, OT, S>
where
    E: Debug,
    BS: BcState<E>,
    OT: ObserversTuple<S>,
    S: UsesInput<Input = TxInput>,
{
    type Observers = OT;
}

impl<E, BS, OT, S> HasObservers for TxExecutor<E, BS, OT, S>
where
    E: Debug,
    BS: BcState<E>,
    OT: ObserversTuple<S>,
    S: UsesInput<Input = TxInput>,
{
    fn observers(&self) -> &Self::Observers {
        &self.observers
    }

    fn observers_mut(&mut self) -> &mut Self::Observers {
        &mut self.observers
    }
}
