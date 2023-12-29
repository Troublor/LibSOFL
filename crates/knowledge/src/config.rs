use libsofl_utils::config::Config;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct KnowledgeConfig {
    pub database: String,
}

impl Default for KnowledgeConfig {
    fn default() -> Self {
        Self {
            database: "postgres://localhost:5432/knowledge".to_string(),
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
        sea_orm::Database::connect(&self.database).await
    }
}
