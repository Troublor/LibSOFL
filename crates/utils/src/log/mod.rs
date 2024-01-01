pub mod config;

pub use tracing::{
    debug, debug_span, error, error_span, event, info, info_span, span, trace,
    trace_span, warn, warn_span, Level, Subscriber,
};
use tracing_subscriber::{
    layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

use crate::config::Config;

use self::config::LogConfig;

static INIT_ONCE: std::sync::Once = std::sync::Once::new();

impl LogConfig {
    pub fn init(&self) {
        INIT_ONCE.call_once(|| {
            // Console log
            let console_filter = EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(self.console_level.clone()))
                .expect("failed to create console logger filter");
            let console_layer = tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_filter(console_filter);

            // File log
            if let Some(log_file) = self.file.clone() {
                let file_filter = EnvFilter::try_from_default_env()
                    .or_else(|_| EnvFilter::try_new(self.file_level.clone()))
                    .expect("failed to create file logger filter");
                let file_layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(move || {
                        let file = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&log_file)
                            .expect("failed to open log file");
                        Box::new(file) as Box<dyn std::io::Write + Send + Sync>
                    })
                    .with_filter(file_filter);
                tracing_subscriber::registry()
                    .with(console_layer)
                    .with(file_layer)
                    .init();
            } else {
                tracing_subscriber::registry().with(console_layer).init();
            }
        });
    }
}

pub fn init_logging_with_config(cfg: LogConfig) {
    cfg.init();
}

pub fn must_init_logging() {
    let cfg = LogConfig::must_load();
    init_logging_with_config(cfg);
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use tempfile::NamedTempFile;

    use crate::config::CONFIG_FILE_ENV_VAR;

    use super::*;

    #[test]
    fn test_log() {
        let mut cfg_file = NamedTempFile::new().unwrap();
        let log_file = NamedTempFile::new().unwrap();
        let cfg = format!(
            r#"
        [log]
        console_level = "info"
        file_level = "info"
        file = "{}"
        "#,
            log_file.path().to_str().unwrap()
        );
        cfg_file.write_all(cfg.as_bytes()).unwrap();
        std::env::set_var(CONFIG_FILE_ENV_VAR, cfg_file.path().as_os_str());
        must_init_logging();
        info!(key = "word", "test log");
        debug!("should not exist");
        let mut log_content = String::new();
        log_file
            .reopen()
            .unwrap()
            .read_to_string(&mut log_content)
            .unwrap();
        assert!(log_content.contains("test log"));
        assert!(!log_content.contains("should not exist"));
    }
}
