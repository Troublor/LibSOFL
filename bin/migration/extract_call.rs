use sea_orm::Schema;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());
        manager
            .create_table(schema.create_table_from_entity(
                libsofl::knowledge::contract::entities::contract::Entity,
            ))
            .await?;
        manager
            .create_table(schema.create_table_from_entity(
                libsofl::knowledge::contract::entities::creation::Entity,
            ))
            .await?;
        manager
            .create_table(schema.create_table_from_entity(
                libsofl::knowledge::contract::entities::invocation::Entity,
            ))
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(libsofl::knowledge::contract::entities::invocation::Entity).to_owned())
            .await?;
        manager.drop_table(Table::drop().table(libsofl::knowledge::contract::entities::creation::Entity).to_owned()).await?;
        manager
            .drop_table(Table::drop().table(libsofl::knowledge::contract::entities::contract::Entity).to_owned())
            .await
    }
}
