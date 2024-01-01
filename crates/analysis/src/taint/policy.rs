use libsofl_core::engine::{
    state::BcState,
    types::{EVMData, Interpreter},
};

use crate::taint::TaintTracker;

#[auto_impl::auto_impl(&mut, Box)]
pub trait TaintPolicy<S: BcState> {
    /// Propagate taint before the execution of an instruction.
    /// The returned vector contains the stack taint effects of the instruction.
    /// The stack taint effects specifies which stack elements are tainted after the execution of the instruction.
    /// The returned vector is considered to match the stack top, i.e., the last element of the vector is the top of the stack.
    /// True means that the stack element at the position should be tainted.
    /// False means that the stack element at the position should be clean.
    /// None means that the stack element at the position should be left unchanged.
    fn before_step(
        &mut self,
        _taint_tracker: &mut TaintTracker,
        _interp: &mut Interpreter<'_>,
        _data: &mut EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        vec![]
    }

    /// Propagate taint after the execution of an instruction.
    fn after_step(
        &mut self,
        _taint_tracker: &mut TaintTracker,
        _op: u8,
        _interp: &mut Interpreter<'_>,
        _data: &mut EVMData<'_, S>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use libsofl_core::engine::{
        memory::MemoryBcState, state::BcState,
        transition::TransitionSpecBuilder,
    };

    use crate::{
        policies,
        taint::propagation::{env::EnvPolicy, math::MathPolicy},
    };

    #[test]
    fn test_compose_multiple_policy() {
        let policy = policies![MathPolicy {}, EnvPolicy {}];
        let mut analyzer = super::super::TaintAnalyzer::new(policy, 32);
        let mut state = MemoryBcState::fresh();
        let spec = TransitionSpecBuilder::default().bypass_check().build();
        state.transit(spec, &mut analyzer).unwrap();
    }
}
