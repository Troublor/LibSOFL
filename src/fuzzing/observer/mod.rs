use libafl::prelude::{Observer, ObserversTuple, UsesInput};
use revm::Database;
use revm_primitives::ExecutionResult;

use crate::engine::inspectors::MultiTxInspector;

pub mod result;
pub mod trace;

pub trait EvmObserver<S, BS>: Observer<S>
where
    S: UsesInput,
    BS: Database,
{
    type Inspector: MultiTxInspector<BS>;

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
    type Inspector: MultiTxInspector<BS>;

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
    type Inspector = ();

    fn get_inspector(
        &mut self,
        _pre_state: &BS,
        _input: &S::Input,
    ) -> Result<Self::Inspector, libafl::Error> {
        Ok(())
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
    type Inspector = (Head::Inspector, Tail::Inspector);

    fn get_inspector(
        &mut self,
        pre_state: &BS,
        input: &<S as UsesInput>::Input,
    ) -> Result<Self::Inspector, libafl::Error> {
        let insp = self.0.get_inspector(pre_state, input)?;
        let insps = self.1.get_inspector(pre_state, input)?;
        Ok((insp, insps))
    }

    fn on_executed(
        &mut self,
        post_state: &BS,
        inspector: Self::Inspector,
        results: Vec<ExecutionResult>,
        input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        let (insp, insps) = inspector;
        self.0
            .on_executed(post_state, insp, results.clone(), input)?;
        self.1.on_executed(post_state, insps, results, input)
    }
}
