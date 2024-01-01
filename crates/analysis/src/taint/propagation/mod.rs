pub mod call;
pub mod env;
pub mod execution;
pub mod math;
pub mod nested_call;

use libsofl_core::engine::{
    state::BcState,
    types::{EVMData, Interpreter},
};

#[macro_export]
macro_rules! default_policy {
    () => {
        policies![
            crate::taint::propagation::math::MathPolicy::default(),
            crate::taint::propagation::env::EnvPolicy::default(),
            crate::taint::propagation::call::CallPolicy::default(),
            crate::taint::propagation::execution::ExecutionPolicy::default(),
            crate::taint::propagation::nested_call::NestedCallPolicy::default()
        ]
    };
}

#[macro_export]
macro_rules! policies {
    ($p:expr) => {
        $p
    };
    ($p1:expr, $p2:expr) => {
        ($p1, $p2)
    };
    ($p1:expr, $p2:expr, $($pTail:expr),+) => {
        ($p1, policies!($p2, $($pTail),+))
    };
}

use super::{policy::TaintPolicy, TaintTracker};

impl<S: BcState> TaintPolicy<S> for () {
    #[inline]
    fn before_step(
        &mut self,
        _taint_tracker: &mut TaintTracker,
        _interp: &mut Interpreter<'_>,
        _data: &mut EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        Vec::new()
    }

    #[inline]
    fn after_step(
        &mut self,
        _taint_tracker: &mut TaintTracker,
        _op: u8,
        _interp: &mut Interpreter<'_>,
        _data: &mut EVMData<'_, S>,
    ) {
    }
}

impl<S: BcState, P1: TaintPolicy<S>, P2: TaintPolicy<S>> TaintPolicy<S>
    for (P1, P2)
{
    /// Propagate taint before the execution of an instruction.
    /// First, propagate taint according to the first policy.
    /// Then, propagate taint according to the second policy.
    /// The taint stack effects of the two policies are combined with disjunction.
    /// In case the length of the taint stack effects of the two policies differ,
    /// the shorter one is padded with `false`.
    #[inline]
    fn before_step(
        &mut self,
        taint_tracker: &mut TaintTracker,
        interp: &mut Interpreter<'_>,
        data: &mut EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        let stack_taint0 = self.0.before_step(taint_tracker, interp, data);
        let stack_taint1 = self.1.before_step(taint_tracker, interp, data);
        let len = stack_taint0.len().max(stack_taint1.len());
        let stack_taint0 = (0..len - stack_taint0.len())
            .map(|_| None)
            .chain(stack_taint0.into_iter());
        let stack_taint1 = (0..len - stack_taint1.len())
            .map(|_| None)
            .chain(stack_taint1.into_iter());
        stack_taint0
            .zip(stack_taint1)
            .map(|(a, b)| {
                if a.is_none() && b.is_none() {
                    None
                } else if !a.is_none() && b.is_none() {
                    a
                } else if a.is_none() && !b.is_none() {
                    b
                } else {
                    Some(a.unwrap() || b.unwrap())
                }
            })
            .collect()
    }

    #[inline]
    fn after_step(
        &mut self,
        taint_tracker: &mut TaintTracker,
        op: u8,
        interp: &mut Interpreter<'_>,
        data: &mut EVMData<'_, S>,
    ) {
        self.0.after_step(taint_tracker, op, interp, data);
        self.1.after_step(taint_tracker, op, interp, data);
    }
}
