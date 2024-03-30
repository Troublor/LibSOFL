use std::{cell::RefCell, sync::Arc};

use eyre::{eyre, Result};
use jsonrpsee::{
    server::{ServerBuilder, ServerHandle},
    Methods,
};
use libsofl_knowledge_base::service::KnowledgeService;
use libsofl_reth::blockchain::provider::RethProvider;
use libsofl_utils::log::info;

pub struct KnowledgeServer<'a> {
    pub provider: Arc<RethProvider>,
    pub host: String,
    pub port: usize,

    pub(crate) server: RefCell<Option<ServerHandle>>,
    pub(crate) services: Vec<Box<dyn KnowledgeService + 'a>>,
}

impl<'a> KnowledgeServer<'a> {
    pub fn new(provider: Arc<RethProvider>, host: String, port: usize) -> Self {
        Self {
            provider,
            host,
            port,
            server: RefCell::new(None),
            services: Vec::new(),
        }
    }

    pub fn register_service(
        &mut self,
        service: Box<dyn KnowledgeService + 'a>,
    ) {
        self.services.push(service);
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.server.borrow().is_some() {
            return Err(eyre!("server already started"));
        }

        let server = ServerBuilder::default()
            .build(&format!("{}:{}", self.host, self.port))
            .await?;

        let mut methods = Methods::new();
        for service in self.services.iter_mut() {
            info!(service = service.name(), "starting service");
            service.start().await?;
            methods.merge(service.rpc_methods())?;
        }

        let server_handle = server.start(methods);
        self.server.replace(Some(server_handle));
        info!(
            host = &self.host,
            port = self.port,
            "started knowledge server"
        );

        self.listen_to_new_blocks();
        info!("listening for new blocks");

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(server) = self.server.borrow_mut().take() {
            info!("stopping knowledge server");
            server.stop()?;
            server.stopped().await;
        }

        for service in self.services.iter_mut() {
            info!(service = service.name(), "stopping service");
            service.stop().await?;
        }
        Ok(())
    }

    fn listen_to_new_blocks(&mut self) {}
}
