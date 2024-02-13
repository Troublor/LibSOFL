#[cfg(test)]
mod tests {
    use alloy_dyn_abi::{FunctionExt, JsonAbiExt};
    use alloy_json_abi::Function;
    use revm::Database;

    use crate::{
        conversion::ConvertTo,
        engine::{
            inspector::no_inspector,
            interruptable::{
                breakpoint::{
                    break_everywhere, break_nowhere, BreakpointResult,
                    RunResult,
                },
                evm::InterruptableEvm,
            },
            memory::MemoryBcState,
            transition::TransitionSpecBuilder,
            types::{
                Address, CallContext, CallInputs, CallScheme,
                InstructionResult, SpecId, TransactTo, Transfer, TxEnv, U256,
            },
        },
        solidity::scripting::deploy_contracts,
    };

    #[test]
    fn test_serialize_context() {
        let mut state = MemoryBcState::fresh();
        let sss = serde_json::to_string(&state).unwrap();
        state = serde_json::from_str(&sss).unwrap();
        let contract = r#"
        contract D {
            function foo() public returns (uint) {
                return 1;
            }
        }

        contract C {
            D d;

            constructor() {
                d = new D();
            }

            function foo() public returns (uint) {
                return d.foo();
            }
        }
        "#;
        let addr = deploy_contracts(
            &mut state,
            "0.8.12",
            contract,
            vec!["C"],
            Default::default(),
        )
        .unwrap()
        .remove(0);
        let func = Function::parse("foo() returns (uint)").unwrap();
        let input = func.abi_encode_input(&[]).unwrap();
        let spec = TransitionSpecBuilder::default()
            .bypass_check()
            .append_tx_env(TxEnv {
                transact_to: TransactTo::Call(addr),
                data: input.cvt(),
                ..Default::default()
            })
            .build();
        let mut evm =
            InterruptableEvm::new(SpecId::LATEST, state, spec, *no_inspector());
        let mut break_times = 0;
        let mut r = evm.run(break_everywhere()).unwrap();
        while let RunResult::Breakpoint(_) = r {
            break_times += 1;
            // let serialized = serde_json::to_string(&evm).unwrap();
            // evm = serde_json::from_str(&serialized).unwrap();
            r = evm.run(break_everywhere()).unwrap();
        }
        assert_eq!(break_times, 6);
        let RunResult::Done(output) = r else {
            panic!("unexpected result");
        };
        let output = output.1.output().unwrap();
        let ret: Vec<alloy_dyn_abi::DynSolValue> =
            func.abi_decode_output(&output, true).unwrap();
        assert_eq!(ret[0].as_uint(), Some((U256::from(1), 256)));

        let (mut state, inspector) = evm.take_state_and_inspector();
        assert!(state.basic(Address::ZERO).is_ok());
        assert_eq!(inspector, *no_inspector());
    }

    /// Test call a existing contract without breakpoint
    #[test]
    fn test_call_contract() {
        let mut state = MemoryBcState::fresh();
        let contract = r#"
        contract D {
            function foo() public returns (uint) {
                return 1;
            }
        }

        contract C {
            D d;

            constructor() {
                d = new D();
            }

            function foo() public returns (uint) {
                return d.foo();
            }
        }
        "#;
        let addr = deploy_contracts(
            &mut state,
            "0.8.12",
            contract,
            vec!["C"],
            Default::default(),
        )
        .unwrap()
        .remove(0);
        let func = Function::parse("foo() returns (uint)").unwrap();
        let input = func.abi_encode_input(&[]).unwrap();
        let spec = TransitionSpecBuilder::default()
            .bypass_check()
            .append_tx_env(TxEnv {
                transact_to: TransactTo::Call(addr),
                data: input.cvt(),
                ..Default::default()
            })
            .build();
        let mut evm =
            InterruptableEvm::new(SpecId::LATEST, state, spec, *no_inspector());
        let r = evm.run(break_nowhere()).unwrap();
        let RunResult::Done(output) = r else {
            panic!("unexpected result");
        };
        let output = output.1.output().unwrap();
        let ret = func.abi_decode_output(&output, true).unwrap();
        assert_eq!(ret[0].as_uint(), Some((U256::from(1), 256)));
    }

    #[test]
    fn test_call_contract_with_breakpoints() {
        let mut state = MemoryBcState::fresh();
        let contract = r#"
        contract D {
            function foo() public returns (uint) {
                return 1;
            }
        }

        contract C {
            D d;

            constructor() {
                d = new D();
            }

            function foo() public returns (uint) {
                return d.foo();
            }
        }
        "#;
        let addr = deploy_contracts(
            &mut state,
            "0.8.12",
            contract,
            vec!["C"],
            Default::default(),
        )
        .unwrap()
        .remove(0);
        let func = Function::parse("foo() returns (uint)").unwrap();
        let input = func.abi_encode_input(&[]).unwrap();
        let spec = TransitionSpecBuilder::default()
            .bypass_check()
            .append_tx_env(TxEnv {
                transact_to: TransactTo::Call(addr),
                data: input.cvt(),
                ..Default::default()
            })
            .build();
        let mut evm =
            InterruptableEvm::new(SpecId::LATEST, state, spec, *no_inspector());
        let mut break_times = 0;
        let mut r = evm.run(break_everywhere()).unwrap();
        while let RunResult::Breakpoint(_) = r {
            break_times += 1;
            r = evm.run(break_everywhere()).unwrap();
        }
        assert_eq!(break_times, 6);
        let RunResult::Done(output) = r else {
            panic!("unexpected result");
        };
        let output = output.1.output().unwrap();
        let ret = func.abi_decode_output(&output, true).unwrap();
        assert_eq!(ret[0].as_uint(), Some((U256::from(1), 256)));
    }

    #[test]
    fn test_call_contract_with_inserted_calls() {
        let mut state = MemoryBcState::fresh();
        let contract = r#"
        contract D {
            function drop() public {
                selfdestruct(payable(msg.sender));
            }
        }

        contract C {
            D d;

            constructor() {
                d = new D();
            }

            function foo() public returns (uint) {
                return 1;
            }
        }
        "#;
        let mut addrs = deploy_contracts(
            &mut state,
            "0.8.12",
            contract,
            vec!["C", "D"],
            Default::default(),
        )
        .unwrap();
        let c_address = addrs.remove(0);
        let d_address = addrs.remove(0);
        let func = Function::parse("foo() returns (uint)").unwrap();
        let input = func.abi_encode_input(&[]).unwrap();
        let spec = TransitionSpecBuilder::default()
            .bypass_check()
            .append_tx_env(TxEnv {
                transact_to: TransactTo::Call(c_address),
                data: input.cvt(),
                ..Default::default()
            })
            .build();
        let mut evm =
            InterruptableEvm::new(SpecId::LATEST, state, spec, *no_inspector());
        let r = evm.run(break_everywhere()).unwrap();
        assert!(matches!(r, RunResult::Breakpoint(_)));
        let r = evm.run(break_everywhere()).unwrap();
        assert!(matches!(r, RunResult::Breakpoint(_)));

        // insert call
        let drop_func = Function::parse("drop()").unwrap();
        let drop_input = drop_func.abi_encode_input(&[]).unwrap();
        let inputs = CallInputs {
            contract: d_address,
            transfer: Transfer {
                source: c_address,
                target: d_address,
                value: U256::from(0),
            },
            input: drop_input.cvt(),
            gas_limit: 1000000,
            context: CallContext {
                address: d_address,
                caller: c_address,
                code_address: d_address,
                apparent_value: U256::ZERO,
                scheme: CallScheme::Call,
            },
            is_static: false,
        };
        let r = evm.msg_call(inputs, break_nowhere()).unwrap();
        let BreakpointResult::NotHit(output) = r else {
            panic!("unexpected result");
        };
        assert_eq!(
            output.instruction_result(),
            InstructionResult::SelfDestruct
        );

        let r = evm.run(break_everywhere()).unwrap();
        assert!(matches!(r, RunResult::Done(_)));
        let RunResult::Done(output) = r else {
            panic!("unexpected result");
        };
        let output = output.1.output().unwrap();
        let ret = func.abi_decode_output(&output, true).unwrap();
        assert_eq!(ret[0].as_uint(), Some((U256::from(1), 256)));
    }
}
