use super::memory::TaintableMemory;

#[derive(Clone, Debug)]
pub struct TaintableCall {
    pub code: TaintableMemory,
    pub caller: bool, // is the call caller tainted?
    pub gas: bool,    // is the call gas tainted?
    pub calldata: TaintableMemory,
    pub value: bool, // is the call value tainted?

    pub status: bool, // is the call success tainted?
    pub return_data: TaintableMemory,
}

impl TaintableCall {
    /// Create a new taintable call.
    pub fn new(word_size: usize) -> Self {
        Self {
            code: TaintableMemory::new(word_size),
            caller: false,
            gas: false,
            calldata: TaintableMemory::new(word_size),
            value: false,
            status: false,
            return_data: TaintableMemory::new(word_size),
        }
    }
}
