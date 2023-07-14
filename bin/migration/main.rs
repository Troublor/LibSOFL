pub use sea_orm_migration::prelude::*;

mod m20230714_015008_extract_call;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20230714_015008_extract_call::Migration)]
    }
}

#[async_std::main]
async fn main() {
    cli::run_cli(Migrator).await;
}
