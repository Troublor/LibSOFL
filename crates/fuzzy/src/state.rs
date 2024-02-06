use std::time::Duration;

use libafl::{
    corpus::{InMemoryCorpus, OnDiskCorpus},
    inputs::UsesInput,
    state::{
        HasExecutions, HasLastReportTime, HasMetadata, HasNamedMetadata, State,
    },
};
use libafl_bolts::rands::RomuDuoJrRand;
use libsofl_core::engine::memory::MemoryBcState;

use crate::{
    blockchain::state_ref::FuzzBcStateRef,
    input::{FuzzInput, MsgCallSeq},
};

/// Fuzzing state
/// Type parameters:
/// - `SR`: FuzzBcStateRef
/// - `RN`: Random number generator
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(bound = "SR: serde::Serialize + serde::de::DeserializeOwned")]
pub struct FuzzState<SR>
where
    SR: FuzzBcStateRef,
{
    /// random number generator
    rand: RomuDuoJrRand,

    /// corpus
    corpus: InMemoryCorpus<FuzzInput<SR>>,

    /// solutions
    solutions: OnDiskCorpus<FuzzInput<SR>>,

    /// number of executions
    executions: usize,

    /// last report time
    last_report_time: Option<Duration>,

    /// arbitrary metadata
    metadata: libafl_bolts::prelude::SerdeAnyMap,
    named_metadata: libafl_bolts::prelude::NamedSerdeAnyMap,
    // chefs: those accounts/contracts controlled by the attacker
}

impl<SR: FuzzBcStateRef> UsesInput for FuzzState<SR> {
    type Input = MsgCallSeq<MemoryBcState<SR>>;
}

impl<SR: FuzzBcStateRef> HasLastReportTime for FuzzState<SR> {
    fn last_report_time(&self) -> &Option<Duration> {
        &self.last_report_time
    }

    fn last_report_time_mut(&mut self) -> &mut Option<Duration> {
        &mut self.last_report_time
    }
}

impl<SR: FuzzBcStateRef> HasExecutions for FuzzState<SR> {
    fn executions(&self) -> &usize {
        &self.executions
    }

    fn executions_mut(&mut self) -> &mut usize {
        &mut self.executions
    }
}

impl<SR: FuzzBcStateRef> HasMetadata for FuzzState<SR> {
    fn metadata_map(&self) -> &libafl_bolts::prelude::SerdeAnyMap {
        &self.metadata
    }

    fn metadata_map_mut(&mut self) -> &mut libafl_bolts::prelude::SerdeAnyMap {
        &mut self.metadata
    }
}

impl<SR: FuzzBcStateRef> HasNamedMetadata for FuzzState<SR> {
    fn named_metadata_map(&self) -> &libafl_bolts::prelude::NamedSerdeAnyMap {
        &self.named_metadata
    }

    fn named_metadata_map_mut(
        &mut self,
    ) -> &mut libafl_bolts::prelude::NamedSerdeAnyMap {
        &mut self.named_metadata
    }
}

impl<SR: FuzzBcStateRef> State for FuzzState<SR> {}
