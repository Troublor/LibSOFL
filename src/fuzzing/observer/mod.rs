use std::{
    fmt::Debug,
    ptr::{addr_of, addr_of_mut},
};

use derive_more::{AsMut, AsRef, Deref, DerefMut};
use libafl::prelude::{Input, MatchName, Observer, ObserversTuple, UsesInput};
use revm::Inspector;
use revm_primitives::ExecutionResult;
use serde::{Deserialize, Serialize};

use crate::engine::{inspectors::combined::CombinedInspector, state::BcState};

pub mod result;
pub mod trace;

pub trait EvmObserver<S, BS>: Observer<S>
where
    S: UsesInput,
    BS: BcState,
{
    type Inspector: Inspector<BS>;

    /// Get the EVM inspector of associated with observer.
    /// The inspector is used to inspect the EVM state during transaction execution.
    /// EVMObserver implementations may need to reset the inspector in [Observer::pre_exec].
    /// * `input` - The input of the current fuzzer exection.
    /// * `index` - The index of the tranasction to be executed in the input, incase the input contains a sequence of transactions.
    fn get_inspector(
        &mut self,
        _input: &S::Input,
        _index: u32,
    ) -> Result<&mut Self::Inspector, libafl::Error>;

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

// type GenericEvmObservers<'a, S, BS> =
//     Vec<Box<dyn EvmObserver<S, BS, Box<dyn Inspector<BS> + 'a>> + 'a>>;

// #[derive(Default, Deref, DerefMut, AsRef, AsMut)]
// pub struct EvmObservers<'a, S, BS>
// where
//     S: UsesInput,
//     BS: BcState,
// {
//     #[deref]
//     #[as_ref]
//     #[as_mut]
//     #[deref_mut]
//     observers: GenericEvmObservers<'a, S, BS>,
// }

// impl<S, BS> EvmObservers<'_, S, BS>
// where
//     S: UsesInput,
//     BS: BcState,
// {
//     pub fn new() -> Self {
//         Self {
//             observers: Vec::new(),
//         }
//     }
// }

// impl<S, BS> Debug for EvmObservers<'_, S, BS>
// where
//     S: UsesInput,
//     BS: BcState,
// {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let mut debug_struct = f.debug_tuple("EvmObserver");
//         for obs in self.iter() {
//             debug_struct.field(obs);
//         }
//         debug_struct.finish()
//     }
// }
// impl<'a, S, BS> EvmObservers<'a, S, BS>
// where
//     S: UsesInput,
//     BS: BcState,
// {
//     pub fn get_inspector(
//         &mut self,
//         _input: &S::Input,
//         _index: u32,
//     ) -> Result<CombinedInspector<BS>, libafl::Error> {
//         // create a combined inspector
//         let mut insp = CombinedInspector::new();
//         for obs in self.iter_mut() {
//             let inner_insp = obs.get_inspector(_input, _index)?;
//             insp.append(inner_insp);
//         }
//         Ok(insp)
//     }

//     pub fn on_execution_result(
//         &mut self,
//         _result: ExecutionResult,
//         _input: &S::Input,
//         _index: u32,
//     ) -> Result<(), libafl::Error> {
//         for obs in self.iter_mut() {
//             obs.on_execution_result(_result.clone(), _input, _index)?;
//         }
//         Ok(())
//     }
// }
// impl<S, BS> MatchName for EvmObservers<'_, S, BS>
// where
//     S: UsesInput,
//     BS: BcState,
// {
//     fn match_name<T>(&self, name: &str) -> Option<&T> {
//         self.iter()
//             .find(|obs| obs.name() == name)
//             .and_then(|obs| unsafe { (addr_of!(obs) as *const T).as_ref() })
//     }

//     fn match_name_mut<T>(&mut self, name: &str) -> Option<&mut T> {
//         self.iter_mut().find(|obs| obs.name() == name).and_then(
//             |mut obs| unsafe { (addr_of_mut!(obs) as *mut T).as_mut() },
//         )
//     }
// }

// impl<S, BS> ObserversTuple<S> for EvmObservers<'_, S, BS>
// where
//     S: UsesInput,
//     BS: BcState,
// {
//     fn pre_exec_all(
//         &mut self,
//         state: &mut S,
//         input: &<S as UsesInput>::Input,
//     ) -> Result<(), libafl::Error> {
//         for obs in self.iter_mut() {
//             obs.pre_exec(state, input)?;
//         }
//         Ok(())
//     }

//     fn post_exec_all(
//         &mut self,
//         state: &mut S,
//         input: &<S as UsesInput>::Input,
//         exit_kind: &libafl::prelude::ExitKind,
//     ) -> Result<(), libafl::Error> {
//         for obs in self.iter_mut() {
//             obs.post_exec(state, input, exit_kind)?;
//         }
//         Ok(())
//     }

//     fn pre_exec_child_all(
//         &mut self,
//         state: &mut S,
//         input: &<S as UsesInput>::Input,
//     ) -> Result<(), libafl::Error> {
//         for obs in self.iter_mut() {
//             obs.pre_exec_child(state, input)?;
//         }
//         Ok(())
//     }

//     fn post_exec_child_all(
//         &mut self,
//         state: &mut S,
//         input: &<S as UsesInput>::Input,
//         exit_kind: &libafl::prelude::ExitKind,
//     ) -> Result<(), libafl::Error> {
//         for obs in self.iter_mut() {
//             obs.post_exec_child(state, input, exit_kind)?;
//         }
//         Ok(())
//     }

//     fn observes_stdout(&self) -> bool {
//         self.iter().any(|obs| obs.observes_stdout())
//     }

//     fn observes_stderr(&self) -> bool {
//         self.iter().any(|obs| obs.observes_stderr())
//     }

//     fn observe_stdout(&mut self, stdout: &[u8]) {
//         for obs in self.iter_mut() {
//             obs.observe_stdout(stdout);
//         }
//     }

//     fn observe_stderr(&mut self, stderr: &[u8]) {
//         for obs in self.iter_mut() {
//             obs.observe_stderr(stderr);
//         }
//     }
// }

pub trait EvmObserversTuple<'a, S: UsesInput, BS: BcState>:
    ObserversTuple<S>
{
    fn get_inspector(
        &'a mut self,
        input: &S::Input,
        index: u32,
    ) -> Result<CombinedInspector<'a, BS>, libafl::Error>;

    fn on_execution_result(
        &'a mut self,
        result: ExecutionResult,
        input: &S::Input,
        index: u32,
    ) -> Result<(), libafl::Error>;
}

impl<'a, S: UsesInput, BS: BcState> EvmObserversTuple<'a, S, BS> for () {
    fn get_inspector(
        &'a mut self,
        _input: &S::Input,
        _index: u32,
    ) -> Result<CombinedInspector<'a, BS>, libafl::Error> {
        Ok(CombinedInspector::new())
    }

    fn on_execution_result(
        &'a mut self,
        _result: ExecutionResult,
        _input: &S::Input,
        _index: u32,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}

impl<'a, S, BS, Head, Tail> EvmObserversTuple<'a, S, BS> for (Head, Tail)
where
    S: UsesInput,
    BS: BcState + 'a,
    Head: EvmObserver<S, BS>,
    Tail: EvmObserversTuple<'a, S, BS>,
    <Head as EvmObserver<S, BS>>::Inspector: 'a,
{
    fn get_inspector(
        &'a mut self,
        input: &<S as UsesInput>::Input,
        index: u32,
    ) -> Result<CombinedInspector<'a, BS>, libafl::Error> {
        let insp = self.0.get_inspector(input, index)?;
        let mut insps = self.1.get_inspector(input, index)?;
        insps.append(insp);
        Ok(insps)
    }

    fn on_execution_result(
        &'a mut self,
        result: ExecutionResult,
        input: &<S as UsesInput>::Input,
        index: u32,
    ) -> Result<(), libafl::Error> {
        self.0.on_execution_result(result.clone(), input, index)?;
        self.1.on_execution_result(result, input, index)
    }
}
