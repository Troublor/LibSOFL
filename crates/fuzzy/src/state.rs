use std::time::Duration;

use libafl::{
    corpus::{InMemoryCorpus, OnDiskCorpus},
    inputs::UsesInput,
    state::{
        HasExecutions, HasLastReportTime, HasMetadata, HasNamedMetadata, State,
    },
};
use libafl_bolts::rands::RomuDuoJrRand;
use libsofl_core::engine::{memory::MemoryBcState, types::Address};

use crate::{
    blockchain::state_ref::FuzzBcStateRef, ingredient::pentry::IngrediantPantry, input::{FuzzInput, MsgCallSeq}
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
    pub rand: RomuDuoJrRand,

    /// corpus
    pub corpus: InMemoryCorpus<FuzzInput<SR>>,

    /// solutions
    pub solutions: OnDiskCorpus<FuzzInput<SR>>,

    /// number of executions
    pub executions: usize,

    /// last report time
    pub last_report_time: Option<Duration>,

    /// arbitrary metadata
    pub metadata: libafl_bolts::prelude::SerdeAnyMap,
    pub named_metadata: libafl_bolts::prelude::NamedSerdeAnyMap,

    // attacker-owned-accounts: those accounts/contracts controlled by the attacker
    pub attacker_accounts: IngrediantPantry<RomuDuoJrRand, Address>,
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
