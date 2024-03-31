use std::sync::Arc;

use jsonrpsee::{core::async_trait, proc_macros::rpc};
use sea_orm::{DatabaseConnection, EntityTrait};

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
pub trait BaseRpc {
    #[method(name = "metadata")]
    async fn metadata(&self, key: String) -> Result<String, Error>;
}

pub struct BaseRpcImpl {
    pub db: Arc<DatabaseConnection>,
}

#[async_trait]
impl BaseRpcServer for BaseRpcImpl {
    async fn metadata(&self, key: String) -> Result<String, Error> {
        crate::entities::metadata::Entity::find_by_id(key)
            .one(self.db.as_ref())
            .await
            .map_err(|_| Error::Internal("Database error".to_string()))
            .and_then(|metadata| {
                metadata
                    .ok_or(Error::NotFound("Metadata not found".to_string()))
            })
            .map(|metadata| metadata.value)
    }
}
