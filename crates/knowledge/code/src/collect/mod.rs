// collect contract code from blockchain

use std::{
    cell::RefCell,
    sync::{atomic::AtomicU64, Arc},
    thread,
};

use crossbeam::{
    channel::{self, Receiver, Sender},
    select,
};
use foundry_block_explorers::errors::EtherscanError;
use libsofl_core::{
    blockchain::{
        provider::{BcProvider, BcStateProvider},
        transaction::Tx,
    },
    conversion::ConvertTo,
    engine::{
        state::BcState, transition::TransitionSpecBuilder, types::DatabaseRef,
    },
};
use libsofl_knowledge_base::config::KnowledgeConfig;
use libsofl_utils::{
    log::{debug, error, info},
    sync::runtime::AsyncRuntime,
};
use sea_orm::{sea_query::OnConflict, EntityTrait};
use serde::{Deserialize, Serialize};

use crate::{entities, error::Error};

pub mod contract_inspector;
pub mod fetcher;

static CODE_KNOWLEDGE_METADATA_KEY: &str = "code_knowledge";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodeKnowledgeMetadata {
    pub progress: u64,
}

pub struct CodeKnowledgeCollector<
    T: Tx,
    S: DatabaseRef,
    P: BcProvider<T> + BcStateProvider<S>,
> {
    provider: Arc<P>,
    fetcher: Arc<fetcher::Fetcher>,
    db_cfg: KnowledgeConfig,
    eager: bool,

    pub progress: Arc<AtomicU64>,
    pub task_tx: Sender<u64>,
    task_rx: Receiver<u64>,
    close_tx: Sender<()>,
    close_rx: Receiver<()>,
    worker: RefCell<thread::JoinHandle<()>>,

    _phantom: std::marker::PhantomData<(T, S)>,
}

impl<T: Tx, S: DatabaseRef, P: BcProvider<T> + BcStateProvider<S>>
    CodeKnowledgeCollector<T, S, P>
{
    pub async fn new(
        provider: P,
        db_cfg: KnowledgeConfig,
        fetcher: Arc<fetcher::Fetcher>,
        eager: bool, // if the database says it is not verified, whehter or not to fetch it
    ) -> Self {
        let db = db_cfg
            .get_database_connection()
            .await
            .expect("failed to connect to database");
        // load progress
        let metadata =
            libsofl_knowledge_base::entities::metadata::Entity::find_by_id(
                CODE_KNOWLEDGE_METADATA_KEY,
            )
            .one(&db)
            .await
            .expect("failed to load metadata");
        let progress = match metadata {
            Some(metadata) => {
                let metadata: CodeKnowledgeMetadata =
                    metadata.try_decode().expect("failed to decode metadata");
                metadata.progress
            }
            None => 0,
        };
        let (close_tx, close_rx) = channel::unbounded();
        let (task_tx, task_rx) = channel::bounded(10);
        Self {
            provider: Arc::new(provider),
            fetcher,
            db_cfg,
            eager,
            progress: Arc::new(AtomicU64::new(progress)),
            task_tx,
            task_rx,
            close_tx,
            close_rx,
            worker: RefCell::new(thread::spawn(|| {})),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<
        T: Tx,
        S: DatabaseRef,
        P: BcProvider<T> + BcStateProvider<S> + Send + Sync + 'static,
    > CodeKnowledgeCollector<T, S, P>
where
    <S as DatabaseRef>::Error: std::fmt::Debug,
{
    pub fn set_progress(&self, progress: u64) {
        self.progress
            .store(progress, std::sync::atomic::Ordering::Release);
    }

    pub fn start(&self) {
        self.spawn_worker();
    }

    pub fn feed_block_task(&self, bn: u64) {
        self.task_tx.send(bn).unwrap();
    }

    fn spawn_worker(&self) {
        let progress = self.progress.clone();
        let close_rx = self.close_rx.clone();
        let task_rx = self.task_rx.clone();
        let provider = self.provider.clone();
        let fetcher = self.fetcher.clone();
        let db_cfg = self.db_cfg.clone();
        let eager = self.eager;
        let thread = thread::spawn(move || {
            let rt = AsyncRuntime::new();
            let task = async { db_cfg.get_database_connection().await };
            let db = rt.block_on(task).expect("failed to connect to database");
            loop {
                select! {
                    recv(task_rx) -> msg => {
                        if let Ok(bn) = msg {
                            progress.store(bn, std::sync::atomic::Ordering::Release);
                        }
                    }
                    recv(close_rx) -> _ => return,
                }

                let bn: u64 =
                    progress.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                let txs = match provider.txs_in_block(bn.cvt()) {
                    Ok(txs) => txs,
                    Err(e) => {
                        error!(block = bn, error = ?e, "failed to load transactions");
                        continue;
                    }
                };
                info!(block = bn, txs = txs.len(), "collecting code knowledge");

                let mut spec_builder = TransitionSpecBuilder::default()
                    .at_block(&provider, bn.cvt());
                for tx in txs {
                    spec_builder = spec_builder.append_tx(tx);
                }
                let spec = spec_builder.build();

                let mut insp = contract_inspector::ContractInspector {
                    contracts: Default::default(),
                };
                let mut state = provider.bc_state_at(bn.cvt()).unwrap();
                match state.transit(spec, &mut insp) {
                    Ok(_) => {}
                    Err(e) => {
                        error!(block = bn, error = ?e, "failed to replay transactions");
                        continue;
                    }
                };

                let contracts = insp.contracts;
                for contract in contracts {
                    let task = async {
                        entities::code::Entity::find_by_id(contract.to_string())
                            .one(&db)
                            .await
                    };
                    match rt.block_on(task) {
                        Ok(m) => {
                            if let Some(m) = m {
                                if !eager || m.verified {
                                    debug!(
                                        contract = contract.to_string(),
                                        "contract code already fetched"
                                    );
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            error!(contract = contract.to_string(), error = ?e, "failed to load contract");
                            continue;
                        }
                    };

                    debug!(
                        contract = contract.to_string(),
                        "fetching contract code"
                    );
                    match close_rx.try_recv() {
                        Ok(_) => return,
                        Err(e) => match e {
                            channel::TryRecvError::Empty => {}
                            channel::TryRecvError::Disconnected => return,
                        },
                    }

                    let task = async { fetcher.fetch_one(contract).await };
                    let model = match rt.block_on(task) {
                        Ok(model) => {
                            debug!(
                                contract = contract.to_string(),
                                "contract code fetched"
                            );
                            model
                        }
                        Err(e) => {
                            if !matches!(
                                e,
                                Error::Etherscan(
                                    EtherscanError::ContractCodeNotVerified(_)
                                )
                            ) {
                                error!(contract = contract.to_string(), error = ?e, "failed to fetch contract");
                            }
                            debug!(
                                contract = contract.to_string(),
                                "contract code not available"
                            );
                            continue;
                        }
                    };

                    let task = async {
                        let model: entities::code::ActiveModel = model.into();
                        entities::code::Entity::insert(model)
                            .on_conflict(
                                OnConflict::column(
                                    entities::code::Column::Contract,
                                )
                                .do_nothing()
                                .to_owned(),
                            )
                            .exec(&db)
                            .await
                    };
                    match rt.block_on(task) {
                        Ok(_) => {}
                        Err(e) => {
                            error!(contract = contract.to_string(), error = ?e, "failed to insert contract");
                            continue;
                        }
                    }
                }

                // save progress
                let metadata = CodeKnowledgeMetadata {
                    progress: progress
                        .load(std::sync::atomic::Ordering::Acquire),
                };
                let model =
                    libsofl_knowledge_base::entities::metadata::ActiveModel::from((CODE_KNOWLEDGE_METADATA_KEY.to_string(), metadata));
                let task = async {
                    libsofl_knowledge_base::entities::metadata::Entity::insert(
                        model,
                    )
                    .on_conflict(OnConflict::column(
                        libsofl_knowledge_base::entities::metadata::Column::Key,
                    ).update_column(
                        libsofl_knowledge_base::entities::metadata::Column::Value,
                    ).to_owned())
                    .exec(&db)
                    .await
                };
                match rt.block_on(task) {
                    Ok(_) => {}
                    Err(e) => {
                        error!(error = ?e, "failed to save metadata");
                        continue;
                    }
                }
            }
        });
        self.worker.replace(thread);
    }

    pub fn stop(&self) {
        info!("stopping code knowledge collector");
        self.close_tx.send(()).unwrap();
        self.worker.replace(thread::spawn(|| {})).join().unwrap();
        info!("code knowledge collector stopped");
    }
}
