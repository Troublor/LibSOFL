use libsofl_core::{
    conversion::ConvertTo,
    engine::{state::BcState, types::opcode},
};

use crate::taint::stack::OPCODE_STACK_DELTA;

use super::PropagationPolicy;

pub struct MathPolicy {}

impl<S: BcState> PropagationPolicy<S> for MathPolicy {
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
            opcode::LT
            | opcode::GT
            | opcode::SLT
            | opcode::SGT
            | opcode::EQ
            | opcode::ISZERO
            | opcode::AND
            | opcode::OR
            | opcode::XOR
            | opcode::NOT
            | opcode::BYTE
            | opcode::SHL
            | opcode::SHR
            | opcode::SAR => {
                let tainted = taint_tracker
                    .stack
                    .any_tainted(OPCODE_STACK_DELTA[op as usize].0);
                vec![Some(tainted)]
            }
            opcode::KECCAK256 => {
                let operand_tainted = taint_tracker.stack.any_tainted(2);
                if operand_tainted {
                    // taint if the memory location is tainted
                    vec![Some(true)]
                } else {
                    // taint if the memory data is tainted
                    stack_borrow!(interp, from, len);
                    let tainted =
                        taint_tracker.memory.is_tainted(from.cvt(), len.cvt());
                    vec![Some(tainted)]
                }
            }
            _ => Vec::new(),
        }
    }
}
