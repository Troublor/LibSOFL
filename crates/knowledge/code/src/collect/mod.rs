// collect contract code from blockchain

use serde::{Deserialize, Serialize};

pub mod collector;
pub mod contract_inspector;

pub static CODE_KNOWLEDGE_METADATA_KEY: &str = "code_knowledge";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodeKnowledgeMetadata {
    pub progress: u64,
}
