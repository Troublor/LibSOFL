use config::Source;
use libsofl_core::error::SoflError;
pub use serde::{Deserialize, Serialize};

pub static CONFIG_FILE_ENV_VAR: &str = "SOFL_CONFIG";
pub static CONFIG_ENV_PREFIX: &str = "SOFL";
pub static CONFIG_ENV_SEPARATOR: &str = "_";

pub trait Config: Deserialize<'static> + Serialize {
    fn section_name() -> &'static str;

    fn load_or(default: Self) -> Result<Self, SoflError> {
        let section = Self::section_name();
        let config_file = std::env::var(CONFIG_FILE_ENV_VAR)
            .unwrap_or_else(|_| "config.toml".to_string());
        let default_source = config::Config::try_from(&default)
            .map_err(|e| {
                SoflError::Config(format!(
                    "failed to load default config: {}",
                    e
                ))
            })?
            .collect()
            .expect("failed to collect default config");
        let cfg = config::Config::builder()
            .set_default(Self::section_name(), default_source)
            .map_err(|e| {
                SoflError::Config(format!(
                    "failed to build config builder: {}",
                    e
                ))
            })?
            .add_source(
                config::File::new(&config_file, config::FileFormat::Toml)
                    .required(false),
            )
            .add_source(
                config::Environment::with_prefix(CONFIG_ENV_PREFIX)
                    .separator(CONFIG_ENV_SEPARATOR),
            )
            .build()
            .map_err(|e| {
                SoflError::Config(format!(
                    "failed to build config builder: {}",
                    e
                ))
            })?;
        let c: Self = cfg.get(section).or_else(|e| match e {
            config::ConfigError::NotFound(_) => Ok(default),
            _ => Err(SoflError::Config(format!("{}", e))),
        })?;
        Ok(c)
    }

    fn load() -> Result<Self, SoflError> {
        let section = Self::section_name();
        let config_file = std::env::var(CONFIG_FILE_ENV_VAR)
            .unwrap_or_else(|_| "config.toml".to_string());
        let cfg = config::Config::builder()
            .add_source(
                config::Environment::with_prefix(CONFIG_ENV_PREFIX)
                    .separator(CONFIG_ENV_SEPARATOR),
            )
            .add_source(
                config::File::new(&config_file, config::FileFormat::Toml)
                    .required(false),
            )
            .build()
            .map_err(|e| {
                SoflError::Config(format!(
                    "failed to build config builder: {}",
                    e
                ))
            })?;
        let c: Self = cfg
            .get(section)
            .map_err(|e| SoflError::Config(format!("{}", e)))?;
        Ok(c)
    }

    fn must_load() -> Self {
        Self::load().expect("failed to load config")
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, CONFIG_ENV_PREFIX, CONFIG_ENV_SEPARATOR};
    use crate::config::CONFIG_FILE_ENV_VAR;
    use libsofl_core::error::SoflError;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[derive(
        Debug,
        Clone,
        Eq,
        PartialEq,
        Default,
        serde::Deserialize,
        serde::Serialize,
    )]
    struct TestConfig {
        test: String,
        other: String,
    }
    impl Config for TestConfig {
        fn section_name() -> &'static str {
            "abc"
        }
    }

    #[test]
    #[ignore = "Run this test together with others will fail. But run it alone will pass."]
    fn test_load_cfg_file() {
        let mut file = NamedTempFile::new().unwrap();
        let config_txt = r#"
        [abc]
        test = "abc"
        other = "123"
        "#;
        file.write_all(config_txt.as_bytes()).unwrap();
        std::env::set_var(CONFIG_FILE_ENV_VAR, file.path().as_os_str());
        let cfg: TestConfig = TestConfig::load_or(Default::default()).unwrap();
        assert_eq!(cfg.test, "abc");
        std::env::remove_var(CONFIG_FILE_ENV_VAR);
    }

    #[test]
    #[ignore = "Run this test together with others will fail. But run it alone will pass."]
    fn test_env_override() {
        let mut file = NamedTempFile::new().unwrap();
        let config_txt = r#"
        [abc]
        test = "abc"
        other = "123"
        "#;
        file.write_all(config_txt.as_bytes()).unwrap();
        std::env::set_var(CONFIG_FILE_ENV_VAR, file.path().as_os_str());

        let env_var = CONFIG_ENV_PREFIX.to_owned()
            + CONFIG_ENV_SEPARATOR
            + "abc"
            + CONFIG_ENV_SEPARATOR
            + "test";
        std::env::set_var(env_var.clone(), "def");
        let cfg = TestConfig::load_or(TestConfig::default()).unwrap();
        assert_eq!(cfg.test, "def");
        std::env::remove_var(env_var);
        std::env::remove_var(CONFIG_FILE_ENV_VAR);
    }

    #[test]
    #[ignore = "Run this test together with others will fail. But run it alone will pass."]
    fn test_no_default() {
        std::env::set_var(CONFIG_FILE_ENV_VAR, "/dev/non_exist");
        let cfg: Result<TestConfig, SoflError> = TestConfig::load();
        assert!(cfg.is_err());
        std::env::remove_var(CONFIG_FILE_ENV_VAR);
    }

    #[test]
    #[ignore = "Run this test together with others will fail. But run it alone will pass."]
    fn test_partial_default() {
        let mut file = NamedTempFile::new().unwrap();
        let config_txt = r#"
        [abc]
        test = "abc"
        "#;
        file.write_all(config_txt.as_bytes()).unwrap();
        std::env::set_var(CONFIG_FILE_ENV_VAR, file.path().as_os_str());

        let cfg: TestConfig = TestConfig::load_or(Default::default()).unwrap();
        assert_eq!(cfg.test, "abc");
        assert_eq!(cfg.other, "");

        std::env::remove_var(CONFIG_FILE_ENV_VAR);
    }
}
