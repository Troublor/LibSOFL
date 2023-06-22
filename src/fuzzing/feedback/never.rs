use libafl::prelude::{Feedback, HasClientPerfMonitor, Named, UsesInput};

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
        state: &mut S,
        manager: &mut EM,
        input: &S::Input,
        observers: &OT,
        exit_kind: &libafl::prelude::ExitKind,
    ) -> Result<bool, libafl::Error>
    where
        EM: libafl::prelude::EventFirer<State = S>,
        OT: libafl::prelude::ObserversTuple<S>,
    {
        return Ok(false);
    }
}
