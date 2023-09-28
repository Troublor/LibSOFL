use libafl::{
    prelude::{Feedback, UsesInput},
    state::HasClientPerfMonitor,
};
use libafl_bolts::Named;

#[derive(Debug, Default)]
pub struct AlwaysFeedback {}

impl Named for AlwaysFeedback {
    fn name(&self) -> &str {
        "AlwaysFeedback"
    }
}

impl<S: UsesInput + HasClientPerfMonitor> Feedback<S> for AlwaysFeedback {
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &S::Input,
        _observers: &OT,
        _exit_kind: &libafl::prelude::ExitKind,
    ) -> Result<bool, libafl::Error>
    where
        EM: libafl::prelude::EventFirer<State = S>,
        OT: libafl::prelude::ObserversTuple<S>,
    {
        Ok(true)
    }
}
