use std::sync::Arc;

use clap::{arg, command, Parser};
use eyre::Result;
use futures::StreamExt;
use libsofl_knowledge_base::{
    config::KnowledgeConfig, service::KnowledgeService,
};
use libsofl_knowledge_code::rpc::service::CodeService;
use libsofl_knowledge_service::KnowledgeServer;
use libsofl_reth::{blockchain::provider::RethProvider, config::RethConfig};
use libsofl_utils::{
    config::ConfigDefault,
    log::{config::LogConfig, info},
};
use sea_orm::DatabaseConnection;
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook_tokio::Signals;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    #[arg(short, long)]
    level: Option<String>,

    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(short, long, default_value = "2425")]
    port: usize,

    #[arg(long)]
    chain_id: Option<u64>,

    #[arg(long)]
    database: Option<String>,

    #[arg(long)]
    datadir: Option<String>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = Cli::parse();

    // logger
    let mut log_cfg = LogConfig::must_load_or_default();
    log_cfg.console_level = args.level.clone().unwrap_or(log_cfg.console_level);
    log_cfg.init();

    // provider
    let mut reth_cfg = RethConfig::must_load_or_default();
    reth_cfg.datadir = args.datadir.unwrap_or(reth_cfg.datadir);
    let provider = reth_cfg.bc_provider().expect("failed to load bc provider");
    let provider = Arc::new(provider);

    // base config
    let mut base_cfg =
        libsofl_knowledge_base::config::KnowledgeConfig::must_load_or_default();
    base_cfg.database_url = args.database.unwrap_or(base_cfg.database_url);
    base_cfg.database_log_level = args
        .level
        .unwrap_or(base_cfg.database_log_level.to_string());

    // database
    let db = base_cfg.get_database_connection().await?;
    let db = Arc::new(db);

    // services
    let services: Vec<Box<dyn KnowledgeService>> = vec![
        create_base_service(db.clone()),
        create_code_service(provider.clone(), db.clone(), &base_cfg).await?,
    ];

    // server
    let mut server =
        KnowledgeServer::new(provider.clone(), args.host, args.port);
    services
        .into_iter()
        .for_each(|service| server.register_service(service));

    // start server
    server.start().await?;

    // handle signals
    let mut signals =
        Signals::new(&[SIGINT, SIGTERM]).expect("failed to register signals");
    let interrupt = tokio::spawn(async move {
        while let Some(sig) = signals.next().await {
            match sig {
                SIGINT | SIGTERM => {
                    info!(sig = sig, "received signal, stopping...");
                    break;
                }
                _ => unreachable!(),
            }
        }
    });

    // wait for interrupt
    interrupt.await?;
    // stop server
    server.stop().await?;

    Ok(())
}

async fn create_code_service(
    provider: Arc<RethProvider>,
    db: Arc<DatabaseConnection>,
    base_cfg: &KnowledgeConfig,
) -> Result<Box<dyn KnowledgeService>> {
    let mut code_knowledge_cfg =
        libsofl_knowledge_code::config::CodeKnowledgeConfig::must_load_or_default();
    code_knowledge_cfg.requests_per_second = Some(5.0);
    let service =
        CodeService::new(provider, db, base_cfg, &code_knowledge_cfg).await?;
    Ok(Box::new(service))
}

fn create_base_service(
    db: Arc<DatabaseConnection>,
) -> Box<dyn KnowledgeService> {
    let service = libsofl_knowledge_base::service::BaseService { db };
    Box::new(service)
}
