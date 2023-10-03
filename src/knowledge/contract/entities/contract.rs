use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contract")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub address: super::Address,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::invocation::Entity")]
    Invocation,

    #[sea_orm(has_many = "super::creation::Entity")]
    Creation,
}

impl Related<super::invocation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Invocation.def()
    }
}

impl Related<super::creation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Creation.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
