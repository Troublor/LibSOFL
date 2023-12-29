use std::collections::{BTreeMap, HashSet};

use libsofl_knowledge::entities;
use libsofl_utils::log::{debug, info};
use sea_orm::{sea_query, EntityTrait};

#[derive(
    Debug, Clone, Eq, PartialEq, Default, serde::Deserialize, serde::Serialize,
)]
struct Progress {
    /// mapping from contract address to the block number from which to the current block that the contract is continuously invoked.
    pub(crate) last_finished_block: u64,
    pub(crate) failed_blocks: Vec<u64>,
}

impl Progress {
    fn new() -> Self {
        Self {
            last_finished_block: 0,
            failed_blocks: Vec::new(),
        }
    }
}

pub(crate) struct DataStore {
    db: sea_orm::DatabaseConnection,

    progress: Progress,
    pending_invocations: BTreeMap<String, u64>,

    flush_threshold: u64,

    creations_to_insert: Vec<entities::creation::ActiveModel>,
    invocations_to_insert: Vec<entities::invocation::ActiveModel>,
}

const METADATA_KEY: &str = "tx_index_progress";

impl DataStore {
    pub async fn new(
        db: sea_orm::DatabaseConnection,
        flush_threshold: u64,
    ) -> Result<Self, sea_orm::DbErr> {
        // load progress
        info!("loading progress from database");
        let progress =
            match entities::metadata::Entity::find_by_id(METADATA_KEY)
                .one(&db)
                .await?
            {
                Some(m) => {
                    let p: Progress = m.try_decode().expect("invalid metadata");
                    info!(
                        last_finished_block = p.last_finished_block,
                        "progress resumed"
                    );
                    p
                }
                None => {
                    info!("no progress found, creating new");
                    Progress::new()
                }
            };
        Ok(Self {
            db,
            progress,
            pending_invocations: BTreeMap::new(),
            flush_threshold,
            creations_to_insert: Vec::new(),
            invocations_to_insert: Vec::new(),
        })
    }
}

impl DataStore {
    /// Add the invoked contract list to the pending_invocations.
    /// If a previously-pending address is not invoked in the current block (the given address list),
    /// It will be flushed to the underlying database.
    pub(crate) async fn add_invocations(
        &mut self,
        block: u64,
        addresses: HashSet<String>,
    ) -> Result<(), sea_orm::DbErr> {
        assert_eq!(
            block,
            self.progress.last_finished_block + 1,
            "block number must be continuous"
        );
        let pending_addresses = self
            .pending_invocations
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        for pending_addr in pending_addresses {
            if !addresses.contains(&pending_addr) {
                let from_block = self
                    .pending_invocations
                    .remove(&pending_addr)
                    .expect("impossible: pending address not found");
                let invocation = entities::invocation::Model {
                    contract: pending_addr,
                    from_block,
                    to_block: block - 1,
                };
                self.invocations_to_insert.push(invocation.into());
                self.flush_invocations().await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn add_creations(
        &mut self,
        block: u64,
        creations: Vec<(String, String, bool)>,
    ) -> Result<(), sea_orm::DbErr> {
        for (contract, tx, destruct) in creations {
            let creation = entities::creation::Model {
                contract,
                tx,
                block,
                destruct,
            };
            self.creations_to_insert.push(creation.into());
            self.flush_creations().await?;
        }
        Ok(())
    }

    pub(crate) fn add_failed_block(&mut self, block: u64) {
        self.progress.failed_blocks.push(block)
    }

    pub(crate) fn update_last_finished_block(&mut self, block: u64) {
        self.progress.last_finished_block = block;
    }

    pub(crate) async fn save_progress(&mut self) -> Result<(), sea_orm::DbErr> {
        let progress = entities::metadata::ActiveModel::from((
            METADATA_KEY.to_string(),
            serde_json::to_value(&self.progress).unwrap(),
        ));
        let threshold = self.flush_threshold;
        self.flush_threshold = 0;
        self.flush_creations().await?;
        self.flush_invocations().await?;
        self.flush_threshold = threshold;
        entities::metadata::Entity::insert(progress)
            .on_conflict(
                sea_query::OnConflict::column(entities::metadata::Column::Key)
                    .update_column(entities::metadata::Column::Value)
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;
        Ok(())
    }

    // database operations

    async fn flush_invocations(&mut self) -> Result<(), sea_orm::DbErr> {
        if self.invocations_to_insert.len() >= self.flush_threshold as usize {
            debug!(
                count = self.invocations_to_insert.len(),
                "flushing invocations to database"
            );
            entities::invocation::Entity::insert_many(
                self.invocations_to_insert.clone(),
            )
            .exec(&self.db)
            .await?;
            self.invocations_to_insert.clear();
        }
        Ok(())
    }

    async fn flush_creations(&mut self) -> Result<(), sea_orm::DbErr> {
        if self.creations_to_insert.len() >= self.flush_threshold as usize {
            debug!(
                count = self.creations_to_insert.len(),
                "flushing creations to database"
            );
            entities::creation::Entity::insert_many(
                self.creations_to_insert.clone(),
            )
            .exec(&self.db)
            .await?;
            self.creations_to_insert.clear();
        }
        Ok(())
    }
}
