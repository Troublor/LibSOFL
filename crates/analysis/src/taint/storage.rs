use std::collections::HashMap;

use libsofl_core::engine::types::{Address, U256};

#[derive(Debug, Clone, Default)]
pub struct TaintableStorage {
    storage: HashMap<Address, HashMap<U256, bool>>,
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
    pub fn taint(&mut self, address: Address, index: U256) {
        let entry = self.storage.entry(address).or_insert_with(HashMap::new);
        entry.insert(index, true);
    }

    /// Clean a storage slot.
    pub fn clean(&mut self, address: Address, index: U256) {
        let entry = self.storage.entry(address).or_insert_with(HashMap::new);
        entry.insert(index, false);
    }

    /// Check if a storage slot is tainted.
    pub fn is_tainted(&self, address: Address, index: U256) -> bool {
        let entry = self.storage.get(&address);
        if let Some(entry) = entry {
            let entry = entry.get(&index);
            if let Some(entry) = entry {
                return *entry;
            }
        }
        false
    }
}
