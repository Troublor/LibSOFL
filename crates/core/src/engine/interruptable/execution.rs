use crate::engine::{
    state::BcState,
    types::{Evm, SharedMemory},
};

use super::CALL_STACK_LIMIT;
use std::ops::Range;

use revm::{Frame, FrameResult};
use revm_primitives::ResultAndState;

use crate::{
    engine::{
        interruptable::breakpoint::RunResult,
        interruptable::serde_helpers::interpreter::InterpreterResultSerde,
        interruptable::serde_helpers::revm::FrameResultSerde,
        revm::revm::{
            interpreter::{
                opcode::InstructionTables, Interpreter, InterpreterAction,
                InterpreterResult,
            },
            primitives::{EVMError, TransactTo},
            FrameOrResult,
        },
        types::{Address, CallInputs, CreateInputs},
    },
    error::SoflError,
};

use super::breakpoint::{Breakpoint, BreakpointResult};

pub enum LoopResult<M> {
    Pause { breakpoint: M, next: Action },
    Continue(Action),
    Finish(FrameResult),
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub enum Action {
    #[default]
    Continue,
    Done(#[serde(with = "FrameResultSerde")] FrameResult),
    BeforeCall(Box<CallInputs>, Range<usize>),
    AfterCall(Address, #[serde(with = "FrameResultSerde")] FrameResult),
    FrameBegin,
    FrameEnd(
        Address,
        #[serde(with = "InterpreterResultSerde")] InterpreterResult,
    ),
    BeforeCreate(Box<CreateInputs>),
    AfterCreate(#[serde(with = "FrameResultSerde")] FrameResult),
}

pub struct Executor<'a, S: BcState, I> {
    pub evm: Evm<'a, I, S>,
    pub call_stack_stages: Vec<Vec<Frame>>, // stages of call stacks
    pub shared_memory: SharedMemory,
    pub next_action_stages: Vec<Action>, // stages of next actions
}

impl<'a, DB: BcState, I> Executor<'a, DB, I> {
    fn begin_stage(&mut self) {
        self.call_stack_stages
            .push(Vec::with_capacity(CALL_STACK_LIMIT as usize + 1));
        self.next_action_stages.push(Action::Continue);
    }

    fn end_stage(&mut self) {
        self.call_stack_stages.pop().expect("call stack is empty");
        self.next_action_stages.pop().expect("next action is empty");
    }

    fn take_call_stack(&mut self) -> Vec<Frame> {
        let len = self.call_stack_stages.len();
        std::mem::take(&mut self.call_stack_stages[len - 1])
    }

    fn replace_call_stack(&mut self, call_stack: Vec<Frame>) {
        let len = self.call_stack_stages.len();
        let _ =
            std::mem::replace(&mut self.call_stack_stages[len - 1], call_stack);
    }

    fn take_shared_memory(&mut self) -> SharedMemory {
        std::mem::take(&mut self.shared_memory)
    }

    fn take_next_action(&mut self) -> Action {
        let len = self.next_action_stages.len();
        std::mem::take(&mut self.next_action_stages[len - 1])
    }

    fn replace_next_action(&mut self, next_action: Action) {
        let len = self.next_action_stages.len();
        let _ = std::mem::replace(
            &mut self.next_action_stages[len - 1],
            next_action,
        );
    }

    fn is_new_transaction(&self) -> bool {
        // !self.in_progress
        self.call_stack_stages.is_empty()
    }
}

impl<'a, S: BcState, I> Executor<'a, S, I> {
    #[inline]
    pub fn msg_call<M, B: Breakpoint<M>>(
        &mut self,
        inputs: CallInputs,
        breakpoints: B,
    ) -> Result<BreakpointResult<M>, SoflError> {
        let action = Action::BeforeCall(Box::new(inputs), Default::default());
        self.begin_stage();
        self.replace_next_action(action);
        let result = self.start_the_loop(breakpoints, None);
        self.end_stage();
        Ok(result)
    }

    #[inline]
    pub fn run<M, B: Breakpoint<M>>(
        &mut self,
        breakpoints: B,
    ) -> Result<RunResult<M>, SoflError> {
        self.run_until_breakpoint(breakpoints).map_err(|e| match e {
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
    fn run_until_breakpoint<M, B: Breakpoint<M>>(
        &mut self,
        breakpoints: B,
    ) -> Result<RunResult<M>, EVMError<S::Error>> {
        let initial_gas_spend = if self.is_new_transaction() {
            // begin an execution stage
            self.begin_stage();

            // validate transaction, akin to `transact` function in the original revm.
            self.evm
                .handler
                .validation()
                .env(&self.evm.context.evm.env)?;
            let initial_gas_spend = self
                .evm
                .handler
                .validation()
                .initial_tx_gas(&self.evm.context.evm.env)?;
            self.evm
                .handler
                .validation()
                .tx_against_state(&mut self.evm.context)?;
            Some(initial_gas_spend)
        } else {
            None
        };

        let output =
            self.run_until_breakpoint_inner(breakpoints, initial_gas_spend)?;

        // handle output of call/create calls if the transaction finishes.
        let r = match output {
            RunResult::Done(output) => {
                let r = self.evm.handler.post_execution().end(
                    &mut self.evm.context,
                    Ok(ResultAndState {
                        state: output.0,
                        result: output.1,
                    }),
                )?;
                self.end_stage();
                RunResult::Done((r.state, r.result))
            }
            RunResult::Breakpoint(_) => output,
        };

        Ok(r)
    }

    /// Akin to `transact_preverified_inner` function in the original revm.
    #[inline]
    fn run_until_breakpoint_inner<M, B: Breakpoint<M>>(
        &mut self,
        breakpoints: B,
        initial_gas_spend: Option<u64>, // None indicate that this is an resumed execution.
    ) -> Result<RunResult<M>, EVMError<S::Error>> {
        // transaction preparation, if this is the new transaction execution
        let first_frame_or_result = if let Some(initial_gas_spend) =
            initial_gas_spend
        {
            let ctx = &mut self.evm.context;
            let pre_exec = self.evm.handler.pre_execution();

            // load access list and beneficiary if needed.
            pre_exec.load_accounts(ctx)?;

            // load precompiles
            let precompiles = pre_exec.load_precompiles();
            ctx.evm.set_precompiles(precompiles);

            // deduce caller balance with its limit.
            pre_exec.deduct_caller(ctx)?;

            let gas_limit = ctx.evm.env.tx.gas_limit - initial_gas_spend;

            let exec = self.evm.handler.execution();
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
        let result = match first_frame_or_result {
            Some(first_frame_or_result) => match first_frame_or_result {
                FrameOrResult::Frame(first_frame) => {
                    self.start_the_loop(breakpoints, Some(first_frame))
                }
                FrameOrResult::Result(result) => {
                    BreakpointResult::NotHit(result)
                }
            },
            None => {
                // resume the execution
                self.start_the_loop(breakpoints, None)
            }
        };

        // handle transaction execution result if the transaction finishes
        let r = match result {
            BreakpointResult::Hit(bp) => RunResult::Breakpoint(bp),
            BreakpointResult::NotHit(mut result) => {
                let ctx = &mut self.evm.context;

                // handle output of call/create calls.
                self.evm
                    .handler
                    .execution()
                    .last_frame_return(ctx, &mut result);

                let post_exec = self.evm.handler.post_execution();
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
    fn start_the_loop<M, B: Breakpoint<M>>(
        &mut self,
        breakpoints: B,
        first_frame: Option<Frame>,
    ) -> BreakpointResult<M> {
        // take instruction talbe
        let table = self
            .evm
            .handler
            .take_instruction_table()
            .expect("Instruction table should be present");

        // run main loop
        let loop_result = match &table {
            InstructionTables::Plain(table) => {
                self.run_the_loop(breakpoints, table, first_frame)
            }
            InstructionTables::Boxed(table) => {
                self.run_the_loop(breakpoints, table, first_frame)
            }
        };

        // return back instruction table
        self.evm.handler.set_instruction_table(table);

        loop_result
    }

    #[inline]
    fn run_the_loop<FN, M, B: Breakpoint<M>>(
        &mut self,
        breakpoints: B,
        table: &[FN; 256],
        first_frame: Option<Frame>, // None indicate that this is an resumed execution.
    ) -> BreakpointResult<M>
    where
        FN: Fn(&mut Interpreter, &mut Evm<'a, I, S>),
    {
        let mut call_stack = self.take_call_stack();
        let mut shared_memory = self.take_shared_memory();

        let next_action = self.take_next_action();

        // push the top-level frame to the call stack, if this is a new transaction
        let mut loop_result = if let Some(first_frame) = first_frame {
            call_stack.push(first_frame);
            shared_memory.new_context();

            // check begin call breakpoint
            let maybe_breakpoint: Option<M> = breakpoints
                .should_break_begin_msg_call(
                    self,
                    call_stack.last().expect("call stack is empty"),
                );
            if let Some(breakpoint) = maybe_breakpoint {
                LoopResult::Pause {
                    breakpoint,
                    next: next_action,
                }
            } else {
                LoopResult::Continue(next_action)
            }
        } else {
            LoopResult::Continue(next_action)
        };

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
                    let exec = &self.evm.handler.execution;
                    // create a new call frame, and push to the call stack.
                    let frame_or_result = exec.call(
                        &mut self.evm.context,
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
                                &mut self.evm.context,
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
                    let exec = &self.evm.handler.execution;
                    // free memory context.
                    shared_memory.free_context();
                    // pop last frame from the stack and consume it to create FrameResult.
                    let returned_frame = call_stack
                        .pop()
                        .expect("We just returned from Interpreter frame");
                    // collect frame result
                    let ctx = &mut self.evm.context;
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
                        let ctx = &mut self.evm.context;
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
                    let exec = &self.evm.handler.execution;
                    // create a new create frame, and push to the call stack.
                    let frame_or_result =
                        exec.create(&mut self.evm.context, inputs);
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
                                &mut self.evm.context,
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
                    let action =
                        interpreter.run(shared_memory, table, &mut self.evm);
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
            let maybe_breakpoint: Option<M> = match &loop_action {
                Action::BeforeCall(inputs, _) => {
                    breakpoints.should_break_before_msg_call(self, &*inputs)
                }
                Action::AfterCall(address, result) => breakpoints
                    .should_break_after_msg_call(self, *address, result),
                Action::FrameBegin => {
                    let frame = call_stack.last().unwrap();
                    breakpoints.should_break_begin_msg_call(self, frame)
                }
                Action::FrameEnd(address, _) => {
                    let frame = call_stack.last().unwrap();
                    breakpoints.should_break_end_msg_call(self, *address, frame)
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
                self.replace_next_action(next);
                BreakpointResult::Hit(breakpoint)
            }
            LoopResult::Continue(_) => {
                unreachable!("LoopResult::Continue is not expected")
            }
            LoopResult::Finish(result) => BreakpointResult::NotHit(result),
        };

        self.replace_call_stack(call_stack);
        self.shared_memory = shared_memory;

        ret
    }
}
