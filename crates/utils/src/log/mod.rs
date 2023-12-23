pub mod config;

pub use tracing::{debug, error, event, info, trace, warn, Level};
use tracing_subscriber::{
    layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

use self::config::must_load_cfg;

static INIT_ONCE: std::sync::Once = std::sync::Once::new();

pub fn init() {
    INIT_ONCE.call_once(|| {
        let cfg = must_load_cfg();

        // Console log
        let console_filter = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new(cfg.console_level))
            .expect("failed to create console logger filter");
        let console_layer = tracing_subscriber::fmt::layer()
            .with_target(false)
            .with_filter(console_filter);

        // File log
        if let Some(log_file) = cfg.file {
            let file_filter = EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(cfg.file_level))
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
        init();
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
