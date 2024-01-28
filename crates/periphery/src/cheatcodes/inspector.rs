use std::collections::BTreeMap;

use libsofl_core::engine::{
    inspector::EvmInspector,
    state::BcState,
    types::{opcode, Address, Inspector, U256},
};

/// Returns [InstructionResult::Continue] on an error, discarding the error.
///
/// Useful for inspectors that read state that might be invalid, but do not want to emit
/// appropriate errors themselves, instead opting to continue.
macro_rules! try_or_continue {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(_) => return,
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
    DB: BcState,
{
    fn step(
        &mut self,
        interpreter: &mut libsofl_core::engine::types::Interpreter,
        _context: &mut libsofl_core::engine::types::EvmContext<DB>,
    ) {
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
    }
}

impl<BS> EvmInspector<BS> for CheatcodeInspector where BS: BcState {}
