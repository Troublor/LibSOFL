use eyre::Result;
use std::sync::Arc;

use jsonrpsee::core::async_trait;
use libsofl_knowledge_base::service::KnowledgeService;
use sea_orm::DatabaseConnection;

use super::IndexRpcServer;

pub struct IndexService {
    pub db: Arc<DatabaseConnection>,
}

#[async_trait]
impl KnowledgeService for IndexService {
    fn name(&self) -> &str {
        "index"
    }

    async fn start(&mut self) -> Result<()> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        Ok(())
    }

    fn rpc_methods(&self) -> jsonrpsee::Methods {
        let rpc = crate::rpc::IndexRpcImpl {
            db: self.db.clone(),
        };
        rpc.into_rpc().into()
    }

    async fn on_new_block(
        &mut self,
        _block_number: u64,
        _txs: Vec<libsofl_reth::blockchain::transaction::RethTx>,
    ) -> Result<()> {
        Ok(())
    }
}
