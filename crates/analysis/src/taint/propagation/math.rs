use libsofl_core::{
    conversion::ConvertTo,
    engine::{state::BcState, types::opcode},
};

use crate::taint::{policy::TaintPolicy, stack::OPCODE_STACK_DELTA};

#[derive(Debug, Clone, Default)]
pub struct MathPolicy {}

impl<S: BcState> TaintPolicy<S> for MathPolicy {
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

#[cfg(test)]
mod tests {
    use alloy_sol_types::{sol_data, SolType};
    use libsofl_core::{
        conversion::ConvertTo,
        engine::{
            memory::MemoryBcState,
            state::BcState,
            types::{opcode, Address, U256},
        },
    };
    use libsofl_utils::solidity::{
        caller::HighLevelCaller, scripting::compile_yul,
    };

    use crate::{
        policies,
        taint::{
            policy::TaintPolicy, propagation::execution::ExecutionPolicy,
            TaintAnalyzer,
        },
    };

    use super::MathPolicy;

    #[derive(Debug, Clone, Default)]
    struct TaintOracle {
        pub tainted: bool,
    }

    impl<S: BcState> TaintPolicy<S> for TaintOracle {
        fn before_step(
            &mut self,
            taint_tracker: &mut crate::taint::TaintTracker,
            interp: &mut libsofl_core::engine::types::Interpreter<'_>,
            _data: &mut libsofl_core::engine::types::EVMData<'_, S>,
        ) -> Vec<Option<bool>> {
            match interp.current_opcode() {
                opcode::CALLDATALOAD => {
                    vec![Some(true)]
                }
                opcode::RETURN => {
                    stack_borrow!(interp, offset, len);
                    let offset = offset.cvt();
                    let len = len.cvt();
                    self.tainted = taint_tracker.memory.is_tainted(offset, len);
                    vec![]
                }
                _ => vec![],
            }
        }
    }

    #[test]
    fn test_arith() {
        let mut state = MemoryBcState::fresh();
        let mut oracle = TaintOracle::default();
        let mut analyzer = TaintAnalyzer::new(
            policies!(
                ExecutionPolicy::default(),
                MathPolicy::default(),
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
                let y := add(x, 1)
                let z := div(y, 2)
                mstore(0, z)
                return(0, 0x20)
            }
        }
        "#,
        )
        .unwrap()
        .remove(0);
        let contract = Address::ZERO;
        let calldata =
            sol_data::Uint::<256>::abi_encode(&ConvertTo::<U256>::cvt(&199u64));
        state.replace_account_code(contract, code.cvt()).unwrap();
        HighLevelCaller::default()
            .bypass_check()
            .call(&mut state, contract, calldata.cvt(), None, &mut analyzer)
            .unwrap();
        assert!(oracle.tainted);
    }

    #[test]
    fn test_logic() {
        let mut state = MemoryBcState::fresh();
        let mut oracle = TaintOracle::default();
        let mut analyzer = TaintAnalyzer::new(
            policies!(
                ExecutionPolicy::default(),
                MathPolicy::default(),
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
                let y := xor(x, 1)
                let z := and(y, 2)
                mstore(0, z)
                return(0, 0x20)
            }
        }
        "#,
        )
        .unwrap()
        .remove(0);
        let contract = Address::ZERO;
        let calldata =
            sol_data::Uint::<256>::abi_encode(&ConvertTo::<U256>::cvt(&199u64));
        state.replace_account_code(contract, code.cvt()).unwrap();
        HighLevelCaller::default()
            .bypass_check()
            .call(&mut state, contract, calldata.cvt(), None, &mut analyzer)
            .unwrap();
        assert!(oracle.tainted);
    }
}
