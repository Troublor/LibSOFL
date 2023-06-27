use libafl::prelude::{Named, Observer, UsesInput};
use reth_revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};
use revm_primitives::ExecutionResult;

use crate::engine::state::BcState;

use super::EvmObserver;

#[derive(Debug)]
pub struct TraceObserver {
    pub inspector: TracingInspector,
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
        self.inspector = TracingInspector::new(TracingInspectorConfig::all());
        Ok(())
    }
    fn post_exec(
        &mut self,
        _state: &mut S,
        _input: &<S as UsesInput>::Input,
        _exit_kind: &libafl::prelude::ExitKind,
    ) -> Result<(), libafl::Error> {
        let builder = self.inspector.clone().into_geth_builder();
        Ok(())
    }
}

impl<S: UsesInput, BS: BcState> EvmObserver<S, BS> for TraceObserver {
    type Inspector = TracingInspector;

    fn on_execution_result(
        &mut self,
        _result: ExecutionResult,
        _input: &S::Input,
        _index: u32,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }

    fn get_inspector(
        &mut self,
        _input: &S::Input,
        _index: u32,
    ) -> Result<&mut TracingInspector, libafl::Error> {
        Ok(&mut self.inspector)
    }
}
