use eyre::Result;
use jsonrpsee::{core::async_trait, Methods};
use libsofl_reth::blockchain::transaction::RethTx;

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
