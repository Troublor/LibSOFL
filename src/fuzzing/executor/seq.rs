use std::fmt::Debug;

use libafl::{
    prelude::{Executor, HasObservers, UsesInput, UsesObservers},
    state::UsesState,
};
use revm::{Database, DatabaseCommit};
use revm_primitives::{BlockEnv, CfgEnv};

use crate::{
    engine::state::{env::TransitionSpecBuilder, BcState},
    fuzzing::{corpus::seq::TxSequenceInput, observer::EvmObserversTuple},
};

#[derive(Default, Debug)]
pub struct TxSequenceExecutor<S, BS, OT>
where
    BS: Database + Clone,
    S: UsesInput<Input = TxSequenceInput>,
    OT: EvmObserversTuple<S, BS>,
{
    evm_cfg: CfgEnv,
    block_env: BlockEnv,
    bc_state: BS,

    observers: OT,

    _phantom: std::marker::PhantomData<S>,
}

impl<S, BS, OT> TxSequenceExecutor<S, BS, OT>
where
    BS: Database + Clone,
    S: UsesInput<Input = TxSequenceInput>,
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

impl<S, BS, OT> UsesState for TxSequenceExecutor<S, BS, OT>
where
    BS: Database + Clone,
    S: UsesInput<Input = TxSequenceInput>,
    OT: EvmObserversTuple<S, BS>,
{
    type State = S;
}

impl<S, BS, OT> UsesObservers for TxSequenceExecutor<S, BS, OT>
where
    BS: Database + Clone,
    S: UsesInput<Input = TxSequenceInput>,
    OT: EvmObserversTuple<S, BS>,
{
    type Observers = OT;
}

impl<S, BS, OT> HasObservers for TxSequenceExecutor<S, BS, OT>
where
    BS: Database + Clone,
    S: UsesInput<Input = TxSequenceInput>,
    OT: EvmObserversTuple<S, BS>,
{
    fn observers(&self) -> &Self::Observers {
        &self.observers
    }

    fn observers_mut(&mut self) -> &mut Self::Observers {
        &mut self.observers
    }
}

impl<EM, Z, S, BS, OT> Executor<EM, Z> for TxSequenceExecutor<S, BS, OT>
where
    EM: UsesState<State = Self::State>,
    Z: UsesState<State = Self::State>,
    BS: Database + Clone + Debug + DatabaseCommit,
    <BS as Database>::Error: std::fmt::Debug,
    S: UsesInput<Input = TxSequenceInput> + Debug,
    OT: EvmObserversTuple<S, BS>,
{
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        state: &mut Self::State,
        _mgr: &mut EM,
        input: &Self::Input,
    ) -> Result<libafl::prelude::ExitKind, libafl::Error> {
        let mut spec_builder = TransitionSpecBuilder::new()
            .set_cfg(self.evm_cfg.clone())
            .set_block(self.block_env.clone())
            .bypass_check();
        for tx in input.to_txs() {
            spec_builder = spec_builder.append_tx(tx.from(), tx);
        }
        let spec = spec_builder.build();
        let bc_state = self.bc_state.clone();
        let mut inspector = self.observers.get_inspector(&bc_state, input)?;
        let (post_state, results) =
            BcState::transit(bc_state, spec, &mut inspector).map_err(|e| {
                libafl::Error::IllegalArgument(
                    format!("Execution error: {:?}", e),
                    Default::default(),
                )
            })?;
        let exit_kind = libafl::prelude::ExitKind::Ok;
        self.observers
            .on_executed(&post_state, inspector, results, input)?;
        self.observers.post_exec_all(state, input, &exit_kind)?;
        Ok(exit_kind)
    }
}
