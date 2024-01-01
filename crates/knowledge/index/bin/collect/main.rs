use std::sync::Arc;

use clap::Parser;
use data::DataStore;
use futures::stream::StreamExt;
use libsofl_core::error::SoflError;
use libsofl_knowledge_index::config::KnowledgeConfig;
use libsofl_reth::config::RethConfig;
use libsofl_utils::{
    config::Config,
    log::{config::LogConfig, error, info, warn},
};
use sea_orm::DbErr;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

pub mod analyze;
pub mod data;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Arg {
    #[arg(short, long, default_value = "info")]
    level: String,

    #[arg(short, long, default_value = None)]
    file: Option<String>, // log file path

    #[arg(short, long, default_value = "8")]
    jobs: usize,

    #[arg(help = "until block number (exclusive)")]
    until_block: u64,

    #[arg(short, long, default_value = "100")]
    db_flush_threshold: u64,
}

#[tokio::main(worker_threads = 32)]
async fn main() {
    let args = Arg::parse();
    let log_cfg = LogConfig {
        console_level: args.level.clone(),
        file_level: args.level.clone(),
        file: args.file.clone(),
    };
    log_cfg.init();
    info!(
        until = args.until_block,
        "start indexing transaction hisotry"
    );

    let cancellation_token = CancellationToken::new();

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
                }
                _ => unreachable!(),
            }
        }
    });

    let task_tracker = TaskTracker::new();
    collect_blocks(
        args.until_block,
        args.jobs,
        cancellation_token.clone(),
        &task_tracker,
        args.db_flush_threshold,
    )
    .await;
    task_tracker.close();
    task_tracker.wait().await;

    handle.close();
    signal_task.await.unwrap();
}

async fn collect_blocks(
    until_block: u64,
    step: usize,
    cancellation_token: CancellationToken,
    task_tracker: &TaskTracker,
    db_flush_threshold: u64,
) {
    let cfg = KnowledgeConfig::load_or_default(Default::default())
        .expect("failed to load config");
    let db = cfg.get_database_connection().await.unwrap();
    info!(url = cfg.database_url, "database connected");
    let cfg = RethConfig::must_load();
    let provider = cfg.bc_provider().unwrap();
    info!(datadir = cfg.datadir, "reth blockchain provider connected");
    let provider = Arc::new(provider);
    let analyzer = analyze::Analyzer::new(provider);
    let mut store = DataStore::new(&db, db_flush_threshold).await.unwrap();

    for block in
        ((store.get_last_finished_block() + 1)..until_block).step_by(step)
    {
        let mut tasks = Vec::new();
        for bn in block..(block + step as u64).min(until_block) {
            let mut analyzer_cloned = analyzer.clone();
            let task = task_tracker
                .spawn_blocking(move || analyzer_cloned.analyze_one_block(bn));
            tasks.push(task);
        }

        if cancellation_token.is_cancelled() {
            warn!(block = block, "block analysis interrupted");
            break;
        }

        for offset in 0..step as u64 {
            let bn = block + offset;
            if bn >= until_block {
                break;
            }
            let task = tasks.remove(0);
            match task.await.unwrap() {
                Ok((creations, invocations)) => {
                    let r = store.add_creations(bn, creations).await;
                    if let Err(e) = r {
                        if e != DbErr::RecordNotInserted {
                            error!(
                                err = format!("{:?}", e),
                                block = bn,
                                "failed to add creations"
                            );
                            store.add_failed_block(bn);
                            continue;
                        }
                    }
                    let r = store.add_invocations(bn, invocations).await;
                    if let Err(e) = r {
                        if e != DbErr::RecordNotInserted {
                            error!(
                                err = format!("{:?}", e),
                                block = bn,
                                "failed to add invocations"
                            );
                            store.add_failed_block(bn);
                            continue;
                        }
                    }
                }
                Err(SoflError::Interrupted) => {
                    warn!(block = bn, "block analysis interrupted");
                    break;
                }
                Err(e) => {
                    error!(
                        err = format!("{:?}", e),
                        block = bn,
                        "failed to analyzed block"
                    );
                    store.add_failed_block(bn);
                }
            }
            store.update_last_finished_block(bn);
        }
    }
    store.save_progress().await.unwrap();
}
