use libafl::prelude::{
    DifferentialObserver, DifferentialObserversTuple, Observer, ObserversTuple,
    UsesInput,
};
use revm::{inspectors::NoOpInspector, Database};
use revm_primitives::ExecutionResult;

use crate::engine::inspectors::{
    InspectorTuple, InspectorWithTxHook, NoInspector,
};

pub mod asset_flow;
pub mod result;
pub mod trace;

pub trait EvmObserver<S, BS>: Observer<S>
where
    S: UsesInput,
    BS: Database,
{
    type Inspector: InspectorWithTxHook<BS>;

    /// Get the EVM inspector of associated with observer.
    /// The inspector is used to inspect the EVM state during transaction execution.
    /// EVMObserver implementations may need to reset the inspector in [Observer::pre_exec].
    /// * `input` - The input of the current fuzzer exection.
    /// * `index` - The index of the tranasction to be executed in the input, incase the input contains a sequence of transactions.
    fn get_inspector(
        &mut self,
        pre_state: &BS,
        input: &S::Input,
    ) -> Result<Self::Inspector, libafl::Error>;

    /// A callback function fed with the EVM execution result.
    /// The callback is called after each transaction execution.
    /// If one fuzzer input contains a sequence of transactions, the callback is called after each transaction execution.
    /// * `inspector` - The EVM inspector associated with the observer.
    /// * `result` - The execution result of the transaction.
    /// * `input` - The input of the current fuzzer exection.
    /// * `index` - The index of the tranasction to be executed in the input, incase the input contains a sequence of transactions.
    fn on_executed(
        &mut self,
        _post_state: &BS,
        _inspector: Self::Inspector,
        _results: Vec<ExecutionResult>,
        _input: &S::Input,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}

pub trait EvmObserversTuple<S: UsesInput, BS: Database>:
    ObserversTuple<S>
{
    type Inspector: InspectorWithTxHook<BS>;

    fn get_inspector(
        &mut self,
        pre_state: &BS,
        input: &S::Input,
    ) -> Result<Self::Inspector, libafl::Error>;

    fn on_executed(
        &mut self,
        post_state: &BS,
        inspector: Self::Inspector,
        results: Vec<ExecutionResult>,
        input: &S::Input,
    ) -> Result<(), libafl::Error>;
}

impl<S: UsesInput, BS: Database> EvmObserversTuple<S, BS> for () {
    type Inspector = NoOpInspector;

    fn get_inspector(
        &mut self,
        _pre_state: &BS,
        _input: &S::Input,
    ) -> Result<Self::Inspector, libafl::Error> {
        Ok(NoOpInspector {})
    }

    fn on_executed(
        &mut self,
        _post_state: &BS,
        _inspector: Self::Inspector,
        _results: Vec<ExecutionResult>,
        _input: &S::Input,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}

impl<S, BS, Head, Tail> EvmObserversTuple<S, BS> for (Head, Tail)
where
    S: UsesInput,
    BS: Database + revm::Database,
    Head: EvmObserver<S, BS>,
    Tail: EvmObserversTuple<S, BS>,
{
    type Inspector = InspectorTuple<BS, Head::Inspector, Tail::Inspector>;

    fn get_inspector(
        &mut self,
        pre_state: &BS,
        input: &<S as UsesInput>::Input,
    ) -> Result<Self::Inspector, libafl::Error> {
        let insp = self.0.get_inspector(pre_state, input)?;
        let insps = self.1.get_inspector(pre_state, input)?;
        Ok(InspectorTuple::new(insp, insps))
    }

    fn on_executed(
        &mut self,
        post_state: &BS,
        inspector: Self::Inspector,
        results: Vec<ExecutionResult>,
        input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        let (insp, insps) = inspector.into();
        self.0
            .on_executed(post_state, insp, results.clone(), input)?;
        self.1.on_executed(post_state, insps, results, input)
    }
}

pub trait DifferentialEvmObserver<S, BS, OTA, OTB>:
    DifferentialObserver<OTA, OTB, S>
where
    S: UsesInput,
    BS: Database,
    OTA: EvmObserversTuple<S, BS>,
    OTB: EvmObserversTuple<S, BS>,
{
    fn get_first_inspector(
        &mut self,
        pre_state: &BS,
        input: &S::Input,
    ) -> Result<OTA::Inspector, libafl::Error>;

    fn get_second_inspector(
        &mut self,
        pre_state: &BS,
        input: &S::Input,
    ) -> Result<OTB::Inspector, libafl::Error>;

    fn on_first_executed(
        &mut self,
        _post_state: &BS,
        _inspector: OTA::Inspector,
        _results: Vec<ExecutionResult>,
        _input: &S::Input,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }

    fn on_second_executed(
        &mut self,
        _post_state: &BS,
        _inspector: OTB::Inspector,
        _results: Vec<ExecutionResult>,
        _input: &S::Input,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}

pub trait DifferentialEvmObserverTuple<S, BS, OTA, OTB>:
    DifferentialObserversTuple<OTA, OTB, S>
where
    S: UsesInput,
    BS: Database,
    OTA: EvmObserversTuple<S, BS>,
    OTB: EvmObserversTuple<S, BS>,
{
    type FirstInspector: InspectorWithTxHook<BS>;
    type SecondInspector: InspectorWithTxHook<BS>;

    fn get_first_inspector(
        &mut self,
        pre_state: &BS,
        input: &S::Input,
    ) -> Result<Self::FirstInspector, libafl::Error>;

    fn get_second_inspector(
        &mut self,
        pre_state: &BS,
        input: &S::Input,
    ) -> Result<Self::SecondInspector, libafl::Error>;

    fn on_first_executed(
        &mut self,
        _post_state: &BS,
        _inspector: Self::FirstInspector,
        _results: Vec<ExecutionResult>,
        _input: &S::Input,
    ) -> Result<(), libafl::Error>;

    fn on_second_executed(
        &mut self,
        _post_state: &BS,
        _inspector: Self::SecondInspector,
        _results: Vec<ExecutionResult>,
        _input: &S::Input,
    ) -> Result<(), libafl::Error>;
}

impl<S, BS, OTA, OTB> DifferentialEvmObserverTuple<S, BS, OTA, OTB> for ()
where
    S: UsesInput,
    BS: Database,
    OTA: EvmObserversTuple<S, BS>,
    OTB: EvmObserversTuple<S, BS>,
{
    type FirstInspector = NoInspector;
    type SecondInspector = NoInspector;

    fn get_first_inspector(
        &mut self,
        _pre_state: &BS,
        _input: &<S as UsesInput>::Input,
    ) -> Result<NoOpInspector, libafl::Error> {
        Ok(NoInspector {})
    }

    fn get_second_inspector(
        &mut self,
        _pre_state: &BS,
        _input: &<S as UsesInput>::Input,
    ) -> Result<NoOpInspector, libafl::Error> {
        Ok(NoInspector {})
    }

    fn on_first_executed(
        &mut self,
        _post_state: &BS,
        _inspector: NoInspector,
        _results: Vec<ExecutionResult>,
        _input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }

    fn on_second_executed(
        &mut self,
        _post_state: &BS,
        _inspector: NoInspector,
        _results: Vec<ExecutionResult>,
        _input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}

impl<S, BS, OTA, OTB, Head, Tail> DifferentialEvmObserverTuple<S, BS, OTA, OTB>
    for (Head, Tail)
where
    S: UsesInput,
    BS: Database,
    OTA: EvmObserversTuple<S, BS>,
    OTB: EvmObserversTuple<S, BS>,
    Head: DifferentialEvmObserver<S, BS, OTA, OTB>,
    Tail: DifferentialEvmObserverTuple<S, BS, OTA, OTB>,
{
    type FirstInspector = InspectorTuple<
        BS,
        OTA::Inspector,
        <Tail as DifferentialEvmObserverTuple<S, BS, OTA, OTB>>::FirstInspector,
    >;
    type SecondInspector = InspectorTuple<BS,
        OTB::Inspector,
        <Tail as DifferentialEvmObserverTuple<S, BS, OTA, OTB>>::SecondInspector,
    >;

    fn get_first_inspector(
        &mut self,
        pre_state: &BS,
        input: &<S as UsesInput>::Input,
    ) -> Result<Self::FirstInspector, libafl::Error> {
        let insp = self.0.get_first_inspector(pre_state, input)?;
        let insps = self.1.get_first_inspector(pre_state, input)?;
        Ok(InspectorTuple::new(insp, insps))
    }

    fn get_second_inspector(
        &mut self,
        pre_state: &BS,
        input: &<S as UsesInput>::Input,
    ) -> Result<Self::SecondInspector, libafl::Error> {
        let insp = self.0.get_second_inspector(pre_state, input)?;
        let insps = self.1.get_second_inspector(pre_state, input)?;
        Ok(InspectorTuple::new(insp, insps))
    }

    fn on_first_executed(
        &mut self,
        post_state: &BS,
        inspector: Self::FirstInspector,
        results: Vec<ExecutionResult>,
        input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        let (insp, insps) = inspector.into();
        self.0
            .on_first_executed(post_state, insp, results.clone(), input)?;
        self.1.on_first_executed(post_state, insps, results, input)
    }

    fn on_second_executed(
        &mut self,
        post_state: &BS,
        inspector: Self::SecondInspector,
        results: Vec<ExecutionResult>,
        input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        let (insp, insps) = inspector.into();
        self.0
            .on_second_executed(post_state, insp, results.clone(), input)?;
        self.1.on_second_executed(post_state, insps, results, input)
    }
}
