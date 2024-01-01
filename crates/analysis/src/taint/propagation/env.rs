use libsofl_core::{
    conversion::ConvertTo,
    engine::{state::BcState, types::opcode},
};

use crate::taint::policy::TaintPolicy;

#[derive(Debug, Clone, Default)]
pub struct EnvPolicy {}

impl<S: BcState> TaintPolicy<S> for EnvPolicy {
    #[inline]
    fn before_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        interp: &mut libsofl_core::engine::types::Interpreter<'_>,
        _data: &mut libsofl_core::engine::types::EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        match interp.current_opcode() {
            opcode::ADDRESS => {
                vec![Some(false)]
            }
            opcode::BALANCE => {
                if taint_tracker.stack.any_tainted(1) {
                    vec![Some(true)]
                } else {
                    vec![Some(false)]
                }
            }
            opcode::ORIGIN => {
                vec![Some(false)]
            }
            opcode::CODESIZE => {
                vec![Some(false)]
            }
            opcode::CODECOPY => {
                stack_borrow!(interp, dest, offset, len);
                if taint_tracker.stack.any_tainted(3) {
                    taint_tracker.memory.taint(dest.cvt(), len.cvt());
                } else {
                    let len = len.cvt();
                    let tainted =
                        taint_tracker.call.code.is_tainted(offset.cvt(), len);
                    if tainted {
                        taint_tracker.memory.taint(dest.cvt(), len);
                    }
                }
                vec![]
            }
            opcode::GASPRICE => {
                vec![Some(false)]
            }
            opcode::EXTCODESIZE => {
                if taint_tracker.stack.any_tainted(1) {
                    vec![Some(true)]
                } else {
                    vec![Some(false)]
                }
            }
            opcode::EXTCODECOPY => {
                stack_borrow!(interp, _addr, dest, _offset, len);
                if taint_tracker.stack.any_tainted(4) {
                    taint_tracker.memory.taint(dest.cvt(), len.cvt());
                }
                vec![]
            }
            opcode::EXTCODEHASH => {
                if taint_tracker.stack.any_tainted(1) {
                    vec![Some(true)]
                } else {
                    vec![Some(false)]
                }
            }
            opcode::BLOCKHASH
            | opcode::COINBASE
            | opcode::TIMESTAMP
            | opcode::NUMBER
            | opcode::DIFFICULTY
            | opcode::GASLIMIT
            | opcode::CHAINID
            | opcode::SELFBALANCE
            | opcode::BASEFEE
            | opcode::BLOBHASH
            | opcode::BLOBBASEFEE => {
                vec![Some(false)]
            }
            _ => Vec::new(),
        }
    }
}
