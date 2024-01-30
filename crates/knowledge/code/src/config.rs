use libsofl_utils::{config::Config, rate_limit::RateLimit};

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CodeKnowledgeConfig {
    pub chain_id: u64,
    pub api_keys: Vec<String>,
    pub requests_per_second: Option<f32>,
    pub cache_size: u64,
}

impl Default for CodeKnowledgeConfig {
    fn default() -> Self {
        Self {
            chain_id: 1,
            api_keys: vec![],
            requests_per_second: None,
            cache_size: 999,
        }
    }
}

impl Config for CodeKnowledgeConfig {
    fn section_name() -> &'static str {
        "code_knowledge"
    }
}

impl CodeKnowledgeConfig {
    pub fn get_rate_limit(&self) -> RateLimit {
        let rate_limit = if let Some(freq) = self.requests_per_second {
            RateLimit::new_frequency(freq)
        } else {
            RateLimit::unlimited()
        };
        rate_limit
    }
}
