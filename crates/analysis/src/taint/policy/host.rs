use libsofl_core::engine::{state::BcState, types::opcode};

use super::PropagationPolicy;

pub struct HostPolicy {}

impl<S: BcState> PropagationPolicy<S> for HostPolicy {
    fn before_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        interp: &mut libsofl_core::engine::types::Interpreter<'_>,
        data: &mut libsofl_core::engine::types::EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        match interp.current_opcode() {
            opcode::BALANCE => {
                if taint_tracker.stack.any_tainted(1) {
                    vec![Some(true)]
                } else {
                    vec![Some(false)]
                }
            }
            _ => Vec::new(),
        }
    }
}
