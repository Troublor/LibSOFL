use revm::{Frame, FrameResult};

use crate::engine::{
    state::BcState,
    types::{Address, CallInputs, ExecutionResult, StateChange},
};

use super::ResumableContext;

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
    Done((StateChange, ExecutionResult)),
}

pub enum BreakpointResult {
    Hit(Breakpoint),
    NotHit(FrameResult),
}

impl Breakpoint {
    pub fn check_msg_call_before<'a, S: BcState>(
        breakpoints: &Vec<Breakpoint>,
        _context: &ResumableContext<'a, S>,
        inputs: &CallInputs,
    ) -> Option<Breakpoint> {
        breakpoints
            .iter()
            .filter(|b| {
                if let Breakpoint::MsgCallBefore(addr) = b {
                    *addr == inputs.contract
                } else {
                    false
                }
            })
            .map(|b| b.clone())
            .next()
    }

    pub fn check_msg_call_begin<'a, S: BcState>(
        breakpoints: &Vec<Breakpoint>,
        _context: &ResumableContext<'a, S>,
        frame: &Frame,
    ) -> Option<Breakpoint> {
        breakpoints
            .iter()
            .filter(|b| {
                if let Breakpoint::MsgCallBegin(addr) = b {
                    *addr == frame.frame_data().interpreter.contract().address
                } else {
                    false
                }
            })
            .map(|b| b.clone())
            .next()
    }

    pub fn check_msg_call_end<'a, S: BcState>(
        breakpoints: &Vec<Breakpoint>,
        _context: &ResumableContext<'a, S>,
        address: Address,
        _result: &Frame,
    ) -> Option<Breakpoint> {
        breakpoints
            .iter()
            .filter(|b| {
                if let Breakpoint::MsgCallEnd(addr) = b {
                    *addr == address
                } else {
                    false
                }
            })
            .map(|b| b.clone())
            .next()
    }

    pub fn check_msg_call_after<'a, S: BcState>(
        breakpoints: &Vec<Breakpoint>,
        _context: &ResumableContext<'a, S>,
        address: Address,
        _result: &FrameResult,
    ) -> Option<Breakpoint> {
        breakpoints
            .iter()
            .filter(|b| {
                if let Breakpoint::MsgCallAfter(addr) = b {
                    *addr == address
                } else {
                    false
                }
            })
            .map(|b| b.clone())
            .next()
    }
}
