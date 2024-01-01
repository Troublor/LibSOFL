mod create_metadata;
mod tx_index;

use libsofl_utils::config::Config;
pub use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(create_metadata::Migration),
            Box::new(tx_index::Migration),
        ]
    }
}

#[tokio::main]
async fn main() {
    // Set databse url env var to the one in the config file
    let cfg =
        libsofl_knowledge_index::config::KnowledgeConfig::load_or_default(
            Default::default(),
        )
        .expect("load config failed");
    let database_env = std::env::var("DATABASE_URL").ok();
    std::env::set_var("DATABASE_URL", cfg.database_url);

    cli::run_cli(Migrator).await;

    // Restore database url env var
    if let Some(database_env) = database_env {
        std::env::set_var("DATABASE_URL", database_env);
    } else {
        std::env::remove_var("DATABASE_URL")
    }
}
