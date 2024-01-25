// use revm::{interpreter::opcode::InstructionTables, Handler, Inspector};
// use revm_primitives::Spec;

// use crate::engine::{state::BcState, types::EVMData};

// pub struct InterruptableEVM<'a, GSPEC: Spec + 'static, S: BcState> {
//     pub data: EVMData<'a, S>,
//     pub inspector: Option<&'a mut dyn Inspector<S>>,
//     pub instruction_table: InstructionTables<'a, Self>,
//     pub handler: Handler<S>,
//     _phantom: std::marker::PhantomData<GSPEC>,
// }
