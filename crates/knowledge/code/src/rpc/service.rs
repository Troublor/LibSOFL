use std::{
    net::SocketAddr,
    sync::{atomic::AtomicU64, Arc},
};

use eyre::Result;
use jsonrpsee::{core::async_trait, server::ServerBuilder, Methods};
use libsofl_knowledge_base::{
    config::KnowledgeConfig, service::KnowledgeService,
};
use libsofl_reth::blockchain::{
    provider::{BlockNumReader, RethProvider},
    transaction::RethTx,
};
use libsofl_utils::log::info;
use sea_orm::{DatabaseConnection, EntityTrait};
use tokio::task::JoinHandle;

use crate::{
    collect::collector::Collector, config::CodeKnowledgeConfig,
    query::query::CodeQuery,
};

use super::{CodeRpcImpl, CodeRpcServer};
use crate::collect::{CodeKnowledgeMetadata, CODE_KNOWLEDGE_METADATA_KEY};

pub struct CodeRpcService {
    pub socket_addr: SocketAddr,
    handle: JoinHandle<()>,
}

impl CodeRpcService {
    pub async fn new(
        query: Arc<CodeQuery>,
        host: &str,
        port: usize,
    ) -> Result<Self, std::io::Error> {
        let server = ServerBuilder::default()
            .build(&format!("{}:{}", host, port))
            .await?;
        let addr = server.local_addr().unwrap();
        let rpc_impl = CodeRpcImpl { query };
        let server_handle = server.start(rpc_impl.into_rpc());
        let handle = tokio::task::spawn(server_handle.stopped());

        info!(host = host, port = port, "started code rpc service");

        Ok(Self {
            socket_addr: addr,
            handle,
        })
    }
}

impl Drop for CodeRpcService {
    fn drop(&mut self) {
        self.handle.abort();
        info!("stopped code rpc service");
    }
}

pub struct CodeService {
    pub provider: Arc<RethProvider>,
    pub query: Arc<CodeQuery>,
    pub db: Arc<DatabaseConnection>,

    _collector_task: Option<JoinHandle<()>>,
}

impl CodeService {
    pub async fn new(
        provider: Arc<RethProvider>,
        db: Arc<DatabaseConnection>,
        base_cfg: &KnowledgeConfig,
        code_cfg: &CodeKnowledgeConfig,
    ) -> Result<Self> {
        let query = Arc::new(CodeQuery::new(base_cfg, code_cfg, false).await?);
        Ok(Self {
            provider,
            query,
            db,
            _collector_task: None,
        })
    }

    async fn load_progress(&self) -> u64 {
        let metadata =
            libsofl_knowledge_base::entities::metadata::Entity::find_by_id(
                CODE_KNOWLEDGE_METADATA_KEY,
            )
            .one(self.db.as_ref())
            .await
            .expect("failed to load metadata");
        let progress = match metadata {
            Some(metadata) => {
                let metadata: CodeKnowledgeMetadata =
                    metadata.try_decode().expect("failed to decode metadata");
                info!(
                    progress = metadata.progress,
                    "resuming collecting code knowledge"
                );
                metadata.progress
            }
            None => 1,
        };
        progress
    }
}

#[async_trait]
impl KnowledgeService for CodeService {
    fn name(&self) -> &str {
        "code"
    }

    async fn start(&mut self) -> Result<()> {
        // start a task that collect until current best block
        let from = self.load_progress().await;
        let until = self.provider.best_block_number()?;
        let current_bn = AtomicU64::new(from);
        let collector = Collector::new(
            self.db.clone(),
            self.query.clone(),
            self.provider.clone(),
            Arc::new(current_bn),
        );
        let task = async move {
            collector.worker_loop(until).await;
        };
        let task_handle = tokio::spawn(task);
        self._collector_task = Some(task_handle);
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        // stop the collector task if any
        if let Some(handle) = self._collector_task.take() {
            if !handle.is_finished() {
                handle.abort();
            }
        }
        Ok(())
    }

    fn rpc_methods(&self) -> Methods {
        let rpc = CodeRpcImpl {
            query: self.query.clone(),
        };
        rpc.into_rpc().into()
    }

    async fn on_new_block(
        &mut self,
        block_number: u64,
        _txs: Vec<RethTx>,
    ) -> Result<()> {
        if self._collector_task.is_none()
            || self._collector_task.as_ref().unwrap().is_finished()
        {
            let from = self.load_progress().await;
            let collector = Collector::new(
                self.db.clone(),
                self.query.clone(),
                self.provider.clone(),
                Arc::new(AtomicU64::new(from)),
            );
            let task = async move {
                collector.worker_loop(block_number).await;
            };
            let task_handle = tokio::spawn(task);
            self._collector_task = Some(task_handle);
        }
        todo!()
    }
}
