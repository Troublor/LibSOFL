use libsofl_core::{
    conversion::ConvertTo,
    engine::{state::BcState, types::opcode},
};

use crate::taint::policy::TaintPolicy;

#[derive(Debug, Clone, Default)]
pub struct TxInputSource {}

impl<S: BcState> TaintPolicy<S> for TxInputSource {
    #[inline]
    fn before_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        interp: &mut libsofl_core::engine::types::Interpreter,
        data: &mut libsofl_core::engine::types::EvmContext<S>,
    ) -> Vec<Option<bool>> {
        // only the first in the call stack is the taint source
        if data.journaled_state.depth() > 1 {
            return vec![];
        }
        match interp.current_opcode() {
            opcode::CALLDATALOAD | opcode::CALLDATASIZE => {
                vec![Some(true)]
            }
            opcode::CALLDATACOPY => {
                stack_borrow!(interp, dest, _offset, len);
                let dest = dest.cvt();
                let len = len.cvt();
                taint_tracker.memory.taint(dest, len);
                vec![]
            }
            _ => vec![],
        }
    }
}
