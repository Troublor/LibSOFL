use libsofl_core::{
    conversion::ConvertTo,
    engine::{state::BcState, types::opcode},
};

use super::PropagationPolicy;

pub struct SystemPolicy {}

impl<S: BcState> PropagationPolicy<S> for SystemPolicy {
    #[inline]
    fn before_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        interp: &mut libsofl_core::engine::types::Interpreter<'_>,
        data: &mut libsofl_core::engine::types::EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        match interp.current_opcode() {
            opcode::KECCAK256 => {
                let operand_tainted = taint_tracker.stack.any_tainted(2);
                if operand_tainted {
                    // taint if the memory location is tainted
                    vec![Some(true)]
                } else {
                    // taint if the memory data is tainted
                    stack_pop!(interp, from, len);
                    let tainted =
                        taint_tracker.memory.is_tainted(from.cvt(), len.cvt());
                    vec![Some(tainted)]
                }
            }
            opcode::ADDRESS | opcode::CALLER | opcode::CALLVALUE => {
                vec![Some(false)]
            }
            opcode::CALLDATALOAD => {
                // taint if the memory location is tainted.
                // we don't consider the input may introduce taint since this is propagation policy.
                // Defining taint source should be done by users.
                todo!()
                // vec![Some()]
            }
            opcode::CALLDATASIZE => {
                vec![Some(false)]
            }
            opcode::CALLDATACOPY => {
                // taint if the memory location is tainted

                vec![Some(true)]
            }

            _ => Vec::new(),
        }
    }
}
