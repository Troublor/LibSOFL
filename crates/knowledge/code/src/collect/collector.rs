use std::sync::{atomic::AtomicU64, Arc};

use crossbeam::atomic::AtomicConsume;
use libsofl_core::{
    blockchain::{
        provider::{BcProvider, BcStateProvider},
        transaction::Tx,
    },
    conversion::ConvertTo,
    engine::{
        state::BcState,
        transition::TransitionSpecBuilder,
        types::{Address, BcStateRef},
    },
};
use libsofl_utils::log::{error, info};
use sea_orm::{sea_query::OnConflict, DatabaseConnection, EntityTrait};
use tokio::task::JoinHandle;

use crate::{
    collect::{
        contract_inspector, CodeKnowledgeMetadata, CODE_KNOWLEDGE_METADATA_KEY,
    },
    error::Error,
    query::query::CodeQuery,
};

pub struct CollectorService {
    task: JoinHandle<()>,
}

impl CollectorService {
    pub async fn new<T, D, P>(
        provider: Arc<P>,
        query: Arc<CodeQuery>,
        db: DatabaseConnection,
        max_bn: u64,
    ) -> Self
    where
        T: Tx + 'static,
        D: BcStateRef + 'static,
        D::Error: std::fmt::Debug,
        P: BcProvider<T> + BcStateProvider<D> + 'static,
    {
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
                info!(
                    progress = metadata.progress,
                    "resuming collecting code knowledge"
                );
                metadata.progress
            }
            None => 1,
        };
        let current_bn = AtomicU64::new(progress);
        let current_bn = Arc::new(current_bn);
        let cbn = current_bn.clone();
        let handle = tokio::task::spawn(async move {
            let collector = Collector::new(db, query, provider, cbn, max_bn);
            collector.worker_loop().await;
        });

        info!(block = progress, "started collecting code knowledge");

        Self { task: handle }
    }
}

impl Drop for CollectorService {
    fn drop(&mut self) {
        self.task.abort();
        info!("stopped collecting code knowledge");
    }
}

pub struct Collector<T, D, P>
where
    T: Tx + 'static,
    D: BcStateRef + 'static,
    D::Error: std::fmt::Debug,
    P: BcProvider<T> + BcStateProvider<D> + 'static,
{
    provider: Arc<P>,
    query: Arc<CodeQuery>,
    db: DatabaseConnection,
    max_bn: u64, // exclusive
    current_bn: Arc<AtomicU64>,

    _phantom: std::marker::PhantomData<(T, D)>,
}

impl<T, D, P> Collector<T, D, P>
where
    T: Tx + 'static,
    D: BcStateRef + 'static,
    D::Error: std::fmt::Debug,
    P: BcProvider<T> + BcStateProvider<D> + 'static,
{
    /// Start a background thread to collect code until `max_bn` (exclusive).
    pub fn new(
        db: DatabaseConnection,
        query: Arc<CodeQuery>,
        p: Arc<P>,
        current_bn: Arc<AtomicU64>,
        max_bn: u64,
    ) -> Self
    where
        T: Tx + 'static,
        D: BcStateRef + 'static,
        D::Error: std::fmt::Debug,
        P: BcProvider<T> + BcStateProvider<D> + 'static,
    {
        Self {
            provider: p,
            query: query,
            db,
            max_bn,
            current_bn,
            _phantom: std::marker::PhantomData,
        }
    }

    pub async fn worker_loop(&self) {
        while self.current_bn.load_consume() < self.max_bn {
            let bn = self
                .current_bn
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match self.process_one_block(bn).await {
                Ok(_) => {}
                Err(err) => {
                    error!(block = bn, err = ?err, "failed to process block");
                }
            };
        }
        info!(block = self.max_bn, "finished collecting code knowledge")
    }

    async fn process_one_block(&self, bn: u64) -> Result<(), Error> {
        let txs = self.provider.txs_in_block(bn.cvt()).map_err(Error::Sofl)?;
        info!(block = bn, txs = txs.len(), "collecting code knowledge");

        let mut spec_builder =
            TransitionSpecBuilder::default().at_block(&self.provider, bn.cvt());
        for tx in txs {
            spec_builder = spec_builder.append_tx(tx);
        }
        let spec = spec_builder.build();

        let mut insp = contract_inspector::ContractInspector {
            contracts: Default::default(),
        };
        let mut state = self.provider.bc_state_at(bn.cvt()).unwrap();
        state.transit(spec, &mut insp).map_err(Error::Sofl)?;

        for contract in insp.contracts {
            match self.process_one_contract(contract).await {
                Ok(_) => {}
                Err(err) => {
                    error!(contract = contract.to_string(), err = ?err, "failed to process contract");
                }
            };
        }

        // save progress
        let metadata = CodeKnowledgeMetadata {
            progress: self
                .current_bn
                .load(std::sync::atomic::Ordering::Acquire),
        };
        let model =
            libsofl_knowledge_base::entities::metadata::ActiveModel::from((
                CODE_KNOWLEDGE_METADATA_KEY.to_string(),
                metadata,
            ));
        libsofl_knowledge_base::entities::metadata::Entity::insert(model)
            .on_conflict(
                OnConflict::column(
                    libsofl_knowledge_base::entities::metadata::Column::Key,
                )
                .update_column(
                    libsofl_knowledge_base::entities::metadata::Column::Value,
                )
                .to_owned(),
            )
            .exec(&self.db)
            .await
            .map_err(Error::Database)?;
        Ok(())
    }

    async fn process_one_contract(
        &self,
        contract: Address,
    ) -> Result<(), Error> {
        // get model will update the database implicitly
        let _ = self.query.get_model_async(contract).await?;
        Ok(())
    }
}
