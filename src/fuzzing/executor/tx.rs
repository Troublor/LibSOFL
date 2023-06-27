use std::fmt::Debug;

use libafl::{
    prelude::{
        Executor, HasObservers, ObserversTuple, UsesInput, UsesObservers,
    },
    state::UsesState,
};
use revm_primitives::{BlockEnv, CfgEnv, ExecutionResult};

use crate::{
    engine::{config::EngineConfig, inspectors::no_inspector, state::BcState},
    fuzzing::{corpus::tx::TxInput, observer::EvmObserversTuple},
};

/// TxExecutor execute a single transaction.
/// That is to say, the input of TxExecutor during fuzzing is a single transaction.
#[derive(Default)]
pub struct TxExecutor<'a, S, BS, OT>
where
    BS: BcState,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<'a, S, BS>,
{
    evm_cfg: CfgEnv,
    block_env: BlockEnv,
    bc_state: BS,

    observers: OT,

    pub out: Option<ExecutionResult>,

    _phantom: std::marker::PhantomData<&'a S>,
}
impl<'a, S, BS, OT> TxExecutor<'a, S, BS, OT>
where
    BS: BcState,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<'a, S, BS>,
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

impl<'a, S, BS, OT> UsesState for TxExecutor<'a, S, BS, OT>
where
    BS: BcState,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<'a, S, BS>,
{
    type State = S;
}

impl<'a, S, BS, OT> Debug for TxExecutor<'a, S, BS, OT>
where
    BS: BcState,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<'a, S, BS>,
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

impl<'a, EM, Z, S, BS, OT> Executor<EM, Z> for TxExecutor<'a, S, BS, OT>
where
    EM: UsesState<State = Self::State>,
    Z: UsesState<State = Self::State>,
    BS: BcState,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<'a, S, BS>,
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
        let out = self
            .bc_state
            .transact(cfg, block, tx, no_inspector())
            .map_err(|e| {
                libafl::Error::IllegalArgument(
                    "failed to execute transaction".to_string(),
                    libafl::ErrorBacktrace::new(),
                )
            })?;
        self.out = Some(out.result);
        let exit_kind = libafl::prelude::ExitKind::Ok;
        self.observers.post_exec_all(state, input, &exit_kind)?;
        Ok(exit_kind)
    }
}

impl<'a, S, BS, OT> UsesObservers for TxExecutor<'a, S, BS, OT>
where
    BS: BcState,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<'a, S, BS>,
{
    type Observers = OT;
}

impl<'a, S, BS, OT> HasObservers for TxExecutor<'a, S, BS, OT>
where
    BS: BcState,
    S: UsesInput<Input = TxInput>,
    OT: EvmObserversTuple<'a, S, BS>,
{
    fn observers_mut(&mut self) -> &mut Self::Observers {
        todo!()
    }

    fn observers(&self) -> &Self::Observers {
        todo!()
    }
}
