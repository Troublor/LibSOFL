use libafl::{
    events::ProgressReporter, stages::StagesTuple, state::UsesState, Fuzzer,
};

use crate::{
    blockchain::state_ref::FuzzBcStateRef, executor::MsgCallSeqExecutor,
    state::FuzzState,
};

pub struct SoflFuzzer<SR, EM, ST> {
    _phantom: std::marker::PhantomData<(SR, EM, ST)>,
}

impl<SR: FuzzBcStateRef, EM, ST> UsesState for SoflFuzzer<SR, EM, ST> {
    type State = FuzzState<SR>;
}

impl<SR, EM, ST> Fuzzer<MsgCallSeqExecutor<SR>, EM, ST>
    for SoflFuzzer<SR, EM, ST>
where
    SR: FuzzBcStateRef,
    EM: ProgressReporter<State = Self::State>,
    ST: StagesTuple<MsgCallSeqExecutor<SR>, EM, Self::State, Self>,
{
    fn fuzz_one(
        &mut self,
        _stages: &mut ST,
        _executor: &mut MsgCallSeqExecutor<SR>,
        _state: &mut EM::State,
        _manager: &mut EM,
    ) -> Result<libafl::prelude::CorpusId, libafl::prelude::Error> {
        todo!()
    }
}
