use libsofl_core::{
    conversion::ConvertTo,
    engine::{state::BcState, types::opcode},
};

use crate::taint::policy::TaintPolicy;

#[derive(Debug, Clone, Default)]
pub struct ExecutionPolicy {}

impl<S: BcState> TaintPolicy<S> for ExecutionPolicy {
    #[inline]
    fn before_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        interp: &mut libsofl_core::engine::types::Interpreter<'_>,
        _data: &mut libsofl_core::engine::types::EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        match interp.current_opcode() {
            opcode::POP => {
                vec![]
            }
            opcode::MLOAD => {
                if taint_tracker.stack.any_tainted(1) {
                    vec![Some(true)]
                } else {
                    stack_borrow!(interp, offset);
                    let tainted =
                        taint_tracker.memory.is_tainted(offset.cvt(), 32);
                    vec![Some(tainted)]
                }
            }
            opcode::MSTORE => {
                stack_borrow!(interp, offset, _value);
                let tainted = taint_tracker.stack.any_tainted(2);
                if tainted {
                    taint_tracker.memory.taint(offset.cvt(), 32);
                }
                vec![]
            }
            opcode::MSTORE8 => {
                stack_borrow!(interp, offset, _value);
                let tainted = taint_tracker.stack.any_tainted(2);
                if tainted {
                    taint_tracker.memory.taint(offset.cvt(), 1);
                }
                vec![]
            }
            opcode::SLOAD => {
                if taint_tracker.stack.any_tainted(1) {
                    vec![Some(true)]
                } else {
                    stack_borrow!(interp, key);
                    let tainted = taint_tracker.storage.is_tainted(*key);
                    vec![Some(tainted)]
                }
            }
            opcode::SSTORE => {
                stack_borrow!(interp, key, _value);
                let tainted = taint_tracker.stack.any_tainted(2);
                if tainted {
                    taint_tracker.storage.taint(*key);
                }
                vec![]
            }
            opcode::JUMP | opcode::JUMPI => vec![],
            opcode::PC | opcode::MSIZE | opcode::GAS => vec![Some(false)],
            opcode::JUMPDEST => vec![],
            opcode::TLOAD => {
                if taint_tracker.stack.any_tainted(1) {
                    vec![Some(true)]
                } else {
                    stack_borrow!(interp, key);
                    let tainted = taint_tracker.storage.is_tainted(*key);
                    vec![Some(tainted)]
                }
            }
            opcode::TSTORE => {
                stack_borrow!(interp, key, _value);
                let tainted = taint_tracker.stack.any_tainted(2);
                if tainted {
                    taint_tracker.storage.taint(*key);
                }
                vec![]
            }
            opcode::MCOPY => {
                stack_borrow!(interp, dest, src, len);
                if taint_tracker.stack.any_tainted(3) {
                    taint_tracker.memory.taint(dest.cvt(), len.cvt());
                } else {
                    let len = len.cvt();
                    let tainted =
                        taint_tracker.memory.is_tainted(src.cvt(), len);
                    if tainted {
                        taint_tracker.memory.taint(dest.cvt(), len);
                    }
                }
                vec![]
            }
            _ => vec![],
        }
    }
}
