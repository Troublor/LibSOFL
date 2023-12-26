use crate::config::Config;

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

impl Config for LogConfig {
    fn section_name() -> &'static str {
        "log"
    }
}
