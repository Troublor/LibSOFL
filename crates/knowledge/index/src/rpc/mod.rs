pub mod service;

use std::sync::Arc;

use jsonrpsee::{core::async_trait, proc_macros::rpc};
use libsofl_core::engine::types::{Address, TxHash};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

#[derive(Debug)]
pub enum Error {
    NotFound(String),
    Internal(String),
}

impl From<Error> for jsonrpsee::types::ErrorObject<'static> {
    fn from(value: Error) -> Self {
        match value {
            Error::Internal(msg) => jsonrpsee::types::ErrorObject::owned(
                jsonrpsee::types::error::INTERNAL_ERROR_CODE,
                jsonrpsee::types::error::INTERNAL_ERROR_MSG,
                Some(msg),
            ),
            Error::NotFound(msg) => jsonrpsee::types::ErrorObject::owned(
                jsonrpsee::types::error::INVALID_PARAMS_CODE,
                jsonrpsee::types::error::INVALID_PARAMS_MSG,
                Some(msg),
            ),
        }
    }
}

#[rpc(client, server, namespace = "kb")]
pub trait IndexRpc {
    #[method(name = "creation")]
    async fn creation(
        &self,
        contract: Address,
    ) -> Result<Vec<(TxHash, i64, bool)>, Error>;

    #[method(name = "invoked_blocks")]
    async fn invoked_blocks(
        &self,
        address: Address,
    ) -> Result<Vec<(i64, i64)>, Error>;
}

pub struct IndexRpcImpl {
    pub db: Arc<DatabaseConnection>,
}

#[async_trait]
impl IndexRpcServer for IndexRpcImpl {
    async fn creation(
        &self,
        contract: Address,
    ) -> Result<Vec<(TxHash, i64, bool)>, Error> {
        let models = crate::entities::creation::Entity::find()
            .filter(
                crate::entities::creation::Column::Contract
                    .eq(contract.to_string()),
            )
            .all(self.db.as_ref())
            .await
            .map_err(|err| Error::Internal(err.to_string()))?;
        let mut rs = vec![];
        for model in models {
            let tx: TxHash = model.tx.parse().expect("failed to parse tx hash");
            let bn = model.block;
            let is_creation = model.destruct;
            rs.push((tx, bn, is_creation));
        }
        // sort by block number
        rs.sort_by(|a, b| a.1.cmp(&b.1));
        Ok(rs)
    }

    async fn invoked_blocks(
        &self,
        contract: Address,
    ) -> Result<Vec<(i64, i64)>, Error> {
        let models = crate::entities::invocation::Entity::find()
            .filter(
                crate::entities::invocation::Column::Contract
                    .eq(contract.to_string()),
            )
            .all(self.db.as_ref())
            .await
            .map_err(|err| Error::Internal(err.to_string()))?;
        let mut rs = vec![];
        for model in models {
            let from_bn = model.from_block;
            let to_bn = model.to_block;
            rs.push((from_bn, to_bn));
        }
        // sort by from block number
        rs.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(rs)
    }
}
