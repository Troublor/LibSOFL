use libsofl_core::{
    conversion::ConvertTo,
    engine::{state::BcState, types::opcode},
};

use crate::taint::policy::TaintPolicy;

#[derive(Debug, Clone, Default)]
pub struct NestedCallPolicy {}

impl<S: BcState> TaintPolicy<S> for NestedCallPolicy {
    #[inline]
    fn before_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        interp: &mut libsofl_core::engine::types::Interpreter<'_>,
        _data: &mut libsofl_core::engine::types::EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        match interp.current_opcode() {
            opcode::RETURNDATASIZE => {
                vec![Some(false)]
            }
            opcode::RETURNDATACOPY => {
                stack_borrow!(interp, dest, offset, len);
                if taint_tracker.stack.any_tainted(3) {
                    taint_tracker.memory.taint(dest.cvt(), len.cvt());
                } else {
                    let len = len.cvt();
                    let tainted = taint_tracker
                        .child_call
                        .as_ref()
                        .expect("invalid bytecode: child_call is None")
                        .return_data
                        .is_tainted(offset.cvt(), len);
                    if tainted {
                        taint_tracker.memory.taint(dest.cvt(), len);
                    }
                }
                vec![]
            }
            opcode::CREATE => {
                taint_stack_borrow!(
                    taint_tracker.stack,
                    value_t,
                    _offset,
                    _len
                );
                let child_call = taint_tracker
                    .child_call
                    .as_mut()
                    .expect("invalid bytecode: child_call is None");
                child_call.value = *value_t;
                stack_borrow!(interp, _value, offset, len);
                child_call.code =
                    taint_tracker.memory.slice(offset.cvt(), len.cvt());
                vec![Some(false)]
            }
            opcode::CALL | opcode::CALLCODE => {
                taint_stack_borrow!(
                    taint_tracker.stack,
                    gas_t,
                    _addr_t,
                    value_t,
                    _arg_offset_t,
                    _arg_len_t,
                    _ret_offset_t,
                    _ret_len_t
                );
                stack_borrow!(
                    interp,
                    _gas,
                    _addr,
                    _value,
                    arg_offset,
                    arg_len,
                    _ret_offset,
                    _ret_len
                );
                let child_call = taint_tracker
                    .child_call
                    .as_mut()
                    .expect("invalid bytecode: child_call is None");
                child_call.value = *value_t;
                child_call.gas = *gas_t;
                child_call.calldata =
                    taint_tracker.memory.slice(arg_offset.cvt(), arg_len.cvt());
                vec![None]
            }
            opcode::RETURN => {
                stack_borrow!(interp, offset, size);
                taint_tracker.call.return_data =
                    taint_tracker.memory.slice(offset.cvt(), size.cvt());
                vec![]
            }
            opcode::DELEGATECALL => {
                taint_stack_borrow!(
                    taint_tracker.stack,
                    gas_t,
                    _addr_t,
                    _arg_offset_t,
                    _arg_len_t,
                    _ret_offset_t,
                    _ret_len_t
                );
                stack_borrow!(
                    interp,
                    _gas,
                    _addr,
                    arg_offset,
                    arg_len,
                    _ret_offset,
                    _ret_len
                );
                let child_call = taint_tracker
                    .child_call
                    .as_mut()
                    .expect("invalid bytecode: child_call is None");
                child_call.gas = *gas_t;
                child_call.value = taint_tracker.call.value;
                child_call.calldata =
                    taint_tracker.memory.slice(arg_offset.cvt(), arg_len.cvt());
                vec![None]
            }
            opcode::CREATE2 => {
                taint_stack_borrow!(
                    taint_tracker.stack,
                    value_t,
                    _offset_t,
                    _len_t,
                    salt_t
                );
                let child_call = taint_tracker
                    .child_call
                    .as_mut()
                    .expect("invalid bytecode: child_call is None");
                child_call.value = *value_t;
                stack_borrow!(interp, _value, _salt, offset, len);
                child_call.code =
                    taint_tracker.memory.slice(offset.cvt(), len.cvt());
                vec![Some(*salt_t)]
            }
            opcode::STATICCALL => {
                taint_stack_borrow!(
                    taint_tracker.stack,
                    gas_t,
                    _addr_t,
                    _arg_offset_t,
                    _arg_len_t,
                    _ret_offset_t,
                    _ret_len_t
                );
                stack_borrow!(
                    interp,
                    _gas,
                    _addr,
                    arg_offset,
                    arg_len,
                    _ret_offset,
                    _ret_len
                );
                let child_call = taint_tracker
                    .child_call
                    .as_mut()
                    .expect("invalid bytecode: child_call is None");
                child_call.gas = *gas_t;
                child_call.calldata =
                    taint_tracker.memory.slice(arg_offset.cvt(), arg_len.cvt());
                vec![None]
            }
            opcode::REVERT => {
                stack_borrow!(interp, offset, size);
                taint_tracker.call.return_data =
                    taint_tracker.memory.slice(offset.cvt(), size.cvt());
                vec![]
            }
            opcode::INVALID | opcode::SELFDESTRUCT => vec![],
            _ => vec![],
        }
    }

    #[inline]
    fn after_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        op: u8,
        _interp: &mut libsofl_core::engine::types::Interpreter<'_>,
        _data: &mut libsofl_core::engine::types::EVMData<'_, S>,
    ) {
        match op {
            opcode::CALL
            | opcode::CALLCODE
            | opcode::DELEGATECALL
            | opcode::STATICCALL => {
                let child_call = taint_tracker
                    .child_call
                    .as_ref()
                    .expect("invalid bytecode: child_call is None");
                if child_call.status {
                    taint_tracker.stack.taint(0)
                }
            }
            _ => {}
        }
    }
}
