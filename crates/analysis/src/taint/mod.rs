// Dynamic taint analysis for EVM execution.

pub mod inspector;
pub mod memory;
#[macro_use]
pub mod stack;
#[macro_use]
pub mod policy;
pub mod call;
pub mod propagation;
pub mod sink;
pub mod source;
pub mod storage;

use std::collections::HashMap;

use libsofl_core::{
    engine::{
        state::BcState,
        types::{Address, EVMData, Interpreter},
    },
    error::SoflError,
};

use self::{
    call::TaintableCall, memory::TaintableMemory, policy::TaintPolicy,
    stack::TaintableStack, storage::TaintableStorage,
};

pub struct TaintTracker<'a> {
    /// current stack
    pub stack: &'a mut TaintableStack,

    /// current memory
    pub memory: &'a mut TaintableMemory,

    /// current storage
    pub storage: &'a mut TaintableStorage,

    /// current call
    pub call: &'a mut TaintableCall,

    /// the recent child call
    /// This field is the next about-to-happen call if the current opcode is CREATE-like or CALL-like.
    /// This field is the most recent call (if it exists) in other opcode.
    pub child_call: Option<&'a mut TaintableCall>,
}

pub trait TaintMarker<S: BcState> {
    fn before_step(
        &self,
        taint_tracker: &mut TaintTracker,
        interp: &mut Interpreter<'_>,
        data: &mut EVMData<'_, S>,
    ) -> Result<(), SoflError>;

    fn after_step(
        &self,
        taint_tracker: &mut TaintTracker,
        interp: &mut Interpreter<'_>,
        data: &mut EVMData<'_, S>,
    ) -> Result<(), SoflError>;
}

pub trait TaintAnalysisSpec<S: BcState>: TaintMarker<S> {}

pub struct TaintAnalyzer<S: BcState, P: TaintPolicy<S>> {
    memory_word_size: usize,
    policy: P,
    storages: HashMap<Address, TaintableStorage>,

    // nested taintable objects (akin to call stack)
    stacks: Vec<TaintableStack>,
    memories: Vec<TaintableMemory>,
    calls: Vec<(TaintableCall, Option<u8>)>,
    child_calls: Vec<Option<TaintableCall>>,

    /// stack taint effects of the current instruction
    stack_taint_effects: Vec<Option<bool>>,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: BcState, P: TaintPolicy<S>> TaintAnalyzer<S, P> {
    #[allow(unused)]
    fn new(policy: P, memory_word_size: usize) -> Self {
        Self {
            memory_word_size,
            policy,
            storages: HashMap::new(),
            stacks: Vec::new(),
            memories: Vec::new(),
            calls: Vec::new(),
            child_calls: Vec::new(),
            stack_taint_effects: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<S: BcState, P: TaintPolicy<S>> TaintAnalyzer<S, P> {}
