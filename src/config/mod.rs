pub mod flags;

use std::{env, path::Path};

use self::flags::SoflConfig;
use config::{Config, ConfigError, Environment, File, FileFormat};

impl SoflConfig {
    pub fn load_file(config_file: &Path) -> Result<Self, ConfigError> {
        let default_config = SoflConfig::default();
        let default_source = Config::try_from(&default_config)?;
        let cfg = Config::builder()
            .add_source(default_source)
            .add_source(File::new(config_file.to_str().unwrap(), FileFormat::Toml).required(false))
            .add_source(Environment::with_prefix("SOFL"))
            .build()?;
        cfg.try_deserialize()
    }

    pub fn load() -> Result<Self, ConfigError> {
        let config_file = env::var("SOFL_CONFIG").unwrap_or_else(|_| "config".into());
        Self::load_file(Path::new(&config_file))
    }
}

#[cfg(test)]
mod tests_nodep {
    use std::{env, path::Path};

    use crate::config::flags::SoflConfig;

    #[test]
    fn test_default_config() {
        let cfg = super::SoflConfig::load_file(Path::new("/dev/null")).unwrap();
        assert_eq!(cfg.reth.datadir, SoflConfig::default().reth.datadir);
    }

    #[test]
    fn test_load_local_config() {
        let cfg = super::SoflConfig::load_file(Path::new("tests/data/test_config.toml")).unwrap();
        assert_eq!(cfg.reth.datadir, "blockchain");
    }
}
