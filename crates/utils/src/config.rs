use libsofl_core::error::SoflError;
pub use serde::{Deserialize, Serialize};
pub trait Config: Deserialize<'static> + Serialize {}

pub struct ConfigLoader {}

pub static CONFIG_FILE_ENV_VAR: &str = "SOFL_CONFIG";
pub static CONFIG_ENV_PREFIX: &str = "SOFL";
pub static CONFIG_ENV_SEPARATOR: &str = "_";

impl ConfigLoader {
    pub fn load_cfg_or_default<T: Config>(section: &str, default: T) -> Result<T, SoflError> {
        let config_file =
            std::env::var(CONFIG_FILE_ENV_VAR).unwrap_or_else(|_| "config.toml".to_string());
        let default_source = config::Config::try_from(&default)
            .map_err(|e| SoflError::Config(format!("failed to load default config: {}", e)))?;
        let cfg = config::Config::builder()
            .add_source(
                config::Environment::with_prefix(CONFIG_ENV_PREFIX).separator(CONFIG_ENV_SEPARATOR),
            )
            .add_source(config::File::new(&config_file, config::FileFormat::Toml).required(false))
            .add_source(default_source)
            .build()
            .map_err(|e| SoflError::Config(format!("failed to build config builder: {}", e)))?;
        let c: T = cfg.get(section).or_else(|e| match e {
            config::ConfigError::NotFound(_) => Ok(default),
            _ => Err(SoflError::Config(format!("{}", e))),
        })?;
        Ok(c)
    }

    pub fn load_cfg<T: Config>(section: &str) -> Result<T, SoflError> {
        let config_file =
            std::env::var(CONFIG_FILE_ENV_VAR).unwrap_or_else(|_| "config.toml".to_string());
        let cfg = config::Config::builder()
            .add_source(
                config::Environment::with_prefix(CONFIG_ENV_PREFIX).separator(CONFIG_ENV_SEPARATOR),
            )
            .add_source(config::File::new(&config_file, config::FileFormat::Toml).required(false))
            .build()
            .map_err(|e| SoflError::Config(format!("failed to build config builder: {}", e)))?;
        let c: T = cfg
            .get(section)
            .map_err(|e| SoflError::Config(format!("{}", e)))?;
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, CONFIG_ENV_PREFIX, CONFIG_ENV_SEPARATOR};
    use crate::config::{ConfigLoader, CONFIG_FILE_ENV_VAR};
    use libsofl_core::error::SoflError;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[derive(Debug, Clone, Eq, PartialEq, Default, serde::Deserialize, serde::Serialize)]
    struct TestConfig {
        test: String,
    }
    impl Config for TestConfig {}

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
        let cfg: TestConfig = ConfigLoader::load_cfg_or_default("abc", Default::default()).unwrap();
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
        let cfg = ConfigLoader::load_cfg_or_default("abc", TestConfig::default()).unwrap();
        assert_eq!(cfg.test, "def")
    }

    #[test]
    #[ignore = "Run this test together with others will fail. But run it alone will pass."]
    fn test_no_default() {
        std::env::set_var(CONFIG_FILE_ENV_VAR, "/dev/non_exist");
        let cfg: Result<TestConfig, SoflError> = ConfigLoader::load_cfg("abc");
        assert!(cfg.is_err());
        std::env::remove_var(CONFIG_FILE_ENV_VAR);
    }
}
