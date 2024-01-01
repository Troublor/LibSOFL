use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema,
};

use crate::entities;
use libsofl_knowledge_base::entities as base_entities;

pub async fn setup_test_db() -> DatabaseConnection {
    // Connecting SQLite
    let db = Database::connect("sqlite::memory:").await.unwrap();
    // Setup Schema helper
    let schema = Schema::new(DbBackend::Sqlite);
    // Create the database
    let sql = schema.create_table_from_entity(base_entities::metadata::Entity);
    db.execute(db.get_database_backend().build(&sql))
        .await
        .unwrap();
    let sql = schema.create_table_from_entity(entities::creation::Entity);
    db.execute(db.get_database_backend().build(&sql))
        .await
        .unwrap();
    let sql = schema.create_table_from_entity(entities::invocation::Entity);
    db.execute(db.get_database_backend().build(&sql))
        .await
        .unwrap();
    db
}

#[cfg(not(feature = "test-using-jsonrpc"))]
use libsofl_reth::blockchain::provider::RethProvider;
#[cfg(not(feature = "test-using-jsonrpc"))]
pub fn get_bc_provider() -> RethProvider {
    use libsofl_reth::config::RethConfig;
    use libsofl_utils::config::Config;

    let cfg = RethConfig::must_load();
    cfg.bc_provider().unwrap()
}

#[cfg(feature = "test-using-jsonrpc")]
use libsofl_jsonrpc::provider::JsonRpcProvider;
#[cfg(feature = "test-using-jsonrpc")]
pub fn get_bc_provider() -> JsonRpcProvider {
    use libsofl_jsonrpc::config::JsonRpcConfig;
    use libsofl_utils::config::Config;

    let cfg = JsonRpcConfig::must_load();
    cfg.bc_provider().unwrap()
}
