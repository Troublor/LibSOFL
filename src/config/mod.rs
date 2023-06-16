pub mod flags;

use std::env;

use self::flags::SoflConfig;
use config::{Config, ConfigError, Environment, File, FileFormat};

impl SoflConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let default_config = SoflConfig::default();
        let default_source = Config::try_from(&default_config)?;
        let config_file = env::var("SEEFUZZ_CONFIG").unwrap_or_else(|_| "config".into());
        let cfg = Config::builder()
            .add_source(default_source)
            .add_source(File::new(config_file.as_str(), FileFormat::Toml).required(false))
            .add_source(Environment::with_prefix("SEEFUZZ"))
            .build()?;
        cfg.try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use crate::config::flags::SoflConfig;

    #[test]
    fn test_default_config() {
        let cfg = super::SoflConfig::load().unwrap();
        assert_eq!(cfg.reth.datadir, SoflConfig::default().reth.datadir);
    }

    #[test]
    fn test_load_local_config() {
        env::set_var("SEEFUZZ_CONFIG", "tests/data/test_config.toml");
        let cfg = super::SoflConfig::load().unwrap();
        assert_eq!(cfg.reth.datadir, "blockchain");
    }
}
