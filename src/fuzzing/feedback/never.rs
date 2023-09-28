use libafl::prelude::{Feedback, HasClientPerfMonitor, UsesInput};
use libafl_bolts::Named;

#[derive(Debug)]
pub struct NeverFeedback {}

impl Named for NeverFeedback {
    fn name(&self) -> &str {
        "NeverFeedback"
    }
}

impl<S: UsesInput + HasClientPerfMonitor> Feedback<S> for NeverFeedback {
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
        Ok(false)
    }
}
