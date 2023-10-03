use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "creation")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub contract: super::Address,
    #[sea_orm(primary_key, auto_increment = false)]
    pub create_tx: super::Hash,
    pub index: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contract::Entity",
        from = "Column::Contract",
        to = "super::contract::Column::Address"
    )]
    Contract,
}

impl Related<super::contract::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contract.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
