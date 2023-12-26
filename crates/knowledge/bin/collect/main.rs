pub mod analyze;

use libsofl_core::engine::{
    inspector::CombinedInspector, memory::MemoryBcState,
};
use libsofl_jsonrpc::state::JsonrRpcBcStateRef;
use libsofl_knowledge::inspectors::{
    extract_creation::ExtractCreationInspector,
    extract_invocation::ExtractInvocationInspector,
};

fn main() {
    println!("Hello, world!");
}
