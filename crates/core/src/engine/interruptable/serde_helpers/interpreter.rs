use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::engine::types::{
    Address, BytecodeLocked, Bytes, CallInputs, Contract, CreateInputs, Gas,
    InstructionResult, Interpreter, InterpreterAction, InterpreterResult,
    JumpMap, SharedMemory, Stack, B256, U256,
};

/// Represents the state of gas during execution.
/// Serde counterpart of Gas struct in revm-interpreter crate
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
#[serde(remote = "Gas")]
pub struct GasSerde {
    /// The initial gas limit.
    #[serde(getter = "Gas::limit")]
    limit: u64,
    /// The total used gas.
    #[serde(getter = "Gas::spend")]
    all_used_gas: u64,
    /// Used gas without memory expansion.
    #[serde(getter = "get_gas_used")]
    used: u64,
    /// Used gas for memory expansion.
    #[serde(getter = "Gas::memory")]
    memory: u64,
    /// Refunded gas. This is used only at the end of execution.
    #[serde(getter = "Gas::refunded")]
    refunded: i64,
}

impl From<GasSerde> for Gas {
    fn from(gas: GasSerde) -> Self {
        let mut g = Gas::new(gas.limit);
        g.record_cost(gas.used);
        g.record_memory(gas.memory);
        g.set_refund(gas.refunded);
        g
    }
}

/// Calculate the `used` field of Gas struct.
/// The formula should be `used = all_used_gas - memory`.
fn get_gas_used(gas: &Gas) -> u64 {
    gas.spend() - gas.memory()
}

/// The result of an interpreter operation.
/// Serde counterpart of InterpreterResult struct in revm-interpreter crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "InterpreterResult")]
pub struct InterpreterResultSerde {
    /// The result of the instruction execution.
    pub result: InstructionResult,
    /// The output of the instruction execution.
    pub output: Bytes,
    /// The gas usage information.
    #[serde(with = "GasSerde")]
    pub gas: Gas,
}

/// Serde counterpart of InterpreterAction enum in revm-interpreter crate.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(remote = "InterpreterAction")]
pub enum InterpreterActionSerde {
    /// CALL, CALLCODE, DELEGATECALL or STATICCALL instruction called.
    Call {
        /// Call inputs
        inputs: Box<CallInputs>,
        /// The offset into `self.memory` of the return data.
        ///
        /// This value must be ignored if `self.return_len` is 0.
        return_memory_offset: Range<usize>,
    },
    /// CREATE or CREATE2 instruction called.
    Create { inputs: Box<CreateInputs> },
    /// Interpreter finished execution.
    Return {
        #[serde(with = "InterpreterResultSerde")]
        result: InterpreterResult,
    },
    /// No action
    #[default]
    None,
}

/// An analyzed bytecode.
#[derive(Clone, Serialize, Deserialize)]
#[serde(remote = "BytecodeLocked")]
pub struct BytecodeLockedSerde {
    #[serde(getter = "BytecodeLocked::bytecode")]
    bytecode: Bytes,

    #[serde(getter = "BytecodeLocked::len")]
    original_len: usize,

    #[serde(getter = "BytecodeLocked::jump_map")]
    jump_map: JumpMap,
}

impl From<BytecodeLockedSerde> for BytecodeLocked {
    fn from(bytecode: BytecodeLockedSerde) -> Self {
        let bytecode = revm::primitives::Bytecode {
            bytecode: bytecode.bytecode,
            state: revm::primitives::BytecodeState::Analysed {
                len: bytecode.original_len,
                jump_map: bytecode.jump_map,
            },
        };
        Self::try_from(bytecode).expect("BytecodeLockedSerde should be valid")
    }
}

/// EVM contract information.
/// Serde counterpart of Contract struct in revm-interpreter crate.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(remote = "Contract")]
pub struct ContractSerde {
    /// Contracts data
    pub input: Bytes,
    /// Bytecode contains contract code, size of original code, analysis with gas block and jump table.
    /// Note that current code is extended with push padding and STOP at end.
    #[serde(with = "BytecodeLockedSerde")]
    pub bytecode: BytecodeLocked,
    /// Bytecode hash.
    pub hash: B256,
    /// Contract address
    pub address: Address,
    /// Caller of the EVM.
    pub caller: Address,
    /// Value send to contract.
    pub value: U256,
}

/// Serde counterpart of Interpreter struct in revm-interpreter crate.
#[derive(Debug, Serialize, Deserialize)]
#[serde(remote = "Interpreter")]
pub struct InterpreterSerde {
    /// Contract information and invoking data
    #[serde(with = "ContractSerde")]
    pub contract: Contract,
    /// The current instruction pointer.
    #[serde(getter = "Interpreter::program_counter")]
    pub instruction_pointer: usize,
    /// The execution control flag. If this is not set to `Continue`, the interpreter will stop
    /// execution.
    pub instruction_result: InstructionResult,
    /// The gas state.
    #[serde(with = "GasSerde")]
    pub gas: Gas,
    /// Shared memory.
    ///
    /// Note: This field is only set while running the interpreter loop.
    /// Otherwise it is taken and replaced with empty shared memory.
    pub shared_memory: SharedMemory,
    /// Stack.
    pub stack: Stack,
    /// The return data buffer for internal calls.
    /// It has multi usage:
    ///
    /// * It contains the output bytes of call sub call.
    /// * When this interpreter finishes execution it contains the output bytes of this contract.
    pub return_data_buffer: Bytes,
    /// Whether the interpreter is in "staticcall" mode, meaning no state changes can happen.
    pub is_static: bool,
    /// Actions that the EVM should do.
    ///
    /// Set inside CALL or CREATE instructions and RETURN or REVERT instructions. Additionally those instructions will set
    /// InstructionResult to CallOrCreate/Return/Revert so we know the reason.
    #[serde(with = "InterpreterActionSerde")]
    pub next_action: InterpreterAction,
}

impl From<InterpreterSerde> for Interpreter {
    fn from(value: InterpreterSerde) -> Self {
        let instruction_pointer = unsafe {
            value
                .contract
                .bytecode
                .as_ptr()
                .offset(value.instruction_pointer as isize)
        };
        Self {
            contract: Box::new(value.contract),
            instruction_pointer,
            instruction_result: value.instruction_result,
            gas: value.gas.into(),
            shared_memory: value.shared_memory,
            stack: value.stack,
            return_data_buffer: value.return_data_buffer,
            is_static: value.is_static,
            next_action: value.next_action.into(),
        }
    }
}
