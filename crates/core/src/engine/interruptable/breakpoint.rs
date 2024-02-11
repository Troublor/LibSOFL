use std::sync::Arc;

use auto_impl::auto_impl;
use revm::{Frame, FrameResult};

use crate::engine::{
    state::BcState,
    types::{Address, CallInputs, ExecutionResult, StateChange},
};

use super::ResumableContext;

#[auto_impl(&, Box, Arc)]
pub trait Breakpoint<M> {
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

impl Breakpoint<()> for NoBreakpoints {
    #[inline]
    fn should_break_before_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _inputs: &CallInputs,
    ) -> Option<()> {
        None
    }

    #[inline]
    fn should_break_begin_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _frame: &Frame,
    ) -> Option<()> {
        None
    }

    #[inline]
    fn should_break_end_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _address: Address,
        _result: &Frame,
    ) -> Option<()> {
        None
    }

    #[inline]
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

impl Breakpoint<()> for AllBreakpoints {
    #[inline]
    fn should_break_before_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _inputs: &CallInputs,
    ) -> Option<()> {
        Some(())
    }

    #[inline]
    fn should_break_begin_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _frame: &Frame,
    ) -> Option<()> {
        Some(())
    }

    #[inline]
    fn should_break_end_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _address: Address,
        _result: &Frame,
    ) -> Option<()> {
        Some(())
    }

    #[inline]
    fn should_break_after_msg_call<S: BcState, I>(
        &self,
        _context: &ResumableContext<S, I>,
        _address: Address,
        _result: &FrameResult,
    ) -> Option<()> {
        Some(())
    }
}
