use std::str::FromStr;

use libsofl_utils::config::Config;
use sea_orm::ConnectOptions;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct KnowledgeConfig {
    pub database_url: String,
    pub database_log_level: String,
}

impl Default for KnowledgeConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://localhost:5432/knowledge".to_string(),
            database_log_level: "info".to_string(),
        }
    }
}

impl Config for KnowledgeConfig {
    fn section_name() -> &'static str {
        "knowledge"
    }
}

impl KnowledgeConfig {
    pub async fn get_database_connection(
        &self,
    ) -> Result<sea_orm::DatabaseConnection, sea_orm::DbErr> {
        let mut opt = ConnectOptions::new(self.database_url.to_owned());
        opt.sqlx_logging(false) // Disable SQLx log
            .sqlx_logging_level(
                log::LevelFilter::from_str(&self.database_log_level)
                    .expect("invalid database log level"),
            ); // Set SQLx log level

        sea_orm::Database::connect(opt).await
    }
}
