use std::collections::HashMap;

use libsofl_core::engine::types::U256;

#[derive(Debug, Clone, Default)]
pub struct TaintableStorage {
    storage: HashMap<U256, bool>,
}

impl TaintableStorage {
    /// Create a new taintable storage.
    pub fn new() -> Self {
        Self {
            storage: HashMap::new(),
        }
    }
}

impl TaintableStorage {
    /// Taint a storage slot.
    pub fn taint(&mut self, index: U256) {
        self.storage.insert(index, true);
    }

    /// Clean a storage slot.
    pub fn clean(&mut self, index: U256) {
        self.storage.insert(index, false);
    }

    /// Check if a storage slot is tainted.
    pub fn is_tainted(&self, index: U256) -> bool {
        *self.storage.get(&index).unwrap_or(&false)
    }
}
