use libsofl_core::error::SoflError;
pub use serde::{Deserialize, Serialize};

pub static CONFIG_FILE_ENV_VAR: &str = "SOFL_CONFIG";
pub static CONFIG_ENV_PREFIX: &str = "SOFL";
pub static CONFIG_ENV_SEPARATOR: &str = "_";

pub trait Config: Deserialize<'static> + Serialize {
    fn section_name() -> &'static str;

    fn load_or_default(default: Self) -> Result<Self, SoflError> {
        let section = Self::section_name();
        let config_file = std::env::var(CONFIG_FILE_ENV_VAR)
            .unwrap_or_else(|_| "config.toml".to_string());
        let default_source =
            config::Config::try_from(&default).map_err(|e| {
                SoflError::Config(format!(
                    "failed to load default config: {}",
                    e
                ))
            })?;
        let cfg = config::Config::builder()
            .add_source(
                config::Environment::with_prefix(CONFIG_ENV_PREFIX)
                    .separator(CONFIG_ENV_SEPARATOR),
            )
            .add_source(
                config::File::new(&config_file, config::FileFormat::Toml)
                    .required(false),
            )
            .add_source(default_source)
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
    }
    impl Config for TestConfig {
        fn section_name() -> &'static str {
            "abc"
        }
    }

    #[test]
    #[ignore = "Run this test together with others will fail. But run it alone will pass."]
    fn test_load_cfg() {
        let mut file = NamedTempFile::new().unwrap();
        let config_txt = r#"
        [abc]
        test = "abc"
        "#;
        file.write_all(config_txt.as_bytes()).unwrap();
        std::env::set_var(CONFIG_FILE_ENV_VAR, file.path().as_os_str());
        let cfg: TestConfig =
            TestConfig::load_or_default(Default::default()).unwrap();
        assert_eq!(cfg.test, "abc");
        std::env::remove_var(CONFIG_FILE_ENV_VAR);
    }

    #[test]
    #[ignore = "Run this test together with others will fail. But run it alone will pass."]
    fn test_env_override() {
        std::env::set_var(
            CONFIG_ENV_PREFIX.to_owned()
                + CONFIG_ENV_SEPARATOR
                + "abc"
                + CONFIG_ENV_SEPARATOR
                + "test",
            "def",
        );
        let cfg = TestConfig::load_or_default(TestConfig::default()).unwrap();
        assert_eq!(cfg.test, "def")
    }

    #[test]
    #[ignore = "Run this test together with others will fail. But run it alone will pass."]
    fn test_no_default() {
        std::env::set_var(CONFIG_FILE_ENV_VAR, "/dev/non_exist");
        let cfg: Result<TestConfig, SoflError> = TestConfig::load();
        assert!(cfg.is_err());
        std::env::remove_var(CONFIG_FILE_ENV_VAR);
    }
}
