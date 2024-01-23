use std::{net::SocketAddr, sync::Arc};

use jsonrpsee::server::ServerBuilder;
use libsofl_utils::log::info;
use tokio::task::JoinHandle;

use crate::query::query::CodeQuery;

use super::{CodeRpcImpl, CodeRpcServer};

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
