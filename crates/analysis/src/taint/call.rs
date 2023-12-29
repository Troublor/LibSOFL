use super::memory::TaintableMemory;

#[derive(Clone, Debug, Default)]
pub struct TaintableCall {
    pub caller: bool, // is the call caller tainted?
    pub gas: bool,    // is the call gas tainted?
    pub calldata: TaintableMemory,
    pub value: bool, // is the call value tainted?

    pub status: bool, // is the call success tainted?
    pub return_data: TaintableMemory,
}
