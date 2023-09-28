use libafl::prelude::{Observer, UsesInput};
use libafl_bolts::Named;
use revm::Database;
use revm_primitives::ExecutionResult;
use serde::{Deserialize, Serialize};

use crate::engine::inspectors::NoInspector;

use super::EvmObserver;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ExecutionResultObserver {
    pub results: Vec<ExecutionResult>,
}

impl<S: UsesInput> Observer<S> for ExecutionResultObserver {
    fn pre_exec(
        &mut self,
        _state: &mut S,
        _input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        self.results.clear();
        Ok(())
    }
}

impl<S: UsesInput, BS: Database> EvmObserver<S, BS>
    for ExecutionResultObserver
{
    type Inspector = NoInspector;

    fn on_executed(
        &mut self,
        _post_state: &BS,
        _inspector: Self::Inspector,
        mut _result: Vec<ExecutionResult>,
        _input: &S::Input,
    ) -> Result<(), libafl::Error> {
        self.results.append(&mut _result);
        Ok(())
    }

    fn get_inspector(
        &mut self,
        _pre_state: &BS,
        _input: &S::Input,
    ) -> Result<NoInspector, libafl::Error> {
        Ok(())
    }
}

impl Named for ExecutionResultObserver {
    fn name(&self) -> &str {
        "ExecutionResultObserver"
    }
}
