use std::sync::Arc;

use auto_impl::auto_impl;
use revm::{Frame, FrameResult, GetInspector};

use crate::engine::{
    state::BcState,
    types::{Address, CallInputs, ExecutionResult, StateChange},
};

use super::ResumableContext;

#[derive(Debug, Clone)]
#[deprecated]
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

#[auto_impl(&, Box, Arc)]
pub trait IBreakpoint<M> {
    fn should_break_before_msg_call<S: BcState, I>(
        &self,
        context: &ResumableContext<S, I>,
        inputs: &CallInputs,
    ) -> Option<M>;

    fn should_break_begin_msg_call<S: BcState, I>(
        &self,
        context: &ResumableContext<S, I>,
        frame: &Frame,
    ) -> Option<M>;

    fn should_break_end_msg_call<S: BcState, I>(
        &self,
        context: &ResumableContext<S, I>,
        address: Address,
        result: &Frame,
    ) -> Option<M>;

    fn should_break_after_msg_call<S: BcState, I>(
        &self,
        context: &ResumableContext<S, I>,
        address: Address,
        result: &FrameResult,
    ) -> Option<M>;
}

pub enum RunResult<M> {
    Breakpoint(M),
    Done((StateChange, ExecutionResult)),
}

pub enum BreakpointResult<M> {
    Hit(M),
    NotHit(FrameResult),
}

pub fn break_everywhere() -> Arc<AllBreakpoints> {
    Arc::new(AllBreakpoints {})
}

pub fn break_nowhere() -> Arc<NoBreakpoints> {
    Arc::new(NoBreakpoints {})
}

pub struct NoBreakpoints {}

impl IBreakpoint<()> for NoBreakpoints {
    fn should_break_before_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _inputs: &CallInputs,
    ) -> Option<()> {
        None
    }

    fn should_break_begin_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _frame: &Frame,
    ) -> Option<()> {
        None
    }

    fn should_break_end_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _address: Address,
        _result: &Frame,
    ) -> Option<()> {
        None
    }

    fn should_break_after_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _address: Address,
        _result: &FrameResult,
    ) -> Option<()> {
        None
    }
}

pub struct AllBreakpoints {}

impl IBreakpoint<()> for AllBreakpoints {
    fn should_break_before_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _inputs: &CallInputs,
    ) -> Option<()> {
        Some(())
    }

    fn should_break_begin_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _frame: &Frame,
    ) -> Option<()> {
        Some(())
    }

    fn should_break_end_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _address: Address,
        _result: &Frame,
    ) -> Option<()> {
        Some(())
    }

    fn should_break_after_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _address: Address,
        _result: &FrameResult,
    ) -> Option<()> {
        Some(())
    }
}

#[allow(deprecated)]
impl Breakpoint {
    pub fn check_msg_call_before<'a, S: BcState, I: GetInspector<S>>(
        breakpoints: &Vec<Breakpoint>,
        _context: &ResumableContext<S, I>,
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

    pub fn check_msg_call_begin<'a, S: BcState, I: GetInspector<S>>(
        breakpoints: &Vec<Breakpoint>,
        _context: &ResumableContext<S, I>,
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

    pub fn check_msg_call_end<'a, S: BcState, I: GetInspector<S>>(
        breakpoints: &Vec<Breakpoint>,
        _context: &ResumableContext<S, I>,
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

    pub fn check_msg_call_after<'a, S: BcState, I: GetInspector<S>>(
        breakpoints: &Vec<Breakpoint>,
        _context: &ResumableContext<S, I>,
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
