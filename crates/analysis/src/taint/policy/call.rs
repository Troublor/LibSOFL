use libsofl_core::{
    conversion::ConvertTo,
    engine::{state::BcState, types::opcode},
};

use super::PropagationPolicy;

pub struct CallPolicy {}

impl<S: BcState> PropagationPolicy<S> for CallPolicy {
    #[inline]
    fn before_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        interp: &mut libsofl_core::engine::types::Interpreter<'_>,
        _data: &mut libsofl_core::engine::types::EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        match interp.current_opcode() {
            opcode::CALLER => {
                vec![Some(taint_tracker.call.caller)]
            }
            opcode::CALLVALUE => {
                vec![Some(taint_tracker.call.value)]
            }
            opcode::CALLDATALOAD => {
                let operand_tainted = taint_tracker.stack.any_tainted(1);
                if operand_tainted {
                    // taint if the calldata location is tainted.
                    vec![Some(true)]
                } else {
                    // taint if the calldata data is tainted.
                    stack_borrow!(interp, offset);
                    let tainted = taint_tracker
                        .call
                        .calldata
                        .is_tainted(offset.cvt(), 32);
                    vec![Some(tainted)]
                }
            }
            opcode::CALLDATASIZE => {
                vec![Some(false)]
            }
            opcode::CALLDATACOPY => {
                // taint if the calldata location is tainted
                stack_borrow!(interp, dest, offset, len);
                let dest = dest.cvt();
                let offset = offset.cvt();
                let len = len.cvt();
                let operand_tainted = taint_tracker.stack.any_tainted(3);
                if operand_tainted {
                    taint_tracker.memory.taint(dest, len);
                } else {
                    // taint if the calldata is tainted
                    let tainted =
                        taint_tracker.call.calldata.is_tainted(offset, len);
                    if tainted {
                        taint_tracker.memory.taint(dest, len);
                    }
                }
                vec![]
            }
            _ => vec![],
        }
    }
}
