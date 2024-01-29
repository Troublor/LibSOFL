use revm_primitives::ResultAndState;

use crate::engine::types::{Address, InterpreterResult};

#[derive(Debug, Clone)]
pub enum Breakpoint {
    /// Breakpoint before a message call to a contract
    MsgCallBefore(Address),

    /// Breakpoint at the begnning of a message call to a contract
    MsgCallBegin(Address),

    /// Breakpoint at the end of a message call to a contract
    MsgCallEnd(Address),

    /// Breakpoint after a message call to a contract
    MsgCallAfter(Address),
}

#[derive(Debug, Clone)]
pub enum RunResult {
    Breakpoint(Breakpoint),
    Done(ResultAndState),
}

#[derive(Debug, Clone)]
pub enum BreakpointResult {
    Hit(Breakpoint),
    NotNit(InterpreterResult),
}
