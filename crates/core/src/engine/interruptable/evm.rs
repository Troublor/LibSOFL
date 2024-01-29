use revm::Evm;

use crate::engine::{
    inspector::{EvmInspector, InspectorContext},
    interruptable::breakpoint::RunResult,
    revm::revm::{
        interpreter::{
            opcode::InstructionTables, Interpreter, InterpreterAction,
            InterpreterResult,
        },
        primitives::{EVMError, Output, TransactTo},
        FrameOrResult,
    },
    state::BcState,
    types::CallInputs,
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
    ) -> Result<RunResult, EVMError<S::Error>> {
        self.run_until(context, vec![])
    }

    /// Run evm until one of the breakpoints.
    #[inline]
    pub fn run_until(
        &self,
        context: &mut ResumableContext<S>,
        breakpoints: Vec<Breakpoint>,
    ) -> Result<RunResult, EVMError<S::Error>> {
        let result = {
            if context.is_empty() {
                // begin a new transaction

                // validate transaction
                context
                    .revm_ctx
                    .handler
                    .validation()
                    .env(&context.revm_ctx.context.evm.env)?;
                let initial_gas_spend =
                    context
                        .revm_ctx
                        .handler
                        .validation()
                        .initial_tx_gas(&context.revm_ctx.context.evm.env)?;
                context
                    .revm_ctx
                    .handler
                    .validation()
                    .tx_against_state(&mut context.revm_ctx.context)?;

                // transaction preparation
                let hndl = &mut context.revm_ctx.handler;
                let ctx = &mut context.revm_ctx.context;

                // load access list and beneficiary if needed.
                hndl.pre_execution().load_accounts(ctx)?;

                // load precompiles
                let precompiles = hndl.pre_execution().load_precompiles();
                ctx.evm.set_precompiles(precompiles);

                // deduce caller balance with its limit.
                hndl.pre_execution().deduct_caller(ctx)?;
                // gas limit used in calls.
                let first_frame = hndl.execution_loop().create_first_frame(
                    ctx,
                    ctx.evm.env.tx.gas_limit - initial_gas_spend,
                );
                match first_frame {
                    FrameOrResult::Frame(first_frame) => {
                        // get created address if any
                        context.created_address = first_frame.created_address();
                        // push first frame to call stack
                        context.call_stack.push(first_frame);
                        // run from top-level call
                        self.run_until_inner_wrapper(context, breakpoints)?
                    }
                    FrameOrResult::Result(result) => {
                        // no contract invocation
                        BreakpointResult::NotNit(result)
                    }
                }
            } else {
                // continue running from breakpoint
                self.run_until_inner_wrapper(context, breakpoints)?
            }
        };

        let output = match result {
            BreakpointResult::NotNit(result) => {
                // get transaction output
                let main_output =
                    match context.revm_ctx.context.evm.env.tx.transact_to {
                        TransactTo::Call(_) => {
                            Output::Call(result.output.clone())
                        }
                        TransactTo::Create(_) => Output::Create(
                            result.output.clone(),
                            context.created_address,
                        ),
                    };

                // transaction finishes
                let hndl = &mut context.revm_ctx.handler;
                let ctx = &mut context.revm_ctx.context;

                // handle output of call/create calls.
                let gas = hndl.execution_loop().first_frame_return(
                    &ctx.evm.env,
                    result.result,
                    result.gas,
                );
                // Reimburse the caller
                hndl.post_execution().reimburse_caller(ctx, &gas)?;
                // Reward beneficiary
                hndl.post_execution().reward_beneficiary(ctx, &gas)?;
                // Returns output of transaction.
                let output = hndl.post_execution().output(
                    ctx,
                    result.result,
                    main_output,
                    &gas,
                )?;
                RunResult::Done((output.state, output.result))
            }
            BreakpointResult::Hit(bp) => RunResult::Breakpoint(bp),
        };
        Ok(output)
    }

    /// Run evm until one of the breakpoints.
    #[inline]
    pub fn run_until_inner_wrapper(
        &self,
        context: &mut ResumableContext<S>,
        breakpoints: Vec<Breakpoint>,
    ) -> Result<BreakpointResult, EVMError<S::Error>> {
        // take instruction talbe
        let instruction_table = context
            .revm_ctx
            .handler
            .take_instruction_table()
            .expect("Instruction table should be present");
        match &instruction_table {
            InstructionTables::Plain(table) => {
                self.run_until_inner(context, table, breakpoints)
            }
            InstructionTables::Boxed(table) => {
                self.run_until_inner(context, table, breakpoints)
            }
        }
    }

    /// Run evm until one of the breakpoints.
    #[inline]
    pub fn run_until_inner<'a, FN>(
        &self,
        context: &mut ResumableContext<'a, S>,
        instruction_table: &[FN; 256],
        breakpoints: Vec<Breakpoint>,
    ) -> Result<BreakpointResult, EVMError<S::Error>>
    where
        FN: Fn(&mut Interpreter, &mut Evm<'a, InspectorContext<'a, S>, S>),
    {
        // take call stack, shared memory, maybe_action for running interpreter
        let mut call_stack = context.take_call_stack();
        let mut shared_memory = context.take_shared_memory();
        let mut maybe_action = context.take_next_action();

        let breakpoint_result;

        loop {
            match maybe_action {
                Some(action) => {
                    // action may be breakpoints
                    match action {
                        InterpreterAction::SubCall {
                            inputs,
                            return_memory_offset,
                        } => {
                            let callee = inputs.contract;
                            // check MsgCallBefore breakpoint
                            let breakpoint = breakpoints
                                .iter()
                                .filter(|b| {
                                    if let Breakpoint::MsgCallBefore(addr) = b {
                                        *addr == callee
                                    } else {
                                        false
                                    }
                                })
                                .next();
                            if let Some(bp) = breakpoint {
                                // if breakpoint is hit, break interpretation loop
                                maybe_action =
                                    Some(InterpreterAction::SubCall {
                                        inputs,
                                        return_memory_offset,
                                    });
                                breakpoint_result =
                                    BreakpointResult::Hit(bp.clone());
                                break;
                            }

                            // otherwise step into the subcall
                            let current_stack_frame =
                                call_stack.last_mut().unwrap();
                            let sub_call_frame = context
                                .revm_ctx
                                .handler
                                .execution_loop()
                                .sub_call(
                                    &mut context.revm_ctx.context,
                                    inputs,
                                    current_stack_frame,
                                    &mut shared_memory,
                                    return_memory_offset,
                                );
                            if let Some(new_frame) = sub_call_frame {
                                shared_memory.new_context();
                                call_stack.push(new_frame);
                            }

                            // check MsgCallBegin breakpoint
                            let breakpoint = breakpoints
                                .iter()
                                .filter(|b| {
                                    if let Breakpoint::MsgCallBegin(addr) = b {
                                        *addr == callee
                                    } else {
                                        false
                                    }
                                })
                                .next();
                            if let Some(bp) = breakpoint {
                                // if breakpoint is hit, break interpretation loop
                                maybe_action = None;
                                breakpoint_result =
                                    BreakpointResult::Hit(bp.clone());
                                break;
                            }

                            // no action, continue
                            maybe_action = None;
                            continue;
                        }
                        InterpreterAction::Create { inputs } => {
                            // TODO: breakpoints for create

                            let current_stack_frame =
                                call_stack.last_mut().unwrap();
                            let sub_call_frame = context
                                .revm_ctx
                                .handler
                                .execution_loop()
                                .sub_create(
                                    &mut context.revm_ctx.context,
                                    current_stack_frame,
                                    inputs,
                                );
                            if let Some(new_frame) = sub_call_frame {
                                shared_memory.new_context();
                                call_stack.push(new_frame);
                            }

                            // no action, continue
                            maybe_action = None;
                            continue;
                        }
                        InterpreterAction::Return { result } => {
                            // free memory context.
                            shared_memory.free_context();

                            let child_frame = call_stack.pop().unwrap();
                            let parent_frame = call_stack.last_mut();

                            if let Some(r) = context
                                .revm_ctx
                                .handler
                                .execution_loop()
                                .frame_return(
                                    &mut context.revm_ctx.context,
                                    child_frame,
                                    parent_frame,
                                    &mut shared_memory,
                                    result,
                                )
                            {
                                // top-level return
                                maybe_action = None;
                                breakpoint_result = BreakpointResult::NotNit(r);
                                break;
                            }

                            // no action, continue
                            maybe_action = None;
                            continue;
                        }
                    }
                }
                None => {
                    // continue run the interpreter
                    let current_stack_frame = call_stack.last_mut().unwrap();
                    let action = current_stack_frame.interpreter.run(
                        shared_memory,
                        instruction_table,
                        &mut context.revm_ctx,
                    );
                    maybe_action = Some(action);
                    // take shared memory back.
                    shared_memory =
                        current_stack_frame.interpreter.take_memory();
                }
            };
        }

        // put back call stack, shared memory, and next action
        context.call_stack = call_stack;
        context.shared_memory = shared_memory;
        context.next_action = maybe_action;

        Ok(breakpoint_result)
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
