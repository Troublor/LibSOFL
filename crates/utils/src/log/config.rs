use crate::config::{Config, ConfigLoader};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogConfig {
    pub console_level: String,
    pub file_level: String,
    pub file: Option<String>,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            console_level: "info".to_string(),
            file_level: "info".to_string(),
            file: None,
        }
    }
}

pub static CONFIG_SECTION: &str = "log";

impl Config for LogConfig {}

pub fn must_load_cfg() -> LogConfig {
    ConfigLoader::load_cfg_or_default(CONFIG_SECTION, Default::default())
        .expect("failed to load log config")
}
