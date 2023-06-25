use std::collections::BTreeMap;

use reth_primitives::Address;
use revm::{
    interpreter::{opcode, InstructionResult, Interpreter},
    Database, EVMData, Inspector,
};
use revm_primitives::U256;

/// Returns [InstructionResult::Continue] on an error, discarding the error.
///
/// Useful for inspectors that read state that might be invalid, but do not want to emit
/// appropriate errors themselves, instead opting to continue.
macro_rules! try_or_continue {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(_) => return InstructionResult::Continue,
        }
    };
}

#[derive(Clone, Debug, Default)]
pub struct RecordAccess {
    pub reads: BTreeMap<Address, Vec<U256>>,
    pub writes: BTreeMap<Address, Vec<U256>>,
}

#[derive(Clone, Debug, Default)]
pub struct CheatcodeInspector {
    pub accesses: Option<RecordAccess>,
}

impl CheatcodeInspector {
    pub fn reset_access_recording(&mut self) {
        self.accesses = Some(RecordAccess::default());
    }

    pub fn disable_access_recording(&mut self) {
        self.accesses = None;
    }
}

impl<DB> Inspector<DB> for CheatcodeInspector
where
    DB: Database<Error = reth_interfaces::Error>,
{
    #[doc = " Called on each step of the interpreter."]
    #[doc = ""]
    #[doc = " Information about the current execution, including the memory, stack and more is available"]
    #[doc = " on `interp` (see [Interpreter])."]
    #[doc = ""]
    #[doc = " # Example"]
    #[doc = ""]
    #[doc = " To get the current opcode, use `interp.current_opcode()`."]
    fn step(
        &mut self,
        interpreter: &mut Interpreter,
        data: &mut EVMData<'_, DB>,
        _: bool,
    ) -> InstructionResult {
        // Record writes and reads if `record` has been called
        if let Some(storage_accesses) = &mut self.accesses {
            match interpreter.contract.bytecode.bytecode()
                [interpreter.program_counter()]
            {
                opcode::SLOAD => {
                    let key = try_or_continue!(interpreter.stack().peek(0));
                    storage_accesses
                        .reads
                        .entry(interpreter.contract().address)
                        .or_insert_with(Vec::new)
                        .push(key);
                }
                opcode::SSTORE => {
                    let key = try_or_continue!(interpreter.stack().peek(0));

                    // An SSTORE does an SLOAD internally
                    storage_accesses
                        .reads
                        .entry(interpreter.contract().address)
                        .or_insert_with(Vec::new)
                        .push(key);
                    storage_accesses
                        .writes
                        .entry(interpreter.contract().address)
                        .or_insert_with(Vec::new)
                        .push(key);
                }
                _ => (),
            }
        }

        InstructionResult::Continue
    }
}
