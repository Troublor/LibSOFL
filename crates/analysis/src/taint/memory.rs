/// TaintableMemory tracks tainted values in EVM memory.
#[derive(Clone, Debug, Default)]
pub struct TaintableMemory {
    memory: Vec<bool>,
    word_size: usize,
}

impl TaintableMemory {
    /// Create a new taintable memory.
    pub fn new(word_size: usize) -> Self {
        Self {
            memory: Vec::new(),
            word_size,
        }
    }
}

impl TaintableMemory {
    /// Taint a number of bytes starting from the given offset.
    /// The offset and size is the same as the one used in EVM memory.
    pub fn taint(&mut self, offset: usize, size: usize) {
        let start = offset / self.word_size;
        let end = (offset + size) / self.word_size;
        if end > self.memory.len() {
            self.memory.resize(end, false);
        }
        for i in start..end {
            self.memory[i] = true;
        }
    }

    /// Clean a number of bytes starting from the given offset.
    pub fn clean(&mut self, offset: usize, size: usize) {
        let start = offset / self.word_size;
        let end = (offset + size) / self.word_size;
        if end > self.memory.len() {
            self.memory.resize(end, false);
        }
        for i in start..end {
            self.memory[i] = false;
        }
    }

    /// Check if a number of bytes starting from the given offset is tainted.
    pub fn is_tainted(&self, offset: usize, size: usize) -> bool {
        let start = offset / self.word_size;
        let end = ((offset + size) / self.word_size).min(self.memory.len());
        for i in start..end {
            if self.memory[i] {
                return true;
            }
        }
        false
    }
}
