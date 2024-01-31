use std::ops::Range;

use revm::{Evm, Frame, FrameResult};
use revm_primitives::ResultAndState;

use crate::{
    engine::{
        inspector::{EvmInspector, InspectorContext},
        interruptable::breakpoint::RunResult,
        revm::revm::{
            interpreter::{
                opcode::InstructionTables, Interpreter, InterpreterAction,
                InterpreterResult,
            },
            primitives::{EVMError, TransactTo},
            FrameOrResult,
        },
        state::BcState,
        types::{Address, CallInputs, CreateInputs},
    },
    error::SoflError,
};

use super::{
    breakpoint::{Breakpoint, BreakpointResult},
    InterruptableEvm, ResumableContext,
};

impl<S: BcState, I: EvmInspector<S>> InterruptableEvm<S, I> {
    #[inline]
    pub fn run(
        &self,
        context: &mut ResumableContext<S>,
        breakpoints: Vec<Breakpoint>,
    ) -> Result<RunResult, SoflError> {
        self.run_until_breakpoint(context, breakpoints)
            .map_err(|e| match e {
                revm::primitives::EVMError::Transaction(ee) => {
                    SoflError::InvalidTransaction(ee)
                }
                revm::primitives::EVMError::Header(ee) => {
                    SoflError::InvalidHeader(ee)
                }
                revm::primitives::EVMError::Database(ee) => {
                    SoflError::BcState(format!("{:?}", ee))
                }
                revm_primitives::EVMError::Custom(ee) => {
                    SoflError::BcState(format!("{:?}", ee))
                }
            })
    }

    /// Run evm until one of the breakpoints.
    /// Akin to `transact` function in the original revm.
    #[inline]
    pub fn run_until_breakpoint(
        &self,
        context: &mut ResumableContext<S>,
        breakpoints: Vec<Breakpoint>,
    ) -> Result<RunResult, EVMError<S::Error>> {
        let initial_gas_spend = if context.is_new_transaction() {
            // validate transaction, akin to `transact` function in the original revm.
            context
                .revm_ctx
                .handler
                .validation()
                .env(&context.revm_ctx.context.evm.env)?;
            let initial_gas_spend = context
                .revm_ctx
                .handler
                .validation()
                .initial_tx_gas(&context.revm_ctx.context.evm.env)?;
            context
                .revm_ctx
                .handler
                .validation()
                .tx_against_state(&mut context.revm_ctx.context)?;
            context.in_progress = true;
            Some(initial_gas_spend)
        } else {
            None
        };

        let output = self.run_until_breakpoint_inner(
            context,
            breakpoints,
            initial_gas_spend,
        )?;

        // handle output of call/create calls if the transaction finishes.
        let r = match output {
            RunResult::Done(output) => {
                let r = context.revm_ctx.handler.post_execution().end(
                    &mut context.revm_ctx.context,
                    Ok(ResultAndState {
                        state: output.0,
                        result: output.1,
                    }),
                )?;
                RunResult::Done((r.state, r.result))
            }
            RunResult::Breakpoint(_) => output,
        };

        Ok(r)
    }

    /// Akin to `transact_preverified_inner` function in the original revm.
    #[inline]
    fn run_until_breakpoint_inner(
        &self,
        context: &mut ResumableContext<S>,
        breakpoints: Vec<Breakpoint>,
        initial_gas_spend: Option<u64>, // None indicate that this is an resumed execution.
    ) -> Result<RunResult, EVMError<S::Error>> {
        // transaction preparation, if this is the new transaction execution
        let first_frame_or_result = if let Some(initial_gas_spend) =
            initial_gas_spend
        {
            let ctx = &mut context.revm_ctx.context;
            let pre_exec = context.revm_ctx.handler.pre_execution();

            // load access list and beneficiary if needed.
            pre_exec.load_accounts(ctx)?;

            // load precompiles
            let precompiles = pre_exec.load_precompiles();
            ctx.evm.set_precompiles(precompiles);

            // deduce caller balance with its limit.
            pre_exec.deduct_caller(ctx)?;

            let gas_limit = ctx.evm.env.tx.gas_limit - initial_gas_spend;

            let exec = context.revm_ctx.handler.execution();
            // call inner handling of call/create
            let first_frame_or_result = match ctx.evm.env.tx.transact_to {
                TransactTo::Call(_) => exec.call(
                    ctx,
                    CallInputs::new_boxed(&ctx.evm.env.tx, gas_limit).unwrap(),
                    0..0,
                ),
                TransactTo::Create(_) => exec.create(
                    ctx,
                    CreateInputs::new_boxed(&ctx.evm.env.tx, gas_limit)
                        .unwrap(),
                ),
            };
            Some(first_frame_or_result)
        } else {
            None
        };

        // Starts the main running loop.
        let result: BreakpointResult = match first_frame_or_result {
            Some(first_frame_or_result) => match first_frame_or_result {
                FrameOrResult::Frame(first_frame) => {
                    self.start_the_loop(context, breakpoints, Some(first_frame))
                }
                FrameOrResult::Result(result) => {
                    BreakpointResult::NotHit(result)
                }
            },
            None => {
                // resume the execution
                self.start_the_loop(context, breakpoints, None)
            }
        };

        // handle transaction execution result if the transaction finishes
        let r: RunResult = match result {
            BreakpointResult::Hit(bp) => RunResult::Breakpoint(bp),
            BreakpointResult::NotHit(mut result) => {
                let ctx = &mut context.revm_ctx.context;

                // handle output of call/create calls.
                context
                    .revm_ctx
                    .handler
                    .execution()
                    .last_frame_return(ctx, &mut result);

                let post_exec = context.revm_ctx.handler.post_execution();
                // Reimburse the caller
                post_exec.reimburse_caller(ctx, result.gas())?;
                // Reward beneficiary
                post_exec.reward_beneficiary(ctx, result.gas())?;
                // Returns output of transaction.
                let r = post_exec.output(ctx, result)?;
                RunResult::Done((r.state, r.result))
            }
        };

        Ok(r)
    }

    #[inline]
    fn start_the_loop(
        &self,
        context: &mut ResumableContext<S>,
        breakpoints: Vec<Breakpoint>,
        first_frame: Option<Frame>,
    ) -> BreakpointResult {
        // take instruction talbe
        let table = context
            .revm_ctx
            .handler
            .take_instruction_table()
            .expect("Instruction table should be present");

        // run main loop
        let loop_result = match &table {
            InstructionTables::Plain(table) => {
                self.run_the_loop(context, breakpoints, table, first_frame)
            }
            InstructionTables::Boxed(table) => {
                self.run_the_loop(context, breakpoints, table, first_frame)
            }
        };

        // return back instruction table
        context.revm_ctx.handler.set_instruction_table(table);

        loop_result
    }

    #[inline]
    pub fn run_the_loop<'a, FN>(
        &self,
        context: &mut ResumableContext<'a, S>,
        breakpoints: Vec<Breakpoint>,
        table: &[FN; 256],
        first_frame: Option<Frame>, // None indicate that this is an resumed execution.
    ) -> BreakpointResult
    where
        FN: Fn(&mut Interpreter, &mut Evm<'a, InspectorContext<'a, S>, S>),
    {
        let mut call_stack = context.take_call_stack();
        let mut shared_memory = context.take_shared_memory();

        // push the top-level frame to the call stack, if this is a new transaction
        if let Some(first_frame) = first_frame {
            call_stack.push(first_frame);
            shared_memory.new_context();
        }

        let mut loop_result: LoopResult =
            LoopResult::Continue(context.take_next_action());
        loop {
            // whether or not to interrupt the execution.
            let action = match loop_result {
                LoopResult::Pause { .. } => {
                    // break if hit a breakpoint
                    break;
                }
                LoopResult::Continue(action) => action,
                LoopResult::Finish(_) => {
                    // break if the transaction finishes
                    break;
                }
            };

            // perform action
            let loop_action = match action {
                Action::BeforeCall(inputs, return_memory_offset) => {
                    let exec = &context.revm_ctx.handler.execution;
                    // create a new call frame, and push to the call stack.
                    let frame_or_result = exec.call(
                        &mut context.revm_ctx.context,
                        inputs.clone(),
                        return_memory_offset,
                    );
                    let loop_result = match frame_or_result {
                        FrameOrResult::Frame(frame) => {
                            // step in the subcall
                            shared_memory.new_context();
                            call_stack.push(frame);
                            Action::FrameBegin
                        }
                        FrameOrResult::Result(result) => {
                            // the callee is not a contract, we directly have frame result.
                            let top_frame = call_stack.last_mut().unwrap();
                            let FrameResult::Call(outcome) = &result else {
                                unreachable!()
                            };
                            exec.insert_call_outcome(
                                &mut context.revm_ctx.context,
                                top_frame,
                                &mut shared_memory,
                                outcome.clone(),
                            );
                            Action::AfterCall(inputs.contract, result)
                        }
                    };
                    loop_result
                }
                Action::AfterCall(..) => {
                    // do nothing, continue the next action
                    Action::Continue
                }
                Action::FrameBegin => {
                    // do nothing, continue to next action
                    Action::Continue
                }
                Action::FrameEnd(_, result) => {
                    let exec = &context.revm_ctx.handler.execution;
                    // free memory context.
                    shared_memory.free_context();
                    // pop last frame from the stack and consume it to create FrameResult.
                    let returned_frame = call_stack
                        .pop()
                        .expect("We just returned from Interpreter frame");
                    // collect frame result
                    let ctx = &mut context.revm_ctx.context;
                    enum CalledOrCreated {
                        Call(Address),
                        Create(Address),
                    }
                    let address: CalledOrCreated = match &returned_frame {
                        Frame::Call(frame) => CalledOrCreated::Call(
                            frame.frame_data.interpreter.contract().address,
                        ),
                        Frame::Create(frame) => CalledOrCreated::Create(
                            frame.frame_data.interpreter.contract().address,
                        ),
                    };
                    let frame_result = match returned_frame {
                        Frame::Call(frame) => {
                            // return_call
                            FrameResult::Call(
                                exec.call_return(ctx, frame, result),
                            )
                        }
                        Frame::Create(frame) => {
                            // return_create
                            FrameResult::Create(
                                exec.create_return(ctx, frame, result),
                            )
                        }
                    };
                    // insert the frame result to parent frame.
                    if let Some(top_frame) = call_stack.last_mut() {
                        let ctx = &mut context.revm_ctx.context;
                        // Insert result to the top frame.
                        match &frame_result {
                            FrameResult::Call(outcome) => {
                                // return_call
                                exec.insert_call_outcome(
                                    ctx,
                                    top_frame,
                                    &mut shared_memory,
                                    outcome.clone(),
                                )
                            }
                            FrameResult::Create(outcome) => {
                                // return_create
                                exec.insert_create_outcome(
                                    ctx,
                                    top_frame,
                                    outcome.clone(),
                                )
                            }
                        }
                        // continue the parent frame
                        match address {
                            CalledOrCreated::Call(addr) => {
                                Action::AfterCall(addr, frame_result)
                            }
                            CalledOrCreated::Create(_) => {
                                Action::AfterCreate(frame_result)
                            }
                        }
                    } else {
                        // there are no more frames, transaction execution is done.
                        Action::Done(frame_result)
                    }
                }
                Action::BeforeCreate(inputs) => {
                    let exec = &context.revm_ctx.handler.execution;
                    // create a new create frame, and push to the call stack.
                    let frame_or_result =
                        exec.create(&mut context.revm_ctx.context, inputs);
                    match frame_or_result {
                        FrameOrResult::Frame(frame) => {
                            // step in the subcall
                            shared_memory.new_context();
                            call_stack.push(frame);
                            Action::FrameBegin
                        }
                        FrameOrResult::Result(result) => {
                            // the callee is not a contract, we directly have frame result.
                            let top_frame = call_stack.last_mut().unwrap();
                            let FrameResult::Create(outcome) = &result else {
                                unreachable!()
                            };
                            exec.insert_create_outcome(
                                &mut context.revm_ctx.context,
                                top_frame,
                                outcome.clone(),
                            );
                            Action::AfterCreate(result)
                        }
                    }
                }
                Action::AfterCreate(..) => {
                    // do nothing, continue the next action
                    Action::Continue
                }
                Action::Continue => {
                    // continue to run interpreter.
                    // peek last stack frame.
                    let stack_frame = call_stack.last_mut().unwrap();
                    // run interpreter
                    let interpreter =
                        &mut stack_frame.frame_data_mut().interpreter;
                    let action = interpreter.run(
                        shared_memory,
                        table,
                        &mut context.revm_ctx,
                    );
                    // take shared memory back.
                    shared_memory = interpreter.take_memory();
                    let loop_result = match action {
                        InterpreterAction::Call {
                            inputs,
                            return_memory_offset,
                        } => {
                            let action = Action::BeforeCall(
                                inputs,
                                return_memory_offset,
                            );
                            action
                        }
                        InterpreterAction::Create { inputs } => {
                            let action = Action::BeforeCreate(inputs);
                            action
                        }
                        InterpreterAction::Return { result } => {
                            let action = Action::FrameEnd(
                                interpreter.contract().address,
                                result,
                            );
                            action
                        }
                        InterpreterAction::None => {
                            unreachable!(
                                "InterpreterAction::None is not expected"
                            )
                        }
                    };
                    loop_result
                }
                Action::Done(_) => {
                    unreachable!("Action::Done is not expected")
                }
            };

            // whether a breakpoint is hit
            let maybe_breakpoint: Option<Breakpoint> = match &loop_action {
                Action::BeforeCall(inputs, _) => {
                    Breakpoint::check_msg_call_before(
                        &breakpoints,
                        context,
                        &*inputs,
                    )
                }
                Action::AfterCall(address, result) => {
                    Breakpoint::check_msg_call_after(
                        &breakpoints,
                        context,
                        *address,
                        result,
                    )
                }
                Action::FrameBegin => {
                    let frame = call_stack.last().unwrap();
                    Breakpoint::check_msg_call_begin(
                        &breakpoints,
                        context,
                        frame,
                    )
                }
                Action::FrameEnd(address, _) => {
                    let frame = call_stack.last().unwrap();
                    Breakpoint::check_msg_call_end(
                        &breakpoints,
                        context,
                        *address,
                        frame,
                    )
                }
                _ => None,
            };
            if let Some(breakpoint) = maybe_breakpoint {
                loop_result = LoopResult::Pause {
                    breakpoint,
                    next: loop_action,
                };
            } else {
                loop_result = match loop_action {
                    Action::Done(result) => LoopResult::Finish(result),
                    _ => LoopResult::Continue(loop_action),
                };
            }
        }

        // return the result of the loop
        let ret = match loop_result {
            LoopResult::Pause { breakpoint, next } => {
                context.next_action = next;
                BreakpointResult::Hit(breakpoint)
            }
            LoopResult::Continue(_) => {
                unreachable!("LoopResult::Continue is not expected")
            }
            LoopResult::Finish(result) => BreakpointResult::NotHit(result),
        };

        context.call_stack = call_stack;
        context.shared_memory = shared_memory;

        ret
    }

    /// Execute custom contract call at current context.
    /// When breakpoint is hit, the execution will be interrupted.
    #[inline]
    pub fn execute_message_call<FN>(
        &mut self,
        _context: &mut ResumableContext<S>,
        _instruction_table: &[FN; 256],
        _call: CallInputs,
        _breakpoints: Vec<Breakpoint>,
    ) where
        FN: Fn(&mut Interpreter, &mut ResumableContext<S>) -> InterpreterResult,
    {
        todo!()
    }
}

pub enum LoopResult {
    Pause {
        breakpoint: Breakpoint,
        next: Action,
    },
    Continue(Action),
    Finish(FrameResult),
}

#[derive(Default)]
pub enum Action {
    #[default]
    Continue,
    Done(FrameResult),
    BeforeCall(Box<CallInputs>, Range<usize>),
    AfterCall(Address, FrameResult),
    FrameBegin,
    FrameEnd(Address, InterpreterResult),
    BeforeCreate(Box<CreateInputs>),
    AfterCreate(FrameResult),
}
