use std::{sync::Arc, thread};

use clap::{arg, command, Parser};
use crossbeam::{atomic::AtomicConsume, sync::WaitGroup};
use futures::StreamExt;
use libsofl_reth::{blockchain::provider::BlockNumReader, config::RethConfig};
use libsofl_utils::{
    config::ConfigDefault,
    log::{config::LogConfig, info},
};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook_tokio::Signals;
use tokio_util::sync::CancellationToken;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    #[arg(short, long)]
    level: Option<String>,

    #[arg(long, default_value = "false")]
    disable_miner: bool,

    #[arg(short, long, default_value = "2425")]
    port: usize,

    #[arg(short, long, default_value = "false")]
    eager: bool,

    #[arg(long)]
    chain_id: Option<u64>,

    #[arg(long)]
    api_key: Option<String>,

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
    code_knowledge_cfg.api_key =
        args.api_key.unwrap_or(code_knowledge_cfg.api_key);
    code_knowledge_cfg.requests_per_second = Some(5.0);
    let fetcher = libsofl_knowledge_code::collect::fetcher::Fetcher::new(
        code_knowledge_cfg.clone(),
    )
    .expect("failed to create fetcher");
    let fetcher = Arc::new(fetcher);

    let collector = if !args.disable_miner {
        let collector =
            libsofl_knowledge_code::collect::CodeKnowledgeCollector::new(
                provider.clone(),
                knowledge_cfg.clone(),
                fetcher.clone(),
                args.eager,
            )
            .await;
        collector.start();
        Some(collector)
    } else {
        None
    };
    let collector_task_tx = collector
        .as_ref()
        .map(|c| (c.task_tx.clone(), c.progress.load_consume()));

    let cancellation_token = CancellationToken::new();
    let wg = WaitGroup::new();

    // handle signals
    let mut signals =
        Signals::new(&[SIGINT, SIGTERM]).expect("failed to register signals");
    let handle = signals.handle();
    let cancel = cancellation_token.clone();
    let signal_task = tokio::spawn(async move {
        while let Some(sig) = signals.next().await {
            match sig {
                SIGINT | SIGTERM => {
                    info!(sig = sig, "received signal, stopping...");
                    cancel.cancel();
                    collector.as_ref().map(|c| c.stop());
                    break;
                }
                _ => unreachable!(),
            }
        }
    });

    let task_feeder = if let Some((task_tx, progress)) = collector_task_tx {
        let p = provider.clone();
        let wg = wg.clone();
        let task_feeder = thread::spawn(move || {
            let mut current = progress;
            let mut latest_block = p.bp.best_block_number().unwrap();
            let mut should_break = false;
            while latest_block > current {
                if should_break {
                    break;
                }
                for block in current..latest_block {
                    if cancellation_token.is_cancelled() {
                        should_break = true;
                        break;
                    }
                    match task_tx.send(block) {
                        Ok(_) => {}
                        Err(_) => {
                            should_break = true;
                            break;
                        }
                    }
                    current = block;
                }
                latest_block = p.bp.best_block_number().unwrap();
            }
            drop(wg);
        });
        Some(task_feeder)
    } else {
        None
    };

    wg.wait();
    task_feeder.map(|t| t.join().unwrap());
    info!("task feeder stopped");

    handle.close();
    signal_task.await.unwrap();
    info!("signal handler stopped");

    Ok(())
}
