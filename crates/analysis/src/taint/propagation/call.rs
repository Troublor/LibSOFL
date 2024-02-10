use libsofl_core::{
    conversion::ConvertTo,
    engine::{state::BcState, types::opcode},
};

use crate::taint::policy::TaintPolicy;

#[derive(Debug, Clone, Default)]
pub struct CallPolicy {}

impl<S: BcState> TaintPolicy<S> for CallPolicy {
    #[inline]
    fn before_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        interp: &mut libsofl_core::engine::types::Interpreter,
        _data: &mut libsofl_core::engine::types::EvmContext<S>,
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

#[cfg(test)]
mod tests {
    use libsofl_core::solidity::{
        caller::HighLevelCaller, scripting::compile_yul,
    };
    use libsofl_core::{
        conversion::ConvertTo,
        engine::{
            memory::MemoryBcState,
            state::BcState,
            types::{opcode, Address},
        },
    };

    use crate::{
        policies,
        taint::{
            policy::TaintPolicy, propagation::execution::ExecutionPolicy,
            TaintAnalyzer,
        },
    };

    use super::CallPolicy;

    #[derive(Debug, Clone, Default)]
    struct TaintOracle {
        pub tainted: bool,
    }

    impl<S: BcState> TaintPolicy<S> for TaintOracle {
        fn before_step(
            &mut self,
            taint_tracker: &mut crate::taint::TaintTracker,
            interp: &mut libsofl_core::engine::types::Interpreter,
            _data: &mut libsofl_core::engine::types::EvmContext<S>,
        ) -> Vec<Option<bool>> {
            taint_tracker.call.calldata.taint(0, 10240);
            let op = interp.current_opcode();
            match op {
                opcode::CALLDATACOPY => {
                    stack_borrow!(interp, dest, _offset, len);
                    self.tainted =
                        taint_tracker.memory.is_tainted(dest.cvt(), len.cvt());
                }
                _ => {}
            }
            vec![]
        }
        fn after_step(
            &mut self,
            taint_tracker: &mut crate::taint::TaintTracker,
            op: u8,
            _interp: &mut libsofl_core::engine::types::Interpreter,
            _data: &mut libsofl_core::engine::types::EvmContext<S>,
        ) {
            match op {
                opcode::CALLDATALOAD => {
                    self.tainted = taint_tracker.stack.any_tainted(1);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_call_data_load() {
        let mut oracle = TaintOracle::default();
        let mut state = MemoryBcState::fresh();
        let mut analyzer = TaintAnalyzer::new(
            policies!(
                ExecutionPolicy::default(),
                CallPolicy::default(),
                &mut oracle
            ),
            32,
        );
        let (_, code) = compile_yul(
            "0.8.12",
            r#"
        object "A" {
            code {
                let x := calldataload(0)
                let y := calldataload(0x20)
                return(x, y)
            }
        }
        "#,
        )
        .unwrap()
        .remove(0);
        let contract = Address::ZERO;
        state.replace_account_code(contract, code.cvt()).unwrap();
        let mut calldata: Vec<u8> = Vec::new();
        calldata.resize(0x40, 0);
        HighLevelCaller::default()
            .bypass_check()
            .call(&mut state, contract, calldata.cvt(), None, &mut analyzer)
            .unwrap();
        assert!(oracle.tainted);
    }

    #[test]
    fn test_call_data_copy() {
        let mut oracle = TaintOracle::default();
        let mut state = MemoryBcState::fresh();
        let mut analyzer = TaintAnalyzer::new(
            policies!(
                ExecutionPolicy::default(),
                CallPolicy::default(),
                &mut oracle
            ),
            32,
        );
        let (_, code) = compile_yul(
            "0.8.12",
            r#"
        object "A" {
            code {
                let x := 0
                let y := 0x20
                calldatacopy(x, y, 0x20)
                return(x, 0x20)
            }
        }
        "#,
        )
        .unwrap()
        .remove(0);
        let contract = Address::ZERO;
        state.replace_account_code(contract, code.cvt()).unwrap();
        let mut calldata: Vec<u8> = Vec::new();
        calldata.resize(0x40, 0);
        HighLevelCaller::default()
            .bypass_check()
            .call(&mut state, contract, calldata.cvt(), None, &mut analyzer)
            .unwrap();
        assert!(oracle.tainted);
    }
}
