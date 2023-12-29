use std::{ops::Range, sync::Arc};

use data::DataStore;
use libsofl_knowledge::config::KnowledgeConfig;
use libsofl_reth::config::RethConfig;
use libsofl_utils::{
    config::Config,
    log::{error, info},
};
pub mod analyze;
pub mod data;

#[tokio::main(worker_threads = 32)]
async fn main() {
    collect_blocks(0..100000, 8).await;
}

async fn collect_blocks(blocks: Range<u64>, step: usize) {
    let db = KnowledgeConfig::must_load()
        .get_database_connection()
        .await
        .unwrap();
    let provider = RethConfig::must_load().bc_provider().unwrap();
    let provider = Arc::new(provider);
    let analyzer = analyze::Analyzer::new(provider);
    let mut store = DataStore::new(db, 100).await.unwrap();

    for block in blocks.step_by(step) {
        let mut tasks = Vec::new();
        for _ in block..block + step as u64 {
            let mut analyzer_cloned = analyzer.clone();
            let task = tokio::spawn(async move {
                analyzer_cloned.analyze_one_block(block).await
            });
            tasks.push(task);
        }
        for offset in 0..step as u64 {
            let task = tasks.remove(0);
            let bn = block + offset;
            match task.await.unwrap() {
                Ok((creations, invocations)) => {
                    let r = store.add_creations(bn, creations).await;
                    if let Err(e) = r {
                        error!(
                            err = format!("{:?}", e),
                            block = bn,
                            "failed to add creations"
                        );
                        store.add_failed_block(bn);
                        break;
                    }
                    let r = store.add_invocations(bn, invocations).await;
                    if let Err(e) = r {
                        error!(
                            err = format!("{:?}", e),
                            block = bn,
                            "failed to add invocations"
                        );
                        store.add_failed_block(bn);
                        break;
                    }
                    info!(block = bn, "block analyzed")
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
