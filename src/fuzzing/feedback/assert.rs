use libafl::{
    prelude::{Feedback, UsesInput},
    state::HasClientPerfMonitor,
};
use libafl_bolts::Named;
use revm_primitives::ExecutionResult;

use crate::fuzzing::observer::result::ExecutionResultObserver;

#[derive(Debug)]
pub struct AssertionFeedback {}

impl AssertionFeedback {
    pub fn new() -> Self {
        AssertionFeedback {}
    }
}

impl Default for AssertionFeedback {
    fn default() -> Self {
        Self::new()
    }
}

impl Named for AssertionFeedback {
    fn name(&self) -> &str {
        "AssertionFeedback"
    }
}

impl<S: UsesInput + HasClientPerfMonitor> Feedback<S> for AssertionFeedback {
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &S::Input,
        observers: &OT,
        _exit_kind: &libafl::prelude::ExitKind,
    ) -> Result<bool, libafl::Error>
    where
        EM: libafl::prelude::EventFirer<State = S>,
        OT: libafl::prelude::ObserversTuple<S>,
    {
        let obs: &ExecutionResultObserver =
            observers.match_name("ExecutionResultObserver").unwrap();
        let r = obs
            .results
            .iter()
            .any(|r| matches!(r, ExecutionResult::Halt { .. }));
        Ok(r)
    }
}
