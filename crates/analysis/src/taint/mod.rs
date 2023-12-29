// Dynamic taint analysis for EVM execution.

pub mod inspector;
pub mod memory;
#[macro_use]
pub mod stack;
#[macro_use]
pub mod policy;
pub mod call;
pub mod storage;

use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
};

use libsofl_core::{
    engine::{
        state::BcState,
        types::{Address, EVMData, Interpreter},
    },
    error::SoflError,
};

use self::{
    call::TaintableCall, memory::TaintableMemory, policy::PropagationPolicy,
    stack::TaintableStack, storage::TaintableStorage,
};

pub struct TaintTracker<'a> {
    /// current stack
    pub stack: TaintableStack,

    /// current memory
    pub memory: TaintableMemory,

    /// current storage
    pub storage: &'a mut TaintableStorage,

    /// current call
    pub call: &'a mut TaintableCall,

    /// the recent child call
    /// This field is the next about-to-happen call if the current opcode is CREATE-like or CALL-like.
    /// This field is the most recent call (if it exists) in other opcode.
    pub child_call: Option<TaintableCall>,
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

pub struct TaintAnalyzer<'a, S: BcState, P: PropagationPolicy<S>> {
    policy: P,
    storages: RefCell<HashMap<Address, TaintableStorage>>,
    trackers: RefCell<Vec<TaintTracker<'a>>>,

    /// stack taint effects of the current instruction
    stack_taint_effects: Vec<Option<bool>>,
    _phantom: std::marker::PhantomData<S>,
}

impl<'a, S: BcState, P: PropagationPolicy<S>> TaintAnalyzer<'a, S, P> {
    fn new(policy: P, memory_word_size: usize) -> Self {
        Self {
            policy,
            storages: RefCell::new(HashMap::new()),
            trackers: RefCell::new(Vec::new()),
            stack_taint_effects: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, S: BcState, P: PropagationPolicy<S>> TaintAnalyzer<'a, S, P> {
    
}
