use libafl::prelude::{Named, Observer, UsesInput};
use revm_primitives::ExecutionResult;
use serde::{Deserialize, Serialize};

use crate::engine::{
    inspectors::{no_inspector, NoInspector},
    state::BcState,
};

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

impl<S: UsesInput, BS: BcState> EvmObserver<S, BS, NoInspector>
    for ExecutionResultObserver
{
    fn on_execution_result(
        &mut self,
        _result: ExecutionResult,
        _input: &S::Input,
        _index: u32,
    ) -> Result<(), libafl::Error> {
        self.results.push(_result);
        Ok(())
    }

    fn get_inspector(
        &mut self,
        _input: &S::Input,
        _index: u32,
    ) -> Result<&mut NoInspector, libafl::Error> {
        Ok(no_inspector())
    }
}

impl Named for ExecutionResultObserver {
    fn name(&self) -> &str {
        "ExecutionResultObserver"
    }
}
