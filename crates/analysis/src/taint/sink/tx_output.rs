use libsofl_core::{
    conversion::ConvertTo,
    engine::{state::BcState, types::opcode},
};

use crate::taint::{memory::TaintableMemory, policy::TaintPolicy};

#[derive(Debug, Clone, Default)]
pub struct TxOutputSink {
    pub return_data: Option<TaintableMemory>,
    pub reverted: bool,
}

impl<S: BcState> TaintPolicy<S> for TxOutputSink {
    fn before_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        interp: &mut libsofl_core::engine::types::Interpreter<'_>,
        data: &mut libsofl_core::engine::types::EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        if data.journaled_state.depth() == 1 {
            match interp.current_opcode() {
                opcode::RETURN => {
                    stack_borrow!(interp, offset, len);
                    let offset = offset.cvt();
                    let len = len.cvt();
                    self.return_data
                        .replace(taint_tracker.memory.slice(offset, len));
                    self.reverted = false;
                }
                opcode::REVERT => {
                    stack_borrow!(interp, offset, len);
                    let offset = offset.cvt();
                    let len = len.cvt();
                    self.return_data
                        .replace(taint_tracker.memory.slice(offset, len));
                    self.reverted = true;
                }
                _ => {}
            }
        }
        vec![]
    }
}
