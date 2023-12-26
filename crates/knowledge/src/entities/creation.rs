use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "creation")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub contract: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub tx: String, // tx hash of the transaction that creates or destroys the contract
    pub index: i32, // the index of the contract creation in the history
    pub destruct: bool, // whether the contract is created or destroyed in this transaction
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
