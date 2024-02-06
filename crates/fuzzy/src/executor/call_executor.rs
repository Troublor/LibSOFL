// use std::collections::HashMap;

// use crate::input::{
//     self,
//     calldata::StructuredCalldata,
//     tx::{HijackTarget, HijackedMsgCallSpec, MsgCall, MsgCallInput},
// };
// use libsofl_core::{
//     engine::{
//         inspector::{no_inspector, CombinedInspector, EvmInspector},
//         memory::MemoryBcState,
//         state::BcState,
//         transition::{TransitionSpec, TransitionSpecBuilder},
//         types::{
//             Address, BcStateRef, BlockEnv, Bytes, CallInputs, CallScheme,
//             CfgEnv, EVMData, ExecutionResult, Gas, Inspector,
//             InstructionResult, Output, TxEnv, U256,
//         },
//     },
//     error::SoflError,
// };
// use revm::{primitives::Env, Database, JournaledState};

// pub fn fuzzy_evm_cfg() -> CfgEnv {
//     let mut cfg: CfgEnv = Default::default();
//     cfg.disable_block_gas_limit = true;
//     cfg.disable_eip3607 = true;
//     cfg
// }

// fn execute_call<'a, S: BcStateRef>(
//     state: &'a mut MemoryBcState<S>,
//     cfg: &CfgEnv,
//     block_env: &BlockEnv,
//     input: &MsgCallInput,
//     insp: &mut dyn EvmInspector<&'a mut MemoryBcState<S>>,
// ) -> Result<ExecutionResult, SoflError>
// where
//     S::Error: std::fmt::Debug,
// {
//     let spec = TransitionSpec {
//         cfg: cfg.clone(),
//         block: block_env.clone(),
//         txs: vec![input.direct_call.to_tx_env()],
//     };
//     let mut hijack_executor =
//         HijackedMsgCallExecutor::new(None, &input.hijacked_calls);
//     let mut insps = CombinedInspector::default();
//     insps.add(insp);
//     insps.add(&mut hijack_executor);
//     let mut r = state.transit(spec, &mut insps)?;
//     Ok(r.remove(0))
// }

// /// ControlLeakExecutor utilize EVM inspector to dynamically execute arbitrary actions in a callback msg call.
// struct HijackedMsgCallExecutor<'a> {
//     journaled_state: Option<JournaledState>,

//     hijacked_calls: &'a HashMap<HijackTarget, Vec<HijackedMsgCallSpec>>,
//     hijack_index: HashMap<HijackTarget, usize>,
// }

// impl<'a> HijackedMsgCallExecutor<'a> {
//     pub fn new(
//         parent_journaled_state: Option<JournaledState>,
//         hijacked_calls: &'a HashMap<HijackTarget, Vec<HijackedMsgCallSpec>>,
//     ) -> Self {
//         Self {
//             journaled_state: parent_journaled_state,
//             hijacked_calls,
//             hijack_index: HashMap::new(),
//         }
//     }
// }

// impl<'a, D: BcStateRef> Inspector<&mut MemoryBcState<D>>
//     for HijackedMsgCallExecutor<'a>
// where
//     D::Error: std::fmt::Debug,
// {
//     fn initialize_interp(
//         &mut self,
//         _interp: &mut revm::interpreter::Interpreter<'_>,
//         data: &mut revm::EVMData<'_, &mut MemoryBcState<D>>,
//     ) {
//         // If there is a parent journaled state, meaning that this is a hijacked call,
//         // then we need to substitute the journaled state with the parent's.
//         if let Some(parent_journaled_state) = self.journaled_state.take() {
//             data.journaled_state = parent_journaled_state.clone();
//         }
//     }

//     fn step(
//         &mut self,
//         interp: &mut revm::interpreter::Interpreter<'_>,
//         data: &mut revm::EVMData<'_, &mut MemoryBcState<D>>,
//     ) {
//     }

//     fn call(
//         &mut self,
//         data: &mut revm::EVMData<'_, &mut MemoryBcState<D>>,
//         inputs: &mut revm::interpreter::CallInputs,
//     ) -> (
//         revm::interpreter::InstructionResult,
//         revm::interpreter::Gas,
//         revm::primitives::Bytes,
//     ) {
//         println!(
//             "call @{:?} {:?} => {:?}",
//             data.journaled_state.depth,
//             inputs.context.caller,
//             inputs.context.address
//         );
//         (InstructionResult::Continue, Gas::new(0), Bytes::new())
//     }

//     fn create_end(
//         &mut self,
//         data: &mut revm::EVMData<'_, &mut MemoryBcState<D>>,
//         _inputs: &revm::interpreter::CreateInputs,
//         ret: revm::interpreter::InstructionResult,
//         address: Option<revm::primitives::Address>,
//         remaining_gas: revm::interpreter::Gas,
//         out: revm::primitives::Bytes,
//     ) -> (
//         revm::interpreter::InstructionResult,
//         Option<revm::primitives::Address>,
//         revm::interpreter::Gas,
//         revm::primitives::Bytes,
//     ) {
//         // grab the journaled state before it is cleanup and finalized
//         // the grabbed journaled state will be forwarded to the parent, akin to
//         // we are using the parent's journaled state at the beginning of this call
//         if data.journaled_state.depth == 0 {
//             self.journaled_state.replace(data.journaled_state.clone());
//         }
//         (ret, address, remaining_gas, out)
//     }
//     fn call_end(
//         &mut self,
//         data: &mut revm::EVMData<'_, &mut MemoryBcState<D>>,
//         inputs: &revm::interpreter::CallInputs,
//         remaining_gas: revm::interpreter::Gas,
//         ret: revm::interpreter::InstructionResult,
//         out: revm::primitives::Bytes,
//     ) -> (
//         revm::interpreter::InstructionResult,
//         revm::interpreter::Gas,
//         revm::primitives::Bytes,
//     ) {
//         // grab the journaled state before it is cleanup and finalized
//         // the grabbed journaled state will be forwarded to the parent, akin to
//         // we are using the parent's journaled state at the beginning of this call
//         if data.journaled_state.depth == 0 {
//             self.journaled_state.replace(data.journaled_state.clone());
//         }

//         println!(
//             "call end @{:?} {:?} => {:?}",
//             data.journaled_state.depth,
//             inputs.context.caller,
//             inputs.context.address,
//         );

//         let k = HijackTarget {
//             code_address: inputs.context.code_address,
//             call_scheme: inputs.context.scheme,
//         };
//         let call_index = self.hijack_index.entry(k).or_insert(0);
//         let call_index = *call_index;
//         let call_specs = self.hijacked_calls.get(&k);
//         self.hijack_index.insert(k, call_index + 1);
//         if let Some(call_specs) = call_specs {
//             if call_index < call_specs.len() {
//                 let call_spec = &call_specs[call_index];
//                 let calls = &call_spec.calls;
//                 let (_, gas, _, journaled_state) = execute_hijacked_calls(
//                     data.db,
//                     &data.env,
//                     data.journaled_state.clone(),
//                     inputs.context.address,
//                     calls,
//                 );
//                 // the hijacked call takes the journaled state,
//                 // now we need to put it back
//                 data.journaled_state = journaled_state;

//                 if gas.spend() > inputs.gas_limit {
//                     return (
//                         InstructionResult::OutOfGas,
//                         Gas::new(inputs.gas_limit),
//                         Bytes::new(),
//                     );
//                 } else if call_spec.success {
//                     return (
//                         InstructionResult::Return,
//                         Gas::new(call_spec.gas_used),
//                         call_spec.return_data.clone(),
//                     );
//                 } else {
//                     return (
//                         InstructionResult::Revert,
//                         Gas::new(call_spec.gas_used),
//                         call_spec.return_data.clone(),
//                     );
//                 }
//             }
//         }
//         (ret, remaining_gas, out)
//     }
// }

// fn execute_hijacked_calls<D: BcStateRef>(
//     state: &mut MemoryBcState<D>,
//     env: &Env,
//     journaled_state: JournaledState,
//     state_address: Address,
//     calls: &Vec<MsgCallInput>,
// ) -> (InstructionResult, Gas, Bytes, JournaledState)
// where
//     D::Error: std::fmt::Debug,
// {
//     let mut gas = 0u64;
//     let mut ret = Bytes::new();
//     let mut js: JournaledState = journaled_state;
//     for call in calls {
//         let (_, gas_used, return_data, journaled_state) =
//             execute_hijacked_call(state, env, js, state_address, call);
//         js = journaled_state;
//         gas += gas_used.spend();
//         ret = return_data;
//     }
//     (InstructionResult::Return, Gas::new(gas), ret, js)
// }

// fn execute_hijacked_call<D: BcStateRef>(
//     state: &mut MemoryBcState<D>,
//     env: &Env,
//     journaled_state: JournaledState,
//     state_address: Address,
//     call: &MsgCallInput,
// ) -> (InstructionResult, Gas, Bytes, JournaledState)
// where
//     D::Error: std::fmt::Debug,
// {
//     let tx_env = TxEnv {
//         caller: state_address,
//         gas_limit: call.direct_call.gas_limit,
//         transact_to: call.direct_call.transact_to.clone(),
//         value: call.direct_call.value,
//         data: call.direct_call.calldata.bytes(),
//         nonce: None,
//         ..env.tx.clone()
//     };
//     let spec = TransitionSpecBuilder::default()
//         .set_cfg(env.cfg.clone())
//         .set_block(env.block.clone())
//         .append_tx_env(tx_env)
//         .build();
//     let mut hijack_executor = HijackedMsgCallExecutor::new(
//         Some(journaled_state),
//         &call.hijacked_calls,
//     );
//     let mut r = state
//         .transit(spec, &mut hijack_executor)
//         .expect("bug: unrecoverable SoflError");
//     let journaled_state = hijack_executor
//         .journaled_state
//         .take()
//         .expect("bug: journaled state should be returned after hijacked call");
//     let r = r.remove(0);
//     match r {
//         ExecutionResult::Success {
//             output, gas_used, ..
//         } => (
//             InstructionResult::Return,
//             Gas::new(gas_used),
//             match output {
//                 Output::Call(ret) => ret,
//                 Output::Create(..) => Bytes::new(),
//             },
//             journaled_state,
//         ),
//         ExecutionResult::Revert { gas_used, output } => (
//             InstructionResult::Revert,
//             Gas::new(gas_used),
//             output,
//             journaled_state,
//         ),
//         ExecutionResult::Halt { gas_used, .. } => (
//             InstructionResult::Revert,
//             Gas::new(gas_used),
//             Bytes::new(),
//             journaled_state,
//         ),
//     }
// }

// impl<'a, D: BcStateRef> EvmInspector<&mut MemoryBcState<D>>
//     for HijackedMsgCallExecutor<'a>
// where
//     D::Error: std::fmt::Debug,
// {
// }

// #[cfg(test)]
// mod tests {
//     use std::collections::HashMap;

//     use alloy_json_abi::Function;
//     use libsofl_core::engine::{
//         inspector::no_inspector,
//         memory::MemoryBcState,
//         types::{Address, Bytes, CallScheme, TransactTo, U256},
//     };
//     use libsofl_utils::solidity::scripting::{
//         deploy_contracts, SolScriptConfig,
//     };
//     use revm::{primitives::BlockEnv, Database};

//     use crate::{
//         executor::call_executor::execute_call,
//         input::{
//             calldata::StructuredCalldata,
//             tx::{HijackTarget, HijackedMsgCallSpec, MsgCall, MsgCallInput},
//         },
//     };

//     use super::fuzzy_evm_cfg;

//     #[test]
//     fn test_reentrancy() {
//         let code = r#"
//         contract Victim {
//             bool public withdrawed = false;
//             constructor () payable {}
//             function withdraw() public {
//                 if (!withdrawed) {
//                     msg.sender.call{value: 1}("");
//                     withdrawed = true;
//                 }
//             }
//         }
//         "#;
//         let mut state = MemoryBcState::fresh();
//         let victim_contract = deploy_contracts(
//             &mut state,
//             "0.8.12",
//             code,
//             vec!["Victim"],
//             SolScriptConfig {
//                 prefund: U256::from(10),
//                 ..Default::default()
//             },
//         )
//         .unwrap()
//         .remove(0);
//         let cfg = fuzzy_evm_cfg();
//         let block = BlockEnv::default();

//         // make a reentrancy call to victim contract
//         let attacker = Address::random();
//         let direct_call = MsgCall {
//             caller: attacker,
//             transact_to: TransactTo::Call(victim_contract),
//             calldata: StructuredCalldata::Typed(
//                 Function::parse("withdraw()").unwrap(),
//                 vec![],
//             ),
//             ..Default::default()
//         };
//         let mut hijacked_calls = HashMap::new();
//         hijacked_calls.insert(
//             HijackTarget {
//                 code_address: attacker,
//                 call_scheme: CallScheme::Call,
//             },
//             vec![HijackedMsgCallSpec {
//                 calls: vec![direct_call.clone().into()],
//                 success: true,
//                 gas_used: 0,
//                 return_data: Bytes::new(),
//             }],
//         );
//         let call = MsgCallInput {
//             direct_call,
//             hijacked_calls,
//         };

//         // execute the call
//         println!("attacker: {:?}", attacker);
//         println!("victim: {:?}", victim_contract);
//         let r = execute_call(&mut state, &cfg, &block, &call, no_inspector())
//             .unwrap();
//         assert!(r.is_success());
//         let victim_balance =
//             state.basic(victim_contract).unwrap().unwrap().balance;
//         println!("victim balance: {:?}", victim_balance);
//         let attacker_balance = state.basic(attacker).unwrap().unwrap().balance;
//         println!("attacker balance: {:?}", attacker_balance);
//         assert!(attacker_balance > U256::from(1));
//     }
// }
