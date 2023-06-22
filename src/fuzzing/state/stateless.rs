use libafl::{
    prelude::{HasTestcase, UsesInput},
    state::State,
};
use serde::{Deserialize, Serialize};

use crate::fuzzing::corpus::tx::TxInput;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PomFuzzState {}

impl State for PomFuzzState {}

impl PomFuzzState {
    pub fn new() -> Self {
        PomFuzzState {}
    }
}

impl UsesInput for PomFuzzState {
    type Input = TxInput;
}

impl HasTestcase for PomFuzzState {
    fn testcase(
        &self,
        id: libafl::prelude::CorpusId,
    ) -> Result<
        std::cell::Ref<libafl::prelude::Testcase<<Self as UsesInput>::Input>>,
        libafl::Error,
    > {
        todo!()
    }

    fn testcase_mut(
        &self,
        id: libafl::prelude::CorpusId,
    ) -> Result<
        std::cell::RefMut<
            libafl::prelude::Testcase<<Self as UsesInput>::Input>,
        >,
        libafl::Error,
    > {
        todo!()
    }
}
