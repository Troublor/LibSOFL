use libafl::prelude::{Observer, UsesInput};
use libafl_bolts::Named;
use reth_revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};
use revm::Database;
use revm_primitives::ExecutionResult;

use super::EvmObserver;

#[derive(Debug)]
pub struct TraceObserver {
    pub inspector: Option<TracingInspector>,
}

impl Named for TraceObserver {
    fn name(&self) -> &str {
        "TraceObserver"
    }
}

impl<S: UsesInput> Observer<S> for TraceObserver {
    fn pre_exec(
        &mut self,
        _state: &mut S,
        _input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        // reset inspector
        self.inspector =
            Some(TracingInspector::new(TracingInspectorConfig::all()));
        Ok(())
    }
    fn post_exec(
        &mut self,
        _state: &mut S,
        _input: &<S as UsesInput>::Input,
        _exit_kind: &libafl::prelude::ExitKind,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}

impl<S: UsesInput, BS: Database> EvmObserver<S, BS> for TraceObserver {
    type Inspector = TracingInspector;

    fn on_executed(
        &mut self,
        _post_state: &BS,
        _inspector: Self::Inspector,
        _results: Vec<ExecutionResult>,
        _input: &S::Input,
    ) -> Result<(), libafl::Error> {
        self.inspector = Some(_inspector);
        Ok(())
    }

    fn get_inspector(
        &mut self,
        _pre_state: &BS,
        _input: &S::Input,
    ) -> Result<TracingInspector, libafl::Error> {
        let insp = self.inspector.take().unwrap();
        Ok(insp)
    }
}
