use libafl::{
    prelude::{Named, Observer, UsesInput},
    state::UsesState,
};
use revm_primitives::{ExecutionResult, Halt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionResultObserver {
    result: ExecutionResult,
}

impl ExecutionResultObserver {
    pub fn new() -> Self {
        ExecutionResultObserver {
            result: ExecutionResult::Halt {
                reason: Halt::NotActivated,
                gas_used: 0,
            },
        }
    }
}

impl ExecutionResultObserver {
    pub fn set_result(&mut self, result: ExecutionResult) {
        self.result = result;
    }

    pub fn get_result(&self) -> &ExecutionResult {
        &self.result
    }
}

impl<S: UsesInput> Observer<S> for ExecutionResultObserver {}

impl Named for ExecutionResultObserver {
    fn name(&self) -> &str {
        "ExecutionResultObserver"
    }
}
