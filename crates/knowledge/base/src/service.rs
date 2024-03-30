use std::sync::Arc;

use eyre::Result;
use jsonrpsee::{core::async_trait, Methods};
use libsofl_reth::blockchain::transaction::RethTx;
use sea_orm::DatabaseConnection;

use crate::rpc::{BaseRpcImpl, BaseRpcServer};

#[async_trait]
pub trait KnowledgeService: Send + Sync {
    fn name(&self) -> &str;

    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;

    /// Get the RPC methods for this service.
    fn rpc_methods(&self) -> Methods;

    /// Called when a new block is mined.
    async fn on_new_block(
        &mut self,
        block_number: u64,
        txs: Vec<RethTx>,
    ) -> Result<()>;
}

pub struct BaseService {
    pub db: Arc<DatabaseConnection>,
}

#[async_trait]
impl KnowledgeService for BaseService {
    fn name(&self) -> &str {
        "base"
    }

    async fn start(&mut self) -> Result<()> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        Ok(())
    }

    fn rpc_methods(&self) -> Methods {
        let rpc = BaseRpcImpl {
            db: self.db.clone(),
        };
        rpc.into_rpc().into()
    }

    async fn on_new_block(
        &mut self,
        _block_number: u64,
        _txs: Vec<RethTx>,
    ) -> Result<()> {
        Ok(())
    }
}
