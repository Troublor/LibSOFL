use std::{
    fmt::Debug,
    ptr::{addr_of, addr_of_mut},
};

use derive_more::{AsMut, AsRef, Deref, DerefMut};
use libafl::prelude::{Input, MatchName, Observer, ObserversTuple, UsesInput};
use revm::Inspector;
use revm_primitives::ExecutionResult;

use crate::engine::state::BcState;

pub mod result;

pub trait EvmObserver<S, BS, E, I>: Observer<S>
where
    S: UsesInput,
    BS: BcState<E>,
    I: Inspector<BS>,
{
    /// Get the EVM inspector of associated with observer.
    /// The inspector is used to inspect the EVM state during transaction execution.
    /// EVMObserver implementations may need to reset the inspector in [Observer::pre_exec].
    /// * `input` - The input of the current fuzzer exection.
    /// * `index` - The index of the tranasction to be executed in the input, incase the input contains a sequence of transactions.
    fn get_inspector(
        &mut self,
        _input: &S::Input,
        _index: u32,
    ) -> Result<&mut I, libafl::Error>;

    /// A callback function fed with the EVM execution result.
    /// The callback is called after each transaction execution.
    /// If one fuzzer input contains a sequence of transactions, the callback is called after each transaction execution.
    /// * `result` - The execution result of the transaction.
    /// * `input` - The input of the current fuzzer exection.
    /// * `index` - The index of the tranasction to be executed in the input, incase the input contains a sequence of transactions.
    fn on_execution_result(
        &mut self,
        _result: ExecutionResult,
        _input: &S::Input,
        _index: u32,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}

#[derive(Default, Deref, DerefMut, AsRef, AsMut)]
pub struct EvmObservers<S, BS, E, I>(Vec<Box<dyn EvmObserver<S, BS, E, I>>>)
where
    S: UsesInput,
    BS: BcState<E>,
    I: Inspector<BS>;

impl<S, BS, E, I> Debug for EvmObservers<S, BS, E, I>
where
    S: UsesInput,
    BS: BcState<E>,
    I: Inspector<BS>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_tuple("EvmObserver");
        for obs in self.iter() {
            debug_struct.field(obs);
        }
        debug_struct.finish()
    }
}
impl<S, BS, E, I> EvmObservers<S, BS, E, I>
where
    S: UsesInput,
    BS: BcState<E>,
    I: Inspector<BS>,
{
    pub fn get_inspector(
        &mut self,
        _input: &impl Input,
        _index: u32,
    ) -> Result<&mut dyn Inspector<BS>, libafl::Error> {
        todo!()
    }

    pub fn on_execution_result(
        &mut self,
        _result: ExecutionResult,
        _input: &impl Input,
        _index: u32,
    ) -> Result<(), libafl::Error> {
        todo!()
    }
}
impl<S, BS, E, I> MatchName for EvmObservers<S, BS, E, I>
where
    S: UsesInput,
    BS: BcState<E>,
    I: Inspector<BS>,
{
    fn match_name<T>(&self, name: &str) -> Option<&T> {
        self.iter()
            .find(|obs| obs.name() == name)
            .and_then(|obs| unsafe { (addr_of!(obs) as *const T).as_ref() })
    }

    fn match_name_mut<T>(&mut self, name: &str) -> Option<&mut T> {
        self.iter_mut().find(|obs| obs.name() == name).and_then(
            |mut obs| unsafe { (addr_of_mut!(obs) as *mut T).as_mut() },
        )
    }
}

impl<S, BS, E, I> ObserversTuple<S> for EvmObservers<S, BS, E, I>
where
    S: UsesInput,
    BS: BcState<E>,
    I: Inspector<BS>,
{
    fn pre_exec_all(
        &mut self,
        state: &mut S,
        input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        for obs in self.iter_mut() {
            obs.pre_exec(state, input)?;
        }
        Ok(())
    }

    fn post_exec_all(
        &mut self,
        state: &mut S,
        input: &<S as UsesInput>::Input,
        exit_kind: &libafl::prelude::ExitKind,
    ) -> Result<(), libafl::Error> {
        for obs in self.iter_mut() {
            obs.post_exec(state, input, exit_kind)?;
        }
        Ok(())
    }

    fn pre_exec_child_all(
        &mut self,
        state: &mut S,
        input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        for obs in self.iter_mut() {
            obs.pre_exec_child(state, input)?;
        }
        Ok(())
    }

    fn post_exec_child_all(
        &mut self,
        state: &mut S,
        input: &<S as UsesInput>::Input,
        exit_kind: &libafl::prelude::ExitKind,
    ) -> Result<(), libafl::Error> {
        for obs in self.iter_mut() {
            obs.post_exec_child(state, input, exit_kind)?;
        }
        Ok(())
    }

    fn observes_stdout(&self) -> bool {
        self.iter().any(|obs| obs.observes_stdout())
    }

    fn observes_stderr(&self) -> bool {
        self.iter().any(|obs| obs.observes_stderr())
    }

    fn observe_stdout(&mut self, stdout: &[u8]) {
        for obs in self.iter_mut() {
            obs.observe_stdout(stdout);
        }
    }

    fn observe_stderr(&mut self, stderr: &[u8]) {
        for obs in self.iter_mut() {
            obs.observe_stderr(stderr);
        }
    }
}
