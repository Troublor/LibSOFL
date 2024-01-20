use libsofl_core::engine::types::Address;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "code")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub contract: String,

    /// source code
    pub source: serde_json::Value,

    /// language
    pub language: String,

    /// The bytecode to deploy the contract
    pub deployment_code: String,

    /// ABI
    pub abi: serde_json::Value,

    /// Storage layout
    pub storage_layout: serde_json::Value,

    /// contract name
    pub name: String,

    /// compiler version
    pub compiler: String,

    /// optimization used, None means no optimization
    pub optimization: Option<i32>,

    /// constructor arguments (hex encoded)
    pub constructor_args: String,

    /// EVM version
    pub evm_version: String,

    /// Library
    pub library: Option<String>,

    /// license type
    pub license: Option<String>,

    /// proxy
    pub proxy: bool,

    /// implementation
    pub implementation: Option<String>,

    /// Swarm source
    pub swarm_source: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
