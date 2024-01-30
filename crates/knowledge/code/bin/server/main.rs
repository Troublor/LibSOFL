use std::sync::Arc;

use clap::{arg, command, Parser};
use futures::StreamExt;
use libsofl_knowledge_code::{
    collect::collector::CollectorService, rpc::service::CodeRpcService,
};
use libsofl_reth::config::RethConfig;
use libsofl_utils::{
    config::ConfigDefault,
    log::{config::LogConfig, info},
};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook_tokio::Signals;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    #[arg(short, long)]
    level: Option<String>,

    #[arg(long, default_value = "false")]
    disable_miner: bool,

    #[arg(long, default_value = "19000000")]
    miner_max_block: u64,

    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(short, long, default_value = "2425")]
    port: usize,

    #[arg(short, long, default_value = "false")]
    eager: bool,

    #[arg(long)]
    chain_id: Option<u64>,

    #[arg(long)]
    database: Option<String>,

    #[arg(long)]
    datadir: Option<String>,
}

#[tokio::main(flavor = "multi_thread")] 
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    // knowledge base
    let mut knowledge_cfg =
        libsofl_knowledge_base::config::KnowledgeConfig::must_load_or_default();
    knowledge_cfg.database_url =
        args.database.unwrap_or(knowledge_cfg.database_url);
    knowledge_cfg.database_log_level = args
        .level
        .unwrap_or(knowledge_cfg.database_log_level.to_string());

    // code knowledge
    let mut code_knowledge_cfg =
        libsofl_knowledge_code::config::CodeKnowledgeConfig::must_load_or_default();
    code_knowledge_cfg.chain_id =
        args.chain_id.unwrap_or(code_knowledge_cfg.chain_id);
    code_knowledge_cfg.requests_per_second = Some(5.0);
    let query = libsofl_knowledge_code::query::query::CodeQuery::new(
        &knowledge_cfg,
        &code_knowledge_cfg,
        args.eager,
    )
    .await
    .expect("failed to create fetcher");
    let query = Arc::new(query);

    let mut collector = if !args.disable_miner {
        let db = knowledge_cfg
            .get_database_connection()
            .await
            .expect("failed to connect to database");
        let service = CollectorService::new(
            provider.clone(),
            query.clone(),
            db,
            args.miner_max_block,
        )
        .await;
        Some(service)
    } else {
        None
    };

    let rpc = CodeRpcService::new(query.clone(), &args.host, args.port)
        .await
        .expect("failed to start rpc service");

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

    interrupt.await.unwrap();

    // cleanup
    collector.take().map(|c| drop(c));
    drop(rpc);

    Ok(())
}
