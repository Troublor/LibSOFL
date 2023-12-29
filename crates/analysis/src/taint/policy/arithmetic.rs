use libsofl_core::engine::{state::BcState, types::opcode};

use crate::taint::stack::OPCODE_STACK_DELTA;

use super::PropagationPolicy;

pub struct ArithmeticPolicy {}

impl<S: BcState> PropagationPolicy<S> for ArithmeticPolicy {
    #[inline]
    fn before_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        interp: &mut libsofl_core::engine::types::Interpreter<'_>,
        _data: &mut libsofl_core::engine::types::EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        let op = interp.current_opcode();
        match op {
            opcode::ADD
            | opcode::MUL
            | opcode::SUB
            | opcode::DIV
            | opcode::SDIV
            | opcode::MOD
            | opcode::SMOD
            | opcode::ADDMOD
            | opcode::MULMOD
            | opcode::EXP
            | opcode::SIGNEXTEND => {
                let tainted = taint_tracker
                    .stack
                    .any_tainted(OPCODE_STACK_DELTA[op as usize].0);
                vec![Some(tainted)]
            }
            _ => Vec::new(),
        }
    }
}
