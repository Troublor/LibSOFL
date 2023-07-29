use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contract")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub address: super::Address,
    pub create_tx: super::Hash,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::invocation::Entity")]
    Invocation,
}

impl Related<super::invocation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Invocation.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
